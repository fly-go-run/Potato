"""Stage the pinned official Rust driver; no application runtime dependencies."""
import argparse
import hashlib
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile

VERSION = "0.24.0"
ARCHIVES = {
    "darwin": ("darwin-universal-binary.tar.gz", "31790cb49baa206f6455fbc259f8f83ae27e86be908f5c8cac5ec2f8521f8382"),
    "win32": ("windows-x86_64-binary.zip", "cc22d7a44ad526f779f2df7e6da053dd898ef8e5014b1ecfc01728645f691be0"),
}


def stage(destination, system=sys.platform, archive=None):
    suffix, digest = ARCHIVES[system]
    name = f"cua-driver-rs-{VERSION}-{suffix}"
    destination = Path(destination)
    with tempfile.TemporaryDirectory(prefix="potato-driver-") as temporary:
        source = Path(archive) if archive else Path(temporary) / name
        if archive is None:
            subprocess.run(["curl", "--fail", "--location", "--retry", "3",
                            "--connect-timeout", "30", "--max-time", "300", "--output", str(source),
                            f"https://github.com/trycua/cua/releases/download/cua-driver-rs-v{VERSION}/{name}"], check=True)
        if hashlib.sha256(source.read_bytes()).hexdigest() != digest:
            raise ValueError("Official computer driver archive checksum mismatch")
        binary = "cua-driver.exe" if system == "win32" else "cua-driver"
        # Extract only the standalone executable, never archive paths or SDK libraries.
        if system == "win32":
            with zipfile.ZipFile(source) as package:
                matches = [p for p in package.namelist() if p == binary or p.endswith("/" + binary)]
                if len(matches) != 1:
                    raise ValueError("Ambiguous driver archive")
                data = package.read(matches[0])
        else:
            with tarfile.open(source) as package:
                matches = [p for p in package.getmembers() if p.isfile() and p.name == binary]
                if len(matches) != 1:
                    raise ValueError("Missing standalone driver")
                data = package.extractfile(matches[0]).read()
        staged = Path(temporary) / binary
        staged.write_bytes(data)
        staged.chmod(0o755)
        if system == "darwin":
            # The pinned universal standalone archive has no usable enclosing
            # bundle signature. Sign the verified bytes as our nested helper.
            subprocess.run(["codesign", "--force", "--sign", "-", str(staged)], check=True)
            subprocess.run(["codesign", "--verify", "--strict", str(staged)], check=True)
        destination.mkdir(parents=True, exist_ok=True)
        shutil.copy2(staged, destination / binary)
        (destination / "VERSION").write_text(VERSION + "\n", encoding="utf-8")
    return destination / binary


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dest", type=Path, default=Path(__file__).resolve().parent / "target/computer-driver")
    parser.add_argument("--archive", type=Path, help="Use a local archive, still checked against the pinned digest")
    args = parser.parse_args()
    print(stage(args.dest, archive=args.archive))
