# -*- coding: utf-8 -*-
"""Locate or fetch the cua-driver binary Potato ships.

Packaged desktop builds embed the official release. Unpackaged runs
download the same tarball into the user data dir on first use. Users
never install CuaDriver.app.
"""

from __future__ import annotations

import hashlib
import logging
import os
import platform
import shutil
import stat
import tarfile
import tempfile
import urllib.request
import zipfile
from pathlib import Path

logger = logging.getLogger(__name__)

# Official Cua Driver release Potato vendors. Bump with a pack-time restage.
CUA_DRIVER_VERSION = "0.20.0"
GITHUB_REPO = "trycua/cua"
HOST_BUNDLE_ID = "io.agentscope.qwenpaw.desktop"

# Official SHA-256 of the archives Potato downloads (cua-driver-rs-v0.20.0).
ARCHIVE_SHA256 = {
    "cua-driver-rs-0.20.0-darwin-universal.tar.gz": (
        "d5e61fecebd9a620e50c2b8b608c8e7e8141f74c6faebc2ae9ef5d0d96cce7b8"
    ),
    "cua-driver-rs-0.20.0-linux-x86_64-binary.tar.gz": (
        "00816b855886743e1acc92ea26a3ea82be216252c1fb89a1fb20aa61f17db963"
    ),
    "cua-driver-rs-0.20.0-linux-arm64-binary.tar.gz": (
        "f3c42d8b04549d1bd2a68ceee6c84519b176943acda7cb1343b06e59cd2ba231"
    ),
    "cua-driver-rs-0.20.0-windows-x86_64-binary.zip": (
        "c020fefee01aacc174a27fea84a0cb77d47ef8290bfc772b3db7e3e06670d2b2"
    ),
    "cua-driver-rs-0.20.0-windows-arm64-binary.zip": (
        "e8d47cb35c7a719f12c3012caa12f1166ecd1614548afe9b18a2c600116f8bde"
    ),
}

# GitHub publishes archive digests for 0.20.0, but not hashes of the
# extracted executables. Keep this empty until an executable digest is
# obtained from an official release source. An unpinned staged binary is
# never reused; the verified archive is downloaded and extracted again.
BINARY_SHA256: dict[str, str] = {}


def cua_driver_tag(version: str = CUA_DRIVER_VERSION) -> str:
    return f"cua-driver-rs-v{version}"


def cua_driver_platform() -> tuple[str, str]:
    """Return (os_label, archive_name) for the current host."""
    system = platform.system()
    machine = platform.machine().lower()
    if system == "Darwin":
        return "darwin", f"cua-driver-rs-{CUA_DRIVER_VERSION}-darwin-universal.tar.gz"
    if system == "Windows":
        label = "windows-arm64" if machine in {"arm64", "aarch64"} else "windows-x86_64"
        return "windows", f"cua-driver-rs-{CUA_DRIVER_VERSION}-{label}-binary.zip"
    if system == "Linux":
        label = "linux-arm64" if machine in {"aarch64", "arm64"} else "linux-x86_64"
        return "linux", f"cua-driver-rs-{CUA_DRIVER_VERSION}-{label}-binary.tar.gz"
    raise RuntimeError(f"unsupported platform for cua-driver: {system}/{machine}")


def cua_driver_download_url(version: str = CUA_DRIVER_VERSION) -> str:
    _os_label, archive = cua_driver_platform()
    if version != CUA_DRIVER_VERSION:
        archive = archive.replace(CUA_DRIVER_VERSION, version)
    return (
        f"https://github.com/{GITHUB_REPO}/releases/download/"
        f"{cua_driver_tag(version)}/{archive}"
    )


def driver_executable_name() -> str:
    return "cua-driver.exe" if platform.system() == "Windows" else "cua-driver"


def runtime_home() -> Path:
    from ..constant import WORKING_DIR

    return Path(WORKING_DIR) / "runtime" / "cua-driver"


def cached_driver_path(version: str = CUA_DRIVER_VERSION) -> Path:
    return runtime_home() / version / driver_executable_name()


def daemon_socket_path() -> str:
    if platform.system() == "Windows":
        return rf"\\.\pipe\potato-cua-driver-{os.getpid()}"
    path = runtime_home() / f"potato-{os.getpid()}.sock"
    path.parent.mkdir(parents=True, exist_ok=True)
    return str(path)


def resolve_cua_driver_binary(explicit: str = "") -> str:
    """Find an already-present binary. Does not download."""
    if explicit:
        path = Path(explicit).expanduser()
        if _is_exec(path):
            return str(path)
    for env_name in (
        "POTATO_DESKTOP_CUA_DRIVER",
        "POTATO_CUA_DRIVER",
        "CUA_DRIVER_CMD",
    ):
        value = (os.environ.get(env_name) or "").strip()
        if value and _is_exec(Path(value).expanduser()):
            return str(Path(value).expanduser())
    cached = cached_driver_path()
    if _is_exec(cached) and cached_binary_is_trusted(cached):
        return str(cached)
    if os.environ.get("POTATO_CUA_ALLOW_SYSTEM") == "1":
        found = shutil.which("cua-driver")
        if found:
            return found
    return ""


def ensure_driver_binary(explicit: str = "") -> str:
    """Return a usable binary, downloading the official release if needed."""
    existing = resolve_cua_driver_binary(explicit)
    if existing:
        return existing
    dest = cached_driver_path()
    dest.parent.mkdir(parents=True, exist_ok=True)
    logger.info("Fetching built-in cua-driver %s into %s", CUA_DRIVER_VERSION, dest)
    _download_official_release(dest)
    if not _is_exec(dest):
        raise RuntimeError(f"cua-driver download did not produce {dest}")
    write_cached_digest(dest)
    return str(dest)


def extract_official_archive(archive: Path, dest_binary: Path) -> None:
    """Extract the official GitHub release archive to *dest_binary*."""
    dest_binary.parent.mkdir(parents=True, exist_ok=True)
    name = dest_binary.name
    # Downloads intentionally use a neutral temporary name
    # (``driver-archive``), so detect ZIP from its signature instead of
    # relying on a suffix.  Windows release assets are ZIP files.
    if zipfile.is_zipfile(archive):
        with zipfile.ZipFile(archive) as zip_file:
            member = _pick_archive_member(zip_file.namelist(), name)
            dest_binary.write_bytes(zip_file.read(member))
    else:
        with tarfile.open(archive, "r:*") as tar:
            member_name = _pick_archive_member(tar.getnames(), name)
            extracted = tar.extractfile(member_name)
            if extracted is None:
                raise RuntimeError(f"{member_name} is not a file in {archive}")
            dest_binary.write_bytes(extracted.read())
    dest_binary.chmod(dest_binary.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


def _download_official_release(dest_binary: Path) -> None:
    url = cua_driver_download_url()
    request = urllib.request.Request(url, headers={"User-Agent": "potato-computer-use"})
    with tempfile.TemporaryDirectory(prefix="potato-cua-") as tmp:
        archive = Path(tmp) / "driver-archive"
        with urllib.request.urlopen(request, timeout=120) as response:
            archive.write_bytes(response.read())
        verify_archive_digest(archive, cua_driver_platform()[1])
        extract_official_archive(archive, dest_binary)


def verify_archive_digest(archive: Path, archive_name: str) -> None:
    expected = ARCHIVE_SHA256.get(archive_name)
    if not expected:
        raise RuntimeError(f"no pinned digest for {archive_name}")
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    if digest != expected:
        raise RuntimeError(
            f"cua-driver archive hash mismatch for {archive_name}: "
            f"got {digest}, expected {expected}",
        )


def binary_digest_is_pinned(binary: Path, archive_name: str) -> bool:
    """Return true only when *binary* matches an official executable pin."""
    expected = BINARY_SHA256.get(archive_name)
    if not expected or not binary.is_file():
        return False
    try:
        digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    except OSError:
        return False
    return digest == expected


def cached_digest_path(binary: Path) -> Path:
    return binary.with_name(binary.name + ".sha256")


def write_cached_digest(binary: Path) -> None:
    """Record the hash of a just-extracted, archive-verified binary."""
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    cached_digest_path(binary).write_text(digest + "\n", encoding="utf-8")


def cached_binary_is_trusted(binary: Path) -> bool:
    """True only when *binary* still matches the sidecar written after extract."""
    sidecar = cached_digest_path(binary)
    if not binary.is_file() or not sidecar.is_file():
        return False
    try:
        expected = sidecar.read_text(encoding="utf-8").strip()
        actual = hashlib.sha256(binary.read_bytes()).hexdigest()
    except OSError:
        return False
    return bool(expected) and expected == actual


def _pick_archive_member(names: list[str], filename: str) -> str:
    matches = [
        name
        for name in names
        if Path(name).name == filename and not name.endswith("/")
    ]
    if not matches:
        raise RuntimeError(f"archive has no {filename}; entries={names[:12]}")
    # Prefer a top-level or Contents/MacOS copy over random nested copies.
    matches.sort(key=lambda name: ("Contents/MacOS" not in name, name.count("/")))
    return matches[0]


def _is_exec(path: Path) -> bool:
    try:
        return path.is_file() and os.access(path, os.X_OK)
    except OSError:
        return False
