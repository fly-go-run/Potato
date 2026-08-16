# -*- coding: utf-8 -*-
"""Brand-migration tests for file-backed loop state."""

from pathlib import Path

from potato.loop.gates.file_loop_gate import FileLoopGate


def test_file_loop_prefers_current_potato_state(tmp_path: Path) -> None:
    current = tmp_path / ".potato" / "loop_state" / "session-1"
    legacy = tmp_path / ".qwenpaw" / "loop_state" / "session-1"
    current.mkdir(parents=True)
    legacy.mkdir(parents=True)

    assert FileLoopGate._build_state_dir(tmp_path, "session-1") == current


def test_file_loop_reuses_legacy_qwenpaw_state(tmp_path: Path) -> None:
    legacy = tmp_path / ".qwenpaw" / "loop_state" / "session-1"
    legacy.mkdir(parents=True)

    assert FileLoopGate._build_state_dir(tmp_path, "session-1") == legacy


def test_file_loop_uses_new_potato_path_for_new_session(tmp_path: Path) -> None:
    expected = tmp_path / ".potato" / "loop_state" / "session-1"

    assert FileLoopGate._build_state_dir(tmp_path, "session-1") == expected
