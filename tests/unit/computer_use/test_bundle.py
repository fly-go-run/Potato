# -*- coding: utf-8 -*-
from __future__ import annotations

import tarfile
from pathlib import Path

import os

import pytest

from potato.computer_use.bundle import (
    CUA_DRIVER_VERSION,
    cua_driver_download_url,
    daemon_socket_path,
    extract_official_archive,
    resolve_cua_driver_binary,
    verify_archive_digest,
    write_cached_digest,
)


def test_download_url_is_official_github_release() -> None:
    url = cua_driver_download_url()
    assert "github.com/trycua/cua/releases/download/" in url
    assert f"cua-driver-rs-v{CUA_DRIVER_VERSION}" in url
    assert CUA_DRIVER_VERSION in url


def test_resolve_ignores_system_app_unless_explicit(tmp_path, monkeypatch) -> None:
    monkeypatch.delenv("POTATO_DESKTOP_CUA_DRIVER", raising=False)
    monkeypatch.delenv("POTATO_CUA_DRIVER", raising=False)
    monkeypatch.delenv("CUA_DRIVER_CMD", raising=False)
    monkeypatch.delenv("POTATO_CUA_ALLOW_SYSTEM", raising=False)
    monkeypatch.setattr(
        "potato.computer_use.bundle.cached_driver_path",
        lambda version=None: tmp_path / "missing",
    )
    assert resolve_cua_driver_binary() == ""
    assert "/Applications/CuaDriver.app" not in resolve_cua_driver_binary()


def test_extract_official_tarball(tmp_path: Path) -> None:
    archive = tmp_path / "driver.tar.gz"
    payload = tmp_path / "payload"
    payload.mkdir()
    nested = payload / "cua-driver-rs-0.20.0-darwin-universal"
    nested.mkdir()
    binary = nested / "cua-driver"
    binary.write_bytes(b"#!/bin/sh\n")
    with tarfile.open(archive, "w:gz") as tar:
        tar.add(nested, arcname=nested.name)
    dest = tmp_path / "out" / "cua-driver"
    extract_official_archive(archive, dest)
    assert dest.is_file()
    assert dest.stat().st_mode & 0o111


def test_verify_archive_digest_rejects_mismatch(tmp_path: Path) -> None:
    name = "cua-driver-rs-0.20.0-darwin-universal.tar.gz"
    archive = tmp_path / name
    archive.write_bytes(b"not-the-official-archive")
    with pytest.raises(RuntimeError, match="hash mismatch"):
        verify_archive_digest(archive, name)


def test_daemon_socket_is_private_to_this_process() -> None:
    path = daemon_socket_path()
    assert str(os.getpid()) in path


def _isolate_resolve(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.delenv("POTATO_DESKTOP_CUA_DRIVER", raising=False)
    monkeypatch.delenv("POTATO_CUA_DRIVER", raising=False)
    monkeypatch.delenv("CUA_DRIVER_CMD", raising=False)
    monkeypatch.delenv("POTATO_CUA_ALLOW_SYSTEM", raising=False)
    monkeypatch.setattr(
        "potato.computer_use.bundle.cached_driver_path",
        lambda version=None: tmp_path / "cua-driver",
    )


def test_cached_binary_without_digest_is_ignored(
    tmp_path: Path,
    monkeypatch,
) -> None:
    _isolate_resolve(tmp_path, monkeypatch)
    binary = tmp_path / "cua-driver"
    binary.write_bytes(b"#!/bin/sh\n")
    binary.chmod(0o755)
    assert resolve_cua_driver_binary() == ""


def test_cached_binary_with_matching_digest_is_used(
    tmp_path: Path,
    monkeypatch,
) -> None:
    _isolate_resolve(tmp_path, monkeypatch)
    binary = tmp_path / "cua-driver"
    binary.write_bytes(b"#!/bin/sh\n")
    binary.chmod(0o755)
    write_cached_digest(binary)
    assert resolve_cua_driver_binary() == str(binary)


def test_cached_binary_with_tampered_file_is_ignored(
    tmp_path: Path,
    monkeypatch,
) -> None:
    _isolate_resolve(tmp_path, monkeypatch)
    binary = tmp_path / "cua-driver"
    binary.write_bytes(b"#!/bin/sh\n")
    binary.chmod(0o755)
    write_cached_digest(binary)
    binary.write_bytes(b"tampered")
    binary.chmod(0o755)
    assert resolve_cua_driver_binary() == ""
