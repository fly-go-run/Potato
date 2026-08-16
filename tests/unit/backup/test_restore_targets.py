# -*- coding: utf-8 -*-
"""Tests for restore target preflight checks."""
# pylint: disable=protected-access
from __future__ import annotations

from pathlib import Path

import pytest

from potato.backup._ops import restore
from potato.backup.models import BackupValidationError


def test_busy_restore_target_reports_user_actionable_error(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    target = tmp_path / "workspace"
    target.mkdir()

    def fake_assert_directory_renamable(_target: Path) -> None:
        raise PermissionError("locked")

    def fake_find_busy_restore_paths(_target: Path) -> list[Path]:
        return [target / "browser"]

    monkeypatch.setattr(
        restore,
        "assert_directory_renamable",
        fake_assert_directory_renamable,
    )
    monkeypatch.setattr(
        restore,
        "find_busy_restore_paths",
        fake_find_busy_restore_paths,
    )

    with pytest.raises(BackupValidationError) as exc_info:
        restore._assert_restore_targets_available([target])

    assert exc_info.value.code == "restore_target_busy"
    assert exc_info.value.details["locked_paths"] == [
        str(target / "browser"),
    ]


def test_directory_commit_failure_does_not_publish_staged_config(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    first = tmp_path / "first"
    second = tmp_path / "second"
    config_tmp = tmp_path / "config.json.tmp"
    config_tmp.write_text('{"restored": true}', encoding="utf-8")
    discarded: list[Path] = []
    config_committed = False

    def fail_first_commit(_target: Path) -> None:
        raise PermissionError("locked")

    def record_discard(target: Path) -> None:
        discarded.append(target)

    def record_config_commit(_staged: Path | None) -> None:
        nonlocal config_committed
        config_committed = True

    monkeypatch.setattr(restore, "commit_tmp", fail_first_commit)
    monkeypatch.setattr(restore, "discard_tmp", record_discard)
    monkeypatch.setattr(
        restore,
        "_commit_staged_global_config",
        record_config_commit,
    )

    with pytest.raises(PermissionError, match="locked"):
        restore._commit_and_finalize(
            [first, second],
            config_tmp,
            {},
            [],
            "backup-1",
        )

    assert config_committed is False
    assert config_tmp.exists() is False
    assert discarded == [first, second]
