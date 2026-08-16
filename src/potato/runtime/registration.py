# -*- coding: utf-8 -*-
"""Ownership primitives for reversible runtime registrations."""

from __future__ import annotations

import inspect
from collections.abc import Awaitable, Callable
from typing import TypeAlias


Disposer: TypeAlias = Callable[[], None] | Callable[[], Awaitable[None]]


def _is_async_callable(fn: Disposer) -> bool:
    """Return whether calling *fn* is known to produce an awaitable."""
    if inspect.iscoroutinefunction(fn):
        return True
    call = getattr(fn, "__call__", None)
    return call is not None and inspect.iscoroutinefunction(call)


class RegistrationHandle:
    """One (atomic group of) registration's ownership token."""

    def __init__(self, dispose_fn: Disposer, *, tag: str = ""):
        if not callable(dispose_fn):
            raise TypeError("dispose_fn must be callable")
        self._dispose_fn = dispose_fn
        self.tag = tag
        self._is_async = _is_async_callable(dispose_fn)
        self._disposed = False

    def dispose_sync(self) -> None:
        """Dispose a synchronous registration exactly once."""
        if self._disposed:
            return
        if self._is_async:
            raise RuntimeError(
                f"registration {self.tag!r} requires async disposal",
            )
        result = self._dispose_fn()
        if inspect.isawaitable(result):
            close = getattr(result, "close", None)
            if close is not None:
                close()
            raise RuntimeError(
                f"registration {self.tag!r} requires async disposal",
            )
        self._disposed = True

    async def dispose(self) -> None:
        """Dispose a registration, awaiting its disposer when necessary."""
        if self._disposed:
            return
        result = self._dispose_fn()
        if inspect.isawaitable(result):
            await result
        self._disposed = True

    @property
    def is_async(self) -> bool:
        """Whether the disposer is known to require awaiting."""
        return self._is_async

    @property
    def disposed(self) -> bool:
        """Whether disposal completed successfully."""
        return self._disposed


class Scope:
    """Ordered registration container disposed in reverse order."""

    def __init__(self, *, tag: str = "") -> None:
        self.tag = tag
        self._handles: list[RegistrationHandle] = []
        self._closed = False

    def add(self, handle: RegistrationHandle) -> RegistrationHandle:
        """Add and return *handle* for convenient registration chaining."""
        if self._closed:
            raise RuntimeError(f"scope {self.tag!r} is closed")
        if not isinstance(handle, RegistrationHandle):
            raise TypeError("scope entries must be RegistrationHandle instances")
        self._handles.append(handle)
        return handle

    def child(self, tag: str) -> "Scope":
        """Create a child whose lifetime is owned by this scope."""
        child = Scope(tag=tag)
        self.add(RegistrationHandle(child.aclose, tag=tag))
        return child

    def close(self) -> None:
        """Synchronously close a scope containing only synchronous handles."""
        if self._closed:
            return
        async_handles = [h for h in self._handles if h.is_async and not h.disposed]
        if async_handles:
            tags = ", ".join(repr(h.tag) for h in async_handles)
            raise RuntimeError(
                f"scope {self.tag!r} requires async disposal: {tags}",
            )
        self._closed = True
        errors: list[Exception] = []
        for handle in reversed(self._handles):
            try:
                handle.dispose_sync()
            except Exception as exc:  # noqa: BLE001
                errors.append(exc)
        if errors:
            raise ExceptionGroup(
                f"errors while closing scope {self.tag!r}",
                errors,
            )

    async def aclose(self) -> None:
        """Close the scope, disposing every handle in reverse order."""
        if self._closed:
            return
        self._closed = True
        errors: list[Exception] = []
        for handle in reversed(self._handles):
            try:
                await handle.dispose()
            except Exception as exc:  # noqa: BLE001
                errors.append(exc)
        if errors:
            raise ExceptionGroup(
                f"errors while closing scope {self.tag!r}",
                errors,
            )

    @property
    def closed(self) -> bool:
        """Whether closing this scope has started."""
        return self._closed


__all__ = ["Disposer", "RegistrationHandle", "Scope"]
