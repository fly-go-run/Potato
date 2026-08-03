# -*- coding: utf-8 -*-
from pathlib import Path

from qwenpaw.utils import console_static


def _write_index(directory: Path) -> None:
    directory.mkdir(parents=True)
    (directory / "index.html").write_text("<!doctype html>", encoding="utf-8")


def test_default_static_candidates_prefer_app_before_legacy_console(
    tmp_path: Path,
) -> None:
    package_dir = tmp_path / "site-packages" / "qwenpaw"
    repo_dir = tmp_path / "repo"
    cwd = tmp_path / "cwd"

    candidates = console_static._default_static_candidates(
        package_dir=package_dir,
        repo_dir=repo_dir,
        cwd=cwd,
    )

    assert candidates == (
        repo_dir / "app" / "dist",
        package_dir / "console",
        cwd / "app" / "dist",
    )


def test_web_static_env_override_wins_over_legacy_console_override(
    monkeypatch,
    tmp_path: Path,
) -> None:
    web_override = tmp_path / "web"
    legacy_override = tmp_path / "legacy-console"
    _write_index(web_override)
    _write_index(legacy_override)

    monkeypatch.setattr(
        console_static.EnvVarLoader,
        "get_str",
        lambda key: (
            str(web_override)
            if key == console_static.WEB_STATIC_ENV
            else str(legacy_override)
            if key == console_static.CONSOLE_STATIC_ENV
            else ""
        ),
    )

    assert console_static.resolve_web_static_dir() == str(web_override)
