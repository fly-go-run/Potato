"""Exercise shipped archives and Windows installation on CI."""

import argparse
import contextlib
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import subprocess
import sys
import tempfile
import zipfile

from package import ROOT, TARGETS, version
from stage_driver import VERSION as DRIVER_VERSION

INSTALL_KEY = r"Software\Potato\GPUI"
UNINSTALL_KEY = (
    r"Software\Microsoft\Windows\CurrentVersion\Uninstall\PotatoGPUI"
)


def assert_no_windows_installation(winreg, default_install):
    # Check both registry views, including custom and incomplete installs.
    for view in (winreg.KEY_WOW64_64KEY, winreg.KEY_WOW64_32KEY):
        for name in (INSTALL_KEY, UNINSTALL_KEY):
            try:
                with winreg.OpenKey(
                    winreg.HKEY_CURRENT_USER,
                    name,
                    0,
                    winreg.KEY_READ | view,
                ):
                    raise RuntimeError(
                        f"Existing installation registry: {name}",
                    )
            except FileNotFoundError:
                pass
    if default_install.exists():
        raise RuntimeError(
            f"Refusing to replace an existing installation: {default_install}",
        )


@contextlib.contextmanager
def windows_read_lock(path):
    """Simulate an in-use EXE without starting the GPU or driver daemon."""
    import ctypes
    from ctypes import wintypes

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.CreateFileW.argtypes = [
        wintypes.LPCWSTR,
        wintypes.DWORD,
        wintypes.DWORD,
        wintypes.LPVOID,
        wintypes.DWORD,
        wintypes.DWORD,
        wintypes.HANDLE,
    ]
    kernel.CreateFileW.restype = wintypes.HANDLE
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.CloseHandle.restype = wintypes.BOOL
    handle = kernel.CreateFileW(str(path), 0x80000000, 1, None, 3, 0, None)
    if handle == wintypes.HANDLE(-1).value:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        yield
    finally:
        kernel.CloseHandle(handle)


def run(command, *, timeout=120, env=None):
    result = subprocess.run(
        command,
        check=False,
        timeout=timeout,
        env=env,
        capture_output=True,
        text=True,
        errors="replace",
    )
    print(result.stdout, end="")
    print(result.stderr, end="", file=sys.stderr)
    result.check_returncode()
    return result


def smoke(binary, data, logs, label, expected):
    report = logs / f"{label}.json"
    report.unlink(missing_ok=True)
    env = dict(os.environ, POTATO_NATIVE_DATA_DIR=str(data))
    if sys.platform == "win32":
        driver = binary.parent / "computer-driver/cua-driver.exe"
        for executable in (binary, driver):
            imports = run(["dumpbin", "/dependents", str(executable)]).stdout
            (logs / f"{label}-{executable.stem}-imports.log").write_text(
                imports,
                encoding="utf-8",
            )
            if re.search(
                r"(?:VCRUNTIME|MSVCP|MSVCR)\d+.*\.dll",
                imports,
                re.IGNORECASE,
            ):
                raise RuntimeError(
                    f"{executable.name} requires a Visual C++ redistributable",
                )
        if (
            run([str(driver), "--version"]).stdout.strip()
            != f"cua-driver {DRIVER_VERSION}"
        ):
            raise RuntimeError("Unexpected computer driver version")
    # A clean working directory catches dependencies on checkout files.
    result = subprocess.run(
        [str(binary), "--startup-smoke", str(report)],
        cwd=data.parent,
        env=env,
        capture_output=True,
        text=True,
        errors="replace",
        timeout=45,
        check=False,
    )
    (logs / f"{label}.stdout.log").write_text(result.stdout, encoding="utf-8")
    (logs / f"{label}.stderr.log").write_text(result.stderr, encoding="utf-8")
    result.check_returncode()
    if not report.is_file():
        raise RuntimeError(
            f"{label}: executable did not produce a startup report",
        )
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
    expected = {
        "version": release_version,
        "os": "macos",
        "arch": "aarch64" if arch == "arm64" else arch,
        "computer_driver_available": True,
        "computer_driver_version": DRIVER_VERSION,
    }

    def check_bundle(bundle, label):
        binary = bundle / "Contents/MacOS/potato-gpui"
        run(["lipo", str(binary), "-verify_arch", arch])
        driver = bundle / "Contents/Resources/computer-driver/cua-driver"
        run(["lipo", str(driver), "-verify_arch", arch])
        if (
            run([str(driver), "--version"]).stdout.strip()
            != f"cua-driver {DRIVER_VERSION}"
        ):
            raise RuntimeError("Unexpected computer driver version")
        run(["codesign", "--verify", "--deep", "--strict", str(bundle)])
        with (bundle / "Contents/Info.plist").open("rb") as stream:
            info = plistlib.load(stream)
        if info["CFBundleShortVersionString"] != release_version.split("-")[0]:
            raise RuntimeError("Bundle version differs from release version")
        smoke(binary, work / f"{label} data", logs, label, expected)

    check_bundle(extracted / "Potato GPUI.app", "macos-zip")
    mount = work / "mounted dmg"
    run(
        [
            "hdiutil",
            "attach",
            "-readonly",
            "-nobrowse",
            "-mountpoint",
            str(mount),
            str(output / f"{stem}.dmg"),
        ],
    )
    try:
        if not (mount / "Applications").is_symlink():
            raise RuntimeError("DMG is missing its Applications link")
        check_bundle(mount / "Potato GPUI.app", "macos-dmg")
    finally:
        run(["hdiutil", "detach", str(mount)])


def verify_windows_registration(winreg, installed, release_version):
    with winreg.OpenKey(
        winreg.HKEY_CURRENT_USER,
        UNINSTALL_KEY,
        0,
        winreg.KEY_READ | winreg.KEY_WOW64_64KEY,
    ) as key:
        if winreg.QueryValueEx(key, "DisplayVersion")[0] != release_version:
            raise RuntimeError(
                "Installed version differs from release version",
            )
        if Path(winreg.QueryValueEx(key, "InstallLocation")[0]) != installed:
            raise RuntimeError("Installer ignored the custom destination")
        for name, expected_command in (
            ("UninstallString", f'"{installed / "Uninstall.exe"}"'),
            (
                "QuietUninstallString",
                f'"{installed / "Uninstall.exe"}" /S',
            ),
        ):
            if winreg.QueryValueEx(key, name)[0] != expected_command:
                raise RuntimeError(
                    f"Invalid registry uninstall command: {name}",
                )


def verify_windows(output, work, logs, release_version):
    import winreg

    assert_no_windows_installation(
        winreg,
        Path(os.environ["LOCALAPPDATA"]) / "Programs/Potato GPUI",
    )
    stem = f"Potato-GPUI-{release_version}-Windows-x86_64"
    expected = {
        "version": release_version,
        "os": "windows",
        "arch": "x86_64",
        "computer_driver_available": True,
        "computer_driver_version": DRIVER_VERSION,
    }
    portable = work / "portable archive"
    with zipfile.ZipFile(output / f"{stem}-portable.zip") as archive:
        archive.extractall(portable)
    smoke(
        portable / "Potato GPUI/Potato-GPUI.exe",
        work / "portable data",
        logs,
        "windows-portable",
        expected,
    )
    installed = work / "自定义安装 Potato GPUI"
    data = work / "用户数据 installed data"
    setup = output / f"{stem}-setup.exe"
    try:
        # NSIS /D=, like _?=, consumes the remaining *unquoted* command line.
        run(f'"{setup}" /S /D={installed}')
        verify_windows_registration(winreg, installed, release_version)
        smoke(
            installed / "Potato-GPUI.exe",
            data,
            logs,
            "windows-installed",
            expected,
        )
        before = {
            p.relative_to(installed): hashlib.sha256(
                p.read_bytes(),
            ).hexdigest()
            for p in installed.rglob("*")
            if p.is_file()
        }
        for name in ("Potato-GPUI.exe", "computer-driver/cua-driver.exe"):
            with windows_read_lock(installed / name):
                for command in (
                    [str(setup), "/S"],
                    f'"{installed / "Uninstall.exe"}" /S _?={installed}',
                ):
                    result = subprocess.run(
                        command,
                        capture_output=True,
                        timeout=30,
                        check=False,
                    )
                    if result.returncode == 0:
                        raise RuntimeError(
                            f"Accepted a locked file: {name}",
                        )
            after = {
                p.relative_to(installed): hashlib.sha256(
                    p.read_bytes(),
                ).hexdigest()
                for p in installed.rglob("*")
                if p.is_file()
            }
            if before != after:
                raise RuntimeError(
                    "Blocked installation/uninstallation changed the payload",
                )
        # Run again against the existing DB, including an in-place upgrade.
        # No /D=: verify the 64-bit registration restores the custom path.
        run([str(setup), "/S"])
        smoke(
            installed / "Potato-GPUI.exe",
            data,
            logs,
            "windows-upgraded",
            expected,
        )
    finally:
        uninstaller = installed / "Uninstall.exe"
        if uninstaller.is_file():
            # NSIS _?= must be last and unquoted, even if its path has spaces.
            # This prevents its normal async temp-copy mode, so CI can wait.
            run(f'"{uninstaller}" /S _?={installed}')
    if (installed / "computer-driver").exists():
        raise RuntimeError("Uninstall left the computer driver behind")
    if (installed / "Potato-GPUI.exe").exists():
        raise RuntimeError("Uninstall left the application executable behind")
    assert_no_windows_installation(
        winreg,
        Path(os.environ["LOCALAPPDATA"]) / "Programs/Potato GPUI",
    )
    if not (data / "potato.sqlite3").is_file():
        raise RuntimeError("Uninstall removed user data")
    (logs / "windows-installation.json").write_text(
        json.dumps(
            {
                "install": True,
                "upgrade": True,
                "uninstall": True,
                "retained_data": True,
                "custom_unicode_path": True,
                "in_use_rejected_without_changes": True,
            },
        ),
        encoding="utf-8",
    )


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
    suffixes = (
        ["portable.zip", "setup.exe"] if system == "win32" else ["zip", "dmg"]
    )
    platform_name = "Windows-x86_64" if system == "win32" else f"macOS-{arch}"
    for suffix in suffixes:
        separator = "-" if system == "win32" else "."
        artifact = output / (
            f"Potato-GPUI-{release_version}-{platform_name}"
            f"{separator}{suffix}"
        )
        with artifact.open("rb") as stream:
            checksum = hashlib.file_digest(stream, "sha256").hexdigest()
        expected = f"{checksum}  {artifact.name}\n"
        if (
            artifact.with_name(artifact.name + ".sha256").read_text()
            != expected
        ):
            raise RuntimeError(f"Checksum mismatch: {artifact.name}")
    with tempfile.TemporaryDirectory(
        prefix="potato package verify ",
    ) as temporary:
        work = Path(temporary)
        if system == "darwin":
            verify_macos(output, work, logs, arch, release_version)
        else:
            verify_windows(output, work, logs, release_version)


if __name__ == "__main__":
    main()
