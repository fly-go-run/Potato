# -*- coding: utf-8 -*-
from __future__ import annotations

import logging
import sys
import time

import click

from ..utils.stdio import ensure_standard_streams

# On Windows, force UTF-8 for stdout/stderr so cron and other commands
# can handle Chinese and other non-ASCII (Linux is UTF-8 by default).
if sys.platform == "win32":
    ensure_standard_streams()
    try:
        sys.stdout.reconfigure(encoding="utf-8")
        sys.stderr.reconfigure(encoding="utf-8")
    except (AttributeError, OSError):
        pass

# pylint: disable=wrong-import-position

logger = logging.getLogger(__name__)
# Store init timings so app_cmd can re-log after setting log level to debug.
_init_timings: list[tuple[str, float]] = []
_t0_main = time.perf_counter()
_init_timings.append(("main.py loaded", 0.0))


def _record(label: str, elapsed: float) -> None:
    _init_timings.append((label, elapsed))
    logger.debug("%.3fs %s", elapsed, label)


# Timed imports below: order and placement are intentional (E402/C0413).
_t = time.perf_counter()
from ..config.utils import read_last_api  # noqa: E402

_record("..config.utils", time.perf_counter() - _t)

_t = time.perf_counter()
from ..__version__ import __version__  # noqa: E402

_record("..__version__", time.perf_counter() - _t)

_total = time.perf_counter() - _t0_main
_init_timings.append(("(total imports)", _total))
logger.debug("%.3fs (total imports)", _total)


def log_init_timings() -> None:
    """Emit init timing debug lines after setup_logger(debug) in app_cmd."""
    for label, elapsed in _init_timings:
        logger.debug("%.3fs %s", elapsed, label)


class LazyGroup(click.Group):
    """Click group that supports lazy loading of subcommands."""

    def __init__(self, *args, lazy_subcommands=None, **kwargs):
        super().__init__(*args, **kwargs)
        self.lazy_subcommands = lazy_subcommands or {}

    def list_commands(self, ctx):
        """Return all command names (both eager and lazy)."""
        base = super().list_commands(ctx)
        return sorted(set(base) | set(self.lazy_subcommands.keys()))

    def get_command(self, ctx, cmd_name):
        """Get command, loading lazily if needed."""
        # Try eager commands first
        cmd = super().get_command(ctx, cmd_name)
        if cmd is not None:
            return cmd

        # Try lazy commands
        if cmd_name in self.lazy_subcommands:
            module_path, attr_name, label = self.lazy_subcommands[cmd_name]
            _t = time.perf_counter()
            try:
                module = __import__(module_path, fromlist=[attr_name])
                cmd = getattr(module, attr_name)
                _record(label, time.perf_counter() - _t)
                # Cache for next time
                self.add_command(cmd, cmd_name)
                return cmd
            except Exception as e:
                logger.error(f"Failed to load command '{cmd_name}': {e}")
                return None

        return None


DESKTOP_ONLY_HINT = (
    "Potato is a desktop app. Open the Potato application to chat.\n"
    "This command is only for desktop packaging and diagnostics."
)


@click.group(
    cls=LazyGroup,
    invoke_without_command=True,
    context_settings={"help_option_names": ["-h", "--help"]},
    lazy_subcommands={
        "acp": ("potato.cli.acp_cmd", "acp_cmd", ".acp_cmd"),
        "app": ("potato.cli.app_cmd", "app_cmd", ".app_cmd"),
        "channels": (
            "potato.cli.channels_cmd",
            "channels_group",
            ".channels_cmd",
        ),
        "channel": (
            "potato.cli.channels_cmd",
            "channels_group",
            ".channels_cmd",
        ),
        "daemon": ("potato.cli.daemon_cmd", "daemon_group", ".daemon_cmd"),
        "chats": ("potato.cli.chats_cmd", "chats_group", ".chats_cmd"),
        "chat": ("potato.cli.chats_cmd", "chats_group", ".chats_cmd"),
        "clean": ("potato.cli.clean_cmd", "clean_cmd", ".clean_cmd"),
        "cron": ("potato.cli.cron_cmd", "cron_group", ".cron_cmd"),
        "env": ("potato.cli.env_cmd", "env_group", ".env_cmd"),
        "init": ("potato.cli.init_cmd", "init_cmd", ".init_cmd"),
        "models": (
            "potato.cli.providers_cmd",
            "models_group",
            ".providers_cmd",
        ),
        "skills": ("potato.cli.skills_cmd", "skills_group", ".skills_cmd"),
        "tui": ("potato.cli.tui.launch", "tui_cmd", ".tui.launch"),
        "uninstall": (
            "potato.cli.uninstall_cmd",
            "uninstall_cmd",
            ".uninstall_cmd",
        ),
        "desktop": ("potato.cli.desktop_cmd", "desktop_cmd", ".desktop_cmd"),
        "update": ("potato.cli.update_cmd", "update_cmd", ".update_cmd"),
        "shutdown": (
            "potato.cli.shutdown_cmd",
            "shutdown_cmd",
            ".shutdown_cmd",
        ),
        "auth": ("potato.cli.auth_cmd", "auth_group", ".auth_cmd"),
        "agents": ("potato.cli.agents_cmd", "agents_group", ".agents_cmd"),
        "agent": ("potato.cli.agents_cmd", "agents_group", ".agents_cmd"),
        "plugin": (
            "potato.cli.plugin_commands",
            "plugin",
            ".plugin_commands",
        ),
        "task": ("potato.cli.task_cmd", "task_cmd", ".task_cmd"),
        "doctor": ("potato.cli.doctor_cmd", "doctor_cmd", ".doctor_cmd"),
        "auto": ("potato.cli.auto", "auto_group", ".auto"),
    },
)
@click.version_option(version=__version__, prog_name="Potato")
@click.option("--host", default=None, help="API Host")
@click.option(
    "--port",
    default=None,
    type=int,
    help="API Port",
)
@click.pass_context
def cli(ctx: click.Context, host: str | None, port: int | None) -> None:
    """Potato CLI."""
    # default from last run if not provided
    last = read_last_api()
    if host is None or port is None:
        if last:
            host = host or last[0]
            port = port or last[1]

    # final fallback
    host = host or "127.0.0.1"
    port = port or 8088

    ctx.ensure_object(dict)
    ctx.obj["host"] = host
    ctx.obj["port"] = port

    # Bare ``potato`` used to open the terminal chat UI. Potato is desktop-only
    # now, so a command with no subcommand prints a pointer and help.
    if ctx.invoked_subcommand is None:
        click.echo(DESKTOP_ONLY_HINT)
        click.echo()
        click.echo(ctx.get_help())
