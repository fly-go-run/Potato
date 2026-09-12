"""Package GPUI releases with Python 3.11+ and NSIS (build tools only)."""

import argparse
import hashlib
import os
from pathlib import Path
import platform
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib

from stage_driver import stage

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[1]
TARGETS = {
    "aarch64-apple-darwin": ("darwin", "arm64"),
    "x86_64-apple-darwin": ("darwin", "x86_64"),
    "x86_64-pc-windows-msvc": ("win32", "x86_64"),
}


def version():
    value = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"][
        "version"
    ]
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", value):
        raise ValueError(f"Unsupported package version: {value}")
    return value


def run(*command):
    subprocess.run([str(arg) for arg in command], check=True)


def package_macos(
    binary,
    output,
    arch,
    release_version,
    debug,
    driver_archive=None,
):
    name = "Potato GPUI Review" if debug else "Potato GPUI"
    bundle = output / f"{name}.app"
    if bundle.exists():
        shutil.rmtree(bundle)
    executable = bundle / "Contents/MacOS/potato-gpui"
    executable.parent.mkdir(parents=True)
    shutil.copy2(binary, executable)
    executable.chmod(0o755)
    resources = bundle / "Contents/Resources"
    resources.mkdir()
    stage(resources / "computer-driver", archive=driver_archive)
    shutil.copyfile(
        REPO / "console/src-tauri/icons/icon.icns",
        resources / "icon.icns",
    )
    with (bundle / "Contents/Info.plist").open("wb") as stream:
        plistlib.dump(
            {
                "CFBundleExecutable": "potato-gpui",
                "CFBundleIdentifier": "dev.potato.gpui-review"
                if debug
                else "dev.potato.gpui",
                "CFBundleName": name,
                "CFBundleDisplayName": name,
                "CFBundleIconFile": "icon.icns",
                "CFBundlePackageType": "APPL",
                "CFBundleShortVersionString": release_version.split("-")[0],
                "CFBundleVersion": release_version.split("-")[0],
                "LSMinimumSystemVersion": "15.0",
                "NSHighResolutionCapable": True,
                "NSMicrophoneUsageDescription": "仅在点击麦克风时录音，将语音转换为聊天输入文字。",
            },
            stream,
        )
    run("codesign", "--force", "--sign", "-", bundle)
    run("codesign", "--verify", "--deep", "--strict", bundle)
    print(bundle)
    if debug:
        return []
    stem = f"Potato-GPUI-{release_version}-macOS-{arch}"
    archive = output / f"{stem}.zip"
    archive.unlink(missing_ok=True)
    # ditto preserves the executable bit and macOS bundle metadata.
    run(
        "ditto",
        "-c",
        "-k",
        "--sequesterRsrc",
        "--keepParent",
        bundle,
        archive,
    )
    dmg = output / f"{stem}.dmg"
    with tempfile.TemporaryDirectory(prefix="potato-dmg-") as staging:
        staged = Path(staging)
        run("ditto", bundle, staged / bundle.name)
        (staged / "Applications").symlink_to("/Applications")
        run(
            "hdiutil",
            "create",
            "-ov",
            "-volname",
            name,
            "-srcfolder",
            staged,
            "-format",
            "UDZO",
            dmg,
        )
    run("hdiutil", "verify", dmg)
    return [archive, dmg]


def package_windows(
    binary,
    output,
    release_version,
    debug,
    driver_archive=None,
):
    if debug:
        dest = output / "Potato-GPUI.exe"
        shutil.copy2(binary, dest)
        stage(output / "computer-driver", archive=driver_archive)
        return [dest]
    makensis = shutil.which("makensis")
    if not makensis:
        raise RuntimeError(
            "NSIS is required: install NSIS and add makensis.exe to PATH",
        )
    stem = f"Potato-GPUI-{release_version}-Windows-x86_64"
    payload = output / "Potato GPUI"
    if payload.exists():
        shutil.rmtree(payload)
    payload.mkdir()
    stage(payload / "computer-driver", archive=driver_archive)
    shutil.copy2(binary, payload / "Potato-GPUI.exe")
    shutil.copyfile(
        REPO / "console/src-tauri/icons/icon.ico",
        payload / "icon.ico",
    )
    shutil.copyfile(REPO / "LICENSE", payload / "LICENSE.txt")
    archive = Path(
        shutil.make_archive(
            str(output / f"{stem}-portable"),
            "zip",
            output,
            payload.name,
        ),
    )
    installer = output / f"{stem}-setup.exe"
    numeric_version = release_version.split("-")[0] + ".0"
    # /D arguments precede the script; subprocess preserves spaced paths.
    run(
        makensis,
        "/V3",
        "/WX",
        f"/DVERSION={release_version}",
        f"/DNUMERIC_VERSION={numeric_version}",
        f"/DPAYLOAD={payload}",
        f"/DOUTPUT={installer}",
        ROOT / "installer.nsi",
    )
    if not installer.is_file():
        raise RuntimeError("NSIS did not produce the requested installer")
    return [archive, installer]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--debug", action="store_true")
    parser.add_argument(
        "--driver-archive",
        type=Path,
        help="Local official driver archive; SHA-256 is always verified",
    )
    parser.add_argument(
        "--target",
        choices=TARGETS,
        help="Cargo --target triple (omit for host build)",
    )
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    parser.add_argument("--print-version", action="store_true")
    args = parser.parse_args()
    release_version = version()
    if args.print_version:
        print(release_version)
        return
    arch = {"aarch64": "arm64", "amd64": "x86_64"}.get(
        platform.machine().lower(),
        platform.machine().lower(),
    )
    if args.target:
        system, arch = TARGETS[args.target]
        if system != sys.platform:
            parser.error("Package on the target operating system")
    if sys.platform not in ("darwin", "win32") or arch not in (
        "arm64",
        "x86_64",
    ):
        parser.error(
            "Supported packages: macOS arm64/x86_64 and Windows x86_64",
        )
    if sys.platform == "win32" and arch != "x86_64":
        parser.error("Windows packaging currently requires x86_64")
    target_dir = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    if args.target:
        target_dir /= args.target
    binary = (
        target_dir
        / ("debug" if args.debug else "release")
        / ("potato-gpui.exe" if sys.platform == "win32" else "potato-gpui")
    )
    if not binary.is_file():
        parser.error(f"Build the executable first: {binary}")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    if sys.platform == "darwin":
        artifacts = package_macos(
            binary,
            output,
            arch,
            release_version,
            args.debug,
            args.driver_archive,
        )
    else:
        artifacts = package_windows(
            binary,
            output,
            release_version,
            args.debug,
            args.driver_archive,
        )
    for artifact in artifacts:
        with artifact.open("rb") as stream:
            checksum = hashlib.file_digest(stream, "sha256").hexdigest()
        artifact.with_name(artifact.name + ".sha256").write_text(
            f"{checksum}  {artifact.name}\n",
            encoding="utf-8",
        )
        print(artifact)


if __name__ == "__main__":
    main()
