# -*- coding: utf-8 -*-
"""Potato terminal chat UI (TUI).

A Textual front-end, bundled into Potato, that drives the agent over ACP by
spawning ``potato acp`` as a subprocess. The UI layer only ever sees the
normalized :class:`~potato.cli.tui.transport.base.TuiTransport` interface and
the :mod:`~potato.cli.tui.events` event union, so the transport is a swappable
seam: a future in-process transport can replace the ACP subprocess without
touching any widget.

Entry points are retired. Bare ``potato`` and ``potato tui`` no longer
open this UI.

Originally developed as the standalone ``paw`` CLI; relocated here in Phase 1.
"""

from __future__ import annotations

from .__version__ import __version__

__all__ = ["__version__"]
