# -*- coding: utf-8 -*-
from __future__ import annotations

import importlib.util
import sys
from pathlib import Path


SCRIPT = (
    Path(__file__).resolve().parents[3]
    / "scripts"
    / "pack-tauri"
    / "stage_cua_driver.py"
)


def _load_stage_module():
    spec = importlib.util.spec_from_file_location("stage_cua_driver", SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_stage_does_not_reuse_binary_without_digest(
    tmp_path: Path,
    monkeypatch,
) -> None:
    stage = _load_stage_module()
    binary = tmp_path / stage.driver_executable_name()
    binary.write_bytes(b"unverified-existing-binary")
    (tmp_path / "VERSION").write_text(
        stage.CUA_DRIVER_VERSION + "\n",
        encoding="utf-8",
    )
    downloaded = False

    def _download(_url: str) -> bytes:
        nonlocal downloaded
        downloaded = True
        return b"verified-archive"

    def _extract(_archive: Path, dest: Path) -> None:
        dest.write_bytes(b"freshly-extracted-binary")

    monkeypatch.setattr(stage, "_http_get", _download)
    monkeypatch.setattr(stage, "verify_archive_digest", lambda *_args: None)
    monkeypatch.setattr(stage, "extract_official_archive", _extract)
    monkeypatch.setattr(sys, "argv", [str(SCRIPT), "--dest", str(tmp_path)])

    assert stage.main() == 0
    assert downloaded is True
    assert binary.read_bytes() == b"freshly-extracted-binary"


def test_stage_missing_version_file_does_not_crash(
    tmp_path: Path,
    monkeypatch,
) -> None:
    stage = _load_stage_module()
    binary = tmp_path / stage.driver_executable_name()
    binary.write_bytes(b"existing")
    monkeypatch.setattr(stage, "_http_get", lambda _url: b"archive")
    monkeypatch.setattr(stage, "verify_archive_digest", lambda *_args: None)
    monkeypatch.setattr(
        stage,
        "extract_official_archive",
        lambda _archive, dest: dest.write_bytes(b"fresh"),
    )
    monkeypatch.setattr(sys, "argv", [str(SCRIPT), "--dest", str(tmp_path)])

    assert stage.main() == 0
    assert (tmp_path / "VERSION").is_file()
