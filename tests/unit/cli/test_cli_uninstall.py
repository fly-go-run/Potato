# -*- coding: utf-8 -*-
from __future__ import annotations

from pathlib import Path

from potato.cli.uninstall_cmd import _remove_path_entry


def test_remove_path_entry_removes_current_and_legacy_installers(
    tmp_path,
) -> None:
    profile = tmp_path / ".zshrc"
    profile.write_text(
        "before\n"
        "# QwenPaw\n"
        'export PATH="$HOME/.qwenpaw/bin:$PATH"\n'
        "middle\n"
        "# Potato\n"
        f'export PATH="{Path.home()}/.potato/bin:$PATH"\n'
        "# CoPaw\n"
        'export PATH="$HOME/.copaw/bin:$PATH"\n'
        "after\n",
    )

    assert _remove_path_entry(profile) is True
    assert profile.read_text() == "before\nmiddle\nafter\n"


def test_remove_path_entry_preserves_unmanaged_profile_content(tmp_path) -> None:
    profile = tmp_path / ".zshrc"
    profile.write_text('export PATH="$HOME/bin:$PATH"\n')

    assert _remove_path_entry(profile) is False
    assert profile.read_text() == 'export PATH="$HOME/bin:$PATH"\n'
