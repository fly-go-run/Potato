# -*- coding: utf-8 -*-
"""Dependency graph and lifecycle algorithm tests for ServiceManager."""
from __future__ import annotations

import asyncio
from types import SimpleNamespace

import pytest

from qwenpaw.app.workspace.service_manager import (
    ServiceDependencyError,
    ServiceDescriptor,
    ServiceManager,
    ServiceStartStatus,
)


def _manager() -> ServiceManager:
    return ServiceManager(SimpleNamespace(agent_id="test-agent"))


def _post_init(
    name: str,
    events: list[str],
    *,
    error: BaseException | None = None,
):
    async def create(_workspace, _service):
        events.append(name)
        if error is not None:
            raise error
        return SimpleNamespace()

    return create


@pytest.mark.asyncio
async def test_missing_required_dependency_is_validated_before_creation():
    events: list[str] = []
    manager = _manager()
    manager.register(
        ServiceDescriptor(
            name="consumer",
            dependencies=["missing"],
            post_init=_post_init("consumer", events),
        ),
    )

    with pytest.raises(
        ServiceDependencyError,
        match="'consumer' requires 'missing'",
    ):
        await manager.start_all()

    assert events == []


@pytest.mark.asyncio
async def test_cycle_reports_path_and_after_edges_participate():
    events: list[str] = []
    manager = _manager()
    manager.register(
        ServiceDescriptor(
            name="a",
            dependencies=["b"],
            post_init=_post_init("a", events),
        ),
    )
    manager.register(
        ServiceDescriptor(
            name="b",
            after=["a"],
            post_init=_post_init("b", events),
        ),
    )

    with pytest.raises(
        ServiceDependencyError,
        match=r"a -> b -> a|b -> a -> b",
    ):
        await manager.start_all()

    assert events == []


@pytest.mark.asyncio
async def test_failed_required_dependency_skips_optional_consumer():
    events: list[str] = []
    manager = _manager()
    manager.register(
        ServiceDescriptor(
            name="dependency",
            post_init=_post_init(
                "dependency",
                events,
                error=RuntimeError("unavailable"),
            ),
            optional=True,
        ),
    )
    manager.register(
        ServiceDescriptor(
            name="consumer",
            dependencies=["dependency"],
            post_init=_post_init("consumer", events),
            optional=True,
        ),
    )

    results = await manager.start_all()

    assert events == ["dependency"]
    assert results["dependency"].status == "skipped_optional"
    assert results["consumer"].status == "skipped_optional"
    assert results["consumer"].blocked_by == ("dependency",)


@pytest.mark.asyncio
async def test_failed_required_dependency_fails_mandatory_consumer():
    events: list[str] = []
    manager = _manager()
    manager.register(
        ServiceDescriptor(
            name="dependency",
            post_init=_post_init(
                "dependency",
                events,
                error=RuntimeError("unavailable"),
            ),
            optional=True,
        ),
    )
    manager.register(
        ServiceDescriptor(
            name="consumer",
            dependencies=["dependency"],
            post_init=_post_init("consumer", events),
        ),
    )

    with pytest.raises(RuntimeError, match="blocked by failed"):
        await manager.start_all()

    assert events == ["dependency"]
    assert (
        manager.last_start_results["consumer"].status
        == ServiceStartStatus.FAILED
    )


@pytest.mark.asyncio
async def test_order_only_dependency_failure_does_not_propagate():
    events: list[str] = []
    manager = _manager()
    manager.register(
        ServiceDescriptor(
            name="optional_setup",
            post_init=_post_init(
                "optional_setup",
                events,
                error=RuntimeError("unavailable"),
            ),
            optional=True,
        ),
    )
    manager.register(
        ServiceDescriptor(
            name="watcher",
            after=["optional_setup", "not_registered"],
            post_init=_post_init("watcher", events),
        ),
    )

    results = await manager.start_all()

    assert events == ["optional_setup", "watcher"]
    assert results["optional_setup"].status == "skipped_optional"
    assert results["watcher"].status == "started"


@pytest.mark.asyncio
async def test_priority_is_tiebreak_within_topological_layer():
    events: list[str] = []
    manager = _manager()
    manager.register(
        ServiceDescriptor(
            name="late",
            priority=30,
            concurrent_init=False,
            post_init=_post_init("late", events),
        ),
    )
    manager.register(
        ServiceDescriptor(
            name="early_second",
            priority=10,
            concurrent_init=False,
            post_init=_post_init("early_second", events),
        ),
    )
    manager.register(
        ServiceDescriptor(
            name="early_first",
            priority=10,
            concurrent_init=False,
            post_init=_post_init("early_first", events),
        ),
    )

    results = await manager.start_all()

    assert events == ["early_second", "early_first", "late"]
    assert [result.status for result in results.values()] == [
        ServiceStartStatus.STARTED,
        ServiceStartStatus.STARTED,
        ServiceStartStatus.STARTED,
    ]


@pytest.mark.asyncio
async def test_concurrent_init_is_preserved_within_priority_group():
    manager = _manager()
    entered: set[str] = set()
    both_entered = asyncio.Event()
    release = asyncio.Event()

    def concurrent_post_init(name: str):
        async def create(_workspace, _service):
            entered.add(name)
            if len(entered) == 2:
                both_entered.set()
            await release.wait()
            return SimpleNamespace()

        return create

    for name in ("a", "b"):
        manager.register(
            ServiceDescriptor(
                name=name,
                priority=10,
                post_init=concurrent_post_init(name),
            ),
        )

    startup = asyncio.create_task(manager.start_all())
    await asyncio.wait_for(both_entered.wait(), timeout=1)
    release.set()
    results = await startup

    assert set(results) == {"a", "b"}


@pytest.mark.asyncio
async def test_stop_uses_reverse_layers_and_concurrency_within_layer():
    manager = _manager()
    events: list[str] = []
    roots_entered: set[str] = set()
    roots_started_together = asyncio.Event()
    release_roots = asyncio.Event()

    class Leaf:
        async def stop(self):
            events.extend(["leaf:start", "leaf:end"])

    class Root:
        def __init__(self, name: str):
            self.name = name

        async def stop(self):
            events.append(f"{self.name}:start")
            roots_entered.add(self.name)
            if len(roots_entered) == 2:
                roots_started_together.set()
            await release_roots.wait()
            events.append(f"{self.name}:end")

    manager.register(
        ServiceDescriptor(name="root_a", stop_method="stop"),
    )
    manager.register(
        ServiceDescriptor(name="root_b", stop_method="stop"),
    )
    manager.register(
        ServiceDescriptor(
            name="leaf",
            dependencies=["root_a", "root_b"],
            stop_method="stop",
        ),
    )
    manager.services.update(
        {
            "root_a": Root("root_a"),
            "root_b": Root("root_b"),
            "leaf": Leaf(),
        },
    )

    shutdown = asyncio.create_task(manager.stop_all(final=True))
    await asyncio.wait_for(roots_started_together.wait(), timeout=1)

    assert events[:2] == ["leaf:start", "leaf:end"]
    release_roots.set()
    await shutdown
    assert set(events[2:4]) == {"root_a:start", "root_b:start"}

