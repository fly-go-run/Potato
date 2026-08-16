# -*- coding: utf-8 -*-
"""potato uninstall — remove the Potato environment and CLI wrapper."""
from __future__ import annotations

import shutil
import re
from pathlib import Path

import click

from ..constant import WORKING_DIR


# Directories created by the installer (relative to WORKING_DIR).
_INSTALLER_DIRS = ("venv", "bin")

# Shell profiles to clean up.
_SHELL_PROFILES = (
    Path.home() / ".zshrc",
    Path.home() / ".bashrc",
    Path.home() / ".bash_profile",
)


def _remove_path_entry(profile: Path) -> bool:
    """
    Remove current and legacy installer PATH lines from a shell profile.

    Existing users may still have a QwenPaw or CoPaw marker even though the
    resolved working directory is reused by Potato.
    """
    if not profile.is_file():
        return False

    text = profile.read_text()
    # Remove installer-managed comment + PATH pairs for every product name.
    home_pattern = rf"(?:\$HOME|{re.escape(str(Path.home()))})"
    cleaned = re.sub(
        r"(?m)^# (?:Potato|QwenPaw|CoPaw)\n"
        rf"export PATH=\"{home_pattern}/"
        r"\.(?:potato|qwenpaw|copaw)/bin:\$PATH\"\n?",
        "",
        text,
    )
    if cleaned == text:
        return False

    profile.write_text(cleaned)
    return True


@click.command("uninstall")
@click.option(
    "--purge",
    is_flag=True,
    help="Also remove all data (config, chats, models, etc.)",
)
@click.option("--yes", is_flag=True, help="Do not prompt for confirmation")
def uninstall_cmd(purge: bool, yes: bool) -> None:
    """Remove Potato environment, CLI wrapper, and shell PATH entries."""
    wd = WORKING_DIR

    if purge:
        click.echo(f"This will remove ALL Potato data in {wd}")
    else:
        click.echo(
            "This will remove the Potato Python environment and CLI wrapper.",
        )
        click.echo(f"Your configuration and data in {wd} will be preserved.")

    if not yes:
        ok = click.confirm("Continue?", default=False)
        if not ok:
            click.echo("Cancelled.")
            return

    # Remove installer-managed directories
    for dirname in _INSTALLER_DIRS:
        d = wd / dirname
        if d.exists():
            shutil.rmtree(d)
            click.echo(f"  Removed {d}")

    # Purge everything if requested
    if purge and wd.exists():
        shutil.rmtree(wd)
        click.echo(f"  Removed {wd}")

    # Clean shell profiles
    for profile in _SHELL_PROFILES:
        if _remove_path_entry(profile):
            click.echo(f"  Cleaned {profile}")

    click.echo("")
    click.echo("Potato uninstalled. Please restart your terminal.")
