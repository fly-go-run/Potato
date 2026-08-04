# -*- coding: utf-8 -*-
"""Dependencies for runtime managers that initialize after the server binds."""

from typing import Any

from fastapi import HTTPException, Request


async def get_runtime_manager(request: Request, state_name: str) -> Any:
    """Wait for background manager construction and return the manager.

    The application deliberately binds its HTTP server before importing the
    optional provider and local-runtime stacks.  Endpoints using those
    managers must therefore join that initialization boundary instead of
    dereferencing ``None`` during the short startup window.
    """
    state = request.app.state
    ready_event = getattr(state, "runtime_managers_ready", None)
    if ready_event is not None and not ready_event.is_set():
        await ready_event.wait()

    manager = getattr(state, state_name, None)
    if manager is not None:
        return manager

    detail = getattr(state, "runtime_manager_error", None)
    raise HTTPException(
        status_code=503,
        detail=detail or f"{state_name} is not ready",
    )
