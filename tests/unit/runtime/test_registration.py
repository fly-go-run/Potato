# -*- coding: utf-8 -*-
"""Tests for reversible registration ownership primitives."""

import pytest

from potato.runtime.registration import RegistrationHandle, Scope


@pytest.mark.asyncio
async def test_registration_handle_disposal_is_idempotent() -> None:
    calls: list[str] = []
    handle = RegistrationHandle(lambda: calls.append("disposed"))

    await handle.dispose()
    await handle.dispose()

    assert calls == ["disposed"]
    assert handle.disposed is True


@pytest.mark.asyncio
async def test_scope_disposes_in_reverse_order() -> None:
    calls: list[int] = []
    scope = Scope()
    scope.add(RegistrationHandle(lambda: calls.append(1)))
    scope.add(RegistrationHandle(lambda: calls.append(2)))

    await scope.aclose()

    assert calls == [2, 1]


@pytest.mark.asyncio
async def test_child_scope_is_closed_with_parent() -> None:
    calls: list[str] = []
    parent = Scope()
    parent.add(RegistrationHandle(lambda: calls.append("parent-before")))
    child = parent.child("mode")
    child.add(RegistrationHandle(lambda: calls.append("child")))
    parent.add(RegistrationHandle(lambda: calls.append("parent-after")))

    await parent.aclose()

    assert calls == ["parent-after", "child", "parent-before"]
    assert child.closed is True


@pytest.mark.asyncio
async def test_scope_mixes_async_and_sync_disposers() -> None:
    calls: list[str] = []
    scope = Scope()

    async def dispose_async() -> None:
        calls.append("async")

    scope.add(RegistrationHandle(lambda: calls.append("sync-first")))
    scope.add(RegistrationHandle(dispose_async))
    scope.add(RegistrationHandle(lambda: calls.append("sync-last")))

    await scope.aclose()

    assert calls == ["sync-last", "async", "sync-first"]


def test_sync_close_rejects_scope_with_async_handle() -> None:
    scope = Scope(tag="plugin")

    async def dispose_async() -> None:
        return None

    handle = scope.add(RegistrationHandle(dispose_async, tag="client"))

    with pytest.raises(RuntimeError, match="requires async disposal"):
        scope.close()

    assert scope.closed is False
    assert handle.disposed is False


@pytest.mark.asyncio
async def test_disposer_errors_are_aggregated_without_interrupting_cleanup() -> None:
    calls: list[str] = []
    scope = Scope(tag="plugin")

    def fail_first() -> None:
        calls.append("fail-first")
        raise ValueError("first")

    async def fail_second() -> None:
        calls.append("fail-second")
        raise RuntimeError("second")

    scope.add(RegistrationHandle(lambda: calls.append("oldest")))
    scope.add(RegistrationHandle(fail_first))
    scope.add(RegistrationHandle(fail_second))
    scope.add(RegistrationHandle(lambda: calls.append("newest")))

    with pytest.raises(ExceptionGroup) as raised:
        await scope.aclose()

    assert calls == ["newest", "fail-second", "fail-first", "oldest"]
    assert [str(exc) for exc in raised.value.exceptions] == ["second", "first"]
    assert scope.closed is True
