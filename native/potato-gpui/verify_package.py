"""Exercise shipped archives and the Windows install/uninstall lifecycle on CI."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
import zipfile

from package import ROOT, TARGETS, version


def run(command, *, timeout=120, env=None):
    result = subprocess.run(command, check=False, timeout=timeout, env=env,
                            capture_output=True, text=True, errors="replace")
    print(result.stdout, end="")
    print(result.stderr, end="", file=sys.stderr)
    result.check_returncode()
    return result


def smoke(binary, data, logs, label, expected):
    report = logs / f"{label}.json"
    report.unlink(missing_ok=True)
    env = dict(os.environ, POTATO_NATIVE_DATA_DIR=str(data))
    # A clean working directory catches accidental dependencies on checkout files.
    result = subprocess.run([str(binary), "--startup-smoke", str(report)],
                            cwd=data.parent, env=env, capture_output=True,
                            text=True, errors="replace", timeout=45)
    (logs / f"{label}.stdout.log").write_text(result.stdout, encoding="utf-8")
    (logs / f"{label}.stderr.log").write_text(result.stderr, encoding="utf-8")
    result.check_returncode()
    if not report.is_file():
        raise RuntimeError(f"{label}: executable did not produce a startup report")
    actual = json.loads(report.read_text())
    if actual != {"ok": True, **expected}:
        raise RuntimeError(f"{label}: unexpected startup report: {actual}")
    if not (data / "potato.sqlite3").is_file():
        raise RuntimeError(f"{label}: core did not initialize SQLite")
    print(f"{label}: {actual}")


def verify_macos(output, work, logs, arch, release_version):
    stem = f"Potato-GPUI-{release_version}-macOS-{arch}"
    extracted = work / "extracted archive"
    run(["ditto", "-x", "-k", str(output / f"{stem}.zip"), str(extracted)])
    expected = {"version": release_version, "os": "macos", "arch": "aarch64" if arch == "arm64" else arch}

    def check_bundle(bundle, label):
        binary = bundle / "Contents/MacOS/potato-gpui"
        run(["lipo", str(binary), "-verify_arch", arch])
        run(["codesign", "--verify", "--deep", "--strict", str(bundle)])
        with (bundle / "Contents/Info.plist").open("rb") as stream:
            info = plistlib.load(stream)
        if info["CFBundleShortVersionString"] != release_version.split("-")[0]:
            raise RuntimeError("Bundle version differs from release version")
        smoke(binary, work / f"{label} data", logs, label, expected)

    check_bundle(extracted / "Potato GPUI.app", "macos-zip")
    mount = work / "mounted dmg"
    run(["hdiutil", "attach", "-readonly", "-nobrowse", "-mountpoint", str(mount), str(output / f"{stem}.dmg")])
    try:
        if not (mount / "Applications").is_symlink():
            raise RuntimeError("DMG is missing its Applications link")
        check_bundle(mount / "Potato GPUI.app", "macos-dmg")
    finally:
        run(["hdiutil", "detach", str(mount)])


def verify_windows(output, work, logs, release_version):
    import winreg

    stem = f"Potato-GPUI-{release_version}-Windows-x86_64"
    expected = {"version": release_version, "os": "windows", "arch": "x86_64"}
    portable = work / "portable archive"
    with zipfile.ZipFile(output / f"{stem}-portable.zip") as archive:
        archive.extractall(portable)
    smoke(portable / "Potato GPUI/Potato-GPUI.exe", work / "portable data", logs, "windows-portable", expected)
    installed = Path(os.environ["LOCALAPPDATA"]) / "Programs/Potato GPUI"
    registry = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\PotatoGPUI"
    if installed.exists():
        raise RuntimeError(f"Refusing to replace an existing installation: {installed}")
    data = work / "installed data"
    try:
        run([str(output / f"{stem}-setup.exe"), "/S"])
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, registry, 0, winreg.KEY_READ | winreg.KEY_WOW64_64KEY) as key:
            if winreg.QueryValueEx(key, "DisplayVersion")[0] != release_version:
                raise RuntimeError("Installed version differs from release version")
        smoke(installed / "Potato-GPUI.exe", data, logs, "windows-installed", expected)
        # Run again against the existing DB, including an in-place upgrade.
        run([str(output / f"{stem}-setup.exe"), "/S"])
        smoke(installed / "Potato-GPUI.exe", data, logs, "windows-upgraded", expected)
    finally:
        uninstaller = installed / "Uninstall.exe"
        if uninstaller.is_file():
            # NSIS _?= must be last and unquoted, even if its path has spaces.
            # This prevents its normal async temp-copy mode, so CI can wait.
            run(f'"{uninstaller}" /S _?={installed}')
    if (installed / "Potato-GPUI.exe").exists():
        raise RuntimeError("Uninstall left the application executable behind")
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, registry, 0, winreg.KEY_READ | winreg.KEY_WOW64_64KEY):
            raise RuntimeError("Uninstall left its registry entry behind")
    except FileNotFoundError:
        pass
    if not (data / "potato.sqlite3").is_file():
        raise RuntimeError("Uninstall removed user data")
    (logs / "windows-installation.json").write_text(json.dumps({
        "install": True, "upgrade": True, "uninstall": True, "retained_data": True,
    }), encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    args = parser.parse_args()
    system, arch = TARGETS[args.target]
    if system != sys.platform:
        parser.error("Run verification on the target operating system")
    output = args.output.resolve()
    logs = output / "verification"
    logs.mkdir(parents=True, exist_ok=True)
    release_version = version()
    suffixes = ["portable.zip", "setup.exe"] if system == "win32" else ["zip", "dmg"]
    platform_name = "Windows-x86_64" if system == "win32" else f"macOS-{arch}"
    for suffix in suffixes:
        separator = "-" if system == "win32" else "."
        artifact = output / f"Potato-GPUI-{release_version}-{platform_name}{separator}{suffix}"
        with artifact.open("rb") as stream:
            checksum = hashlib.file_digest(stream, "sha256").hexdigest()
        expected = f"{checksum}  {artifact.name}\n"
        if artifact.with_name(artifact.name + ".sha256").read_text() != expected:
            raise RuntimeError(f"Checksum mismatch: {artifact.name}")
    with tempfile.TemporaryDirectory(prefix="potato package verify ") as temporary:
        work = Path(temporary)
        if system == "darwin":
            verify_macos(output, work, logs, arch, release_version)
        else:
            verify_windows(output, work, logs, release_version)


if __name__ == "__main__":
    main()
