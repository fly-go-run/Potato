# -*- coding: utf-8 -*-
"""Potato Pet backend plugin entry point."""

# pylint: disable=wrong-import-position,wrong-import-order

import atexit
import logging
import os
import sys
from pathlib import Path

# ``potato plugin install`` execs this file as a plain module (no
# package), so sibling modules are not reachable via relative imports
# unless the plugin directory is on sys.path before importing them.
_plugin_dir = str(Path(__file__).resolve().parent)
if _plugin_dir not in sys.path:
    sys.path.insert(0, _plugin_dir)

# Evict stale cached sibling modules so hot-reload after plugin
# reinstall picks up the new files instead of the previous version
# lingering in ``sys.modules``.
for _sib in (
    "emitter",
    "patch_runner",
    "patch_approval",
    "router",
    "pet_paths",
):
    _cached = sys.modules.get(_sib)
    if _cached is None:
        continue
    _cached_file = getattr(_cached, "__file__", None) or ""
    if _cached_file and os.path.realpath(_cached_file).startswith(
        os.path.realpath(_plugin_dir) + os.sep,
    ):
        del sys.modules[_sib]

from potato.plugins.api import PluginApi  # noqa: E402

from emitter import (  # noqa: E402
    emit_pet_event,
    ensure_desktop_available,
    stop_desktop,
)
from patch_approval import (  # noqa: E402
    patch_approval_service,
    restore_approval_service,
)
from patch_runner import (  # noqa: E402
    patch_agent_runner,
    restore_agent_runner,
)
from router import build_router  # noqa: E402

# Logger uses ``potato.*`` so messages appear in the project logger
# (``~/.potato/potato.log``).
logger = logging.getLogger("potato.pet_desktop")


def _atexit_stop_pet_desktop() -> None:
    """Best-effort stop when the interpreter exits without lifespan hooks."""
    try:
        stop_desktop(force=True, aggressive=True, grace=5.0)
    except Exception:
        logger.debug(
            "Potato Pet: atexit stop skipped or failed",
            exc_info=True,
        )


class PotatoPetPlugin:
    """Emit Potato backend lifecycle events to the desktop pet."""

    def register(self, api: PluginApi):
        """Register startup/shutdown hooks and plugin HTTP routes."""
        logger.info("Registering Potato Pet plugin")

        # Runtime patches (``AgentRunner`` / ``ApprovalService``) are
        # applied exclusively from the startup hook below — applying them
        # here at import time runs before Potato has finished wiring up
        # the affected classes and silently swallowing the import error
        # would leave the plugin in a broken state.
        api.register_startup_hook(
            hook_name="potato_pet_startup",
            callback=self._startup,
            priority=80,
        )
        api.register_shutdown_hook(
            hook_name="potato_pet_shutdown",
            callback=self._shutdown,
            priority=120,
        )
        api.register_http_router(
            build_router(),
            prefix="/potato-pet",
            tags=["potato-pet"],
        )

        atexit.register(_atexit_stop_pet_desktop)

        logger.info("Potato Pet plugin registered")

    def _startup(self):
        """Patch the runner and notify the desktop.

        Patch failures (e.g. an upstream rename of ``AgentRunner`` /
        ``ApprovalService``) surface as ``logger.exception`` so the
        plugin install system can flag them; we still attempt to keep
        the desktop autostart and ``potato.startup`` emit going so the
        UI is never silently dead.
        """
        try:
            patch_agent_runner()
        except Exception:
            logger.exception(
                "Potato Pet: failed to patch AgentRunner; "
                "lifecycle events will be unavailable",
            )
        try:
            patch_approval_service()
        except Exception:
            logger.exception(
                "Potato Pet: failed to patch ApprovalService; "
                "approval events will be unavailable",
            )

        try:
            ensure_desktop_available()
            emit_pet_event(
                "potato.startup",
                text="Potato started",
                duration_ms=1500,
            )
            logger.info("Potato Pet startup hook complete")
        except Exception:
            logger.exception("Potato Pet startup hook failed")

    def _shutdown(self):
        """Notify the desktop, terminate it, and restore the runner patch.

        The pet desktop is treated as a child of Potato: when Potato
        exits, the floating window goes with it. ``stop_desktop`` only
        kills a process that this plugin instance spawned (so a desktop
        started independently via ``python -m potato_pet_desktop start``
        is left alone), and the whole behaviour can be opted out of via
        ``POTATO_PET_STOP_ON_SHUTDOWN=0``.
        """
        try:
            emit_pet_event("potato.shutdown", text="", duration_ms=500)
        except Exception:
            logger.warning(
                "Potato Pet: shutdown event emit failed",
                exc_info=True,
            )

        try:
            result = stop_desktop(force=True, aggressive=True, grace=5.0)
            logger.info("Potato Pet: stop_desktop result=%s", result)
        except Exception:
            logger.exception("Potato Pet: failed to stop desktop process")

        try:
            restore_approval_service()
            restore_agent_runner()
        except Exception:
            logger.exception("Potato Pet: failed to restore class methods")

        logger.info("Potato Pet shutdown hook complete")


plugin = PotatoPetPlugin()
