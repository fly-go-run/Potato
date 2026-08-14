# -*- coding: utf-8 -*-
"""Workspace lifecycle failure cleanup tests."""
from __future__ import annotations

import pytest

import qwenpaw.app.workspace.workspace as workspace_module
from qwenpaw.app.workspace.service_manager import (
    ServiceDescriptor,
    ServiceManager,
)
from qwenpaw.app.workspace.workspace import Workspace


@pytest.mark.asyncio
async def test_start_failure_rolls_back_actual_started_services(monkeypatch):
    events: list[str] = []

    class OwnedReusableService:
        async def start(self):
            events.append("owned:start")

        async def stop(self):
            events.append("owned:stop")

    async def create_owned(_workspace, _service):
        return OwnedReusableService()

    async def fail_after_owned(_workspace, _service):
        events.append("failure:start")
        raise RuntimeError("startup failed")

    workspace = Workspace.__new__(Workspace)
    workspace.agent_id = "test-agent"
    workspace._config = None  # pylint: disable=protected-access
    workspace._started = False  # pylint: disable=protected-access
    workspace._service_manager = ServiceManager(  # pylint: disable=protected-access
        workspace,
    )
    workspace._service_manager.register(  # pylint: disable=protected-access
        ServiceDescriptor(
            name="owned",
            post_init=create_owned,
            start_method="start",
            stop_method="stop",
            reusable=True,
        ),
    )
    workspace._service_manager.register(  # pylint: disable=protected-access
        ServiceDescriptor(
            name="failure",
            dependencies=["owned"],
            post_init=fail_after_owned,
        ),
    )
    workspace._migrate_legacy_weixin_data = (  # pylint: disable=protected-access
        lambda: None
    )

    monkeypatch.setattr(
        workspace_module,
        "load_agent_config",
        lambda _agent_id: object(),
    )
    monkeypatch.setattr(
        "qwenpaw.agents.skill_system.ensure_skill_pool_initialized",
        lambda: None,
    )

    with pytest.raises(RuntimeError, match="startup failed"):
        await workspace.start()

    assert events == ["owned:start", "failure:start", "owned:stop"]
    assert workspace._started is False  # pylint: disable=protected-access
    assert not workspace._service_manager.has_started_services  # pylint: disable=protected-access


def test_production_service_graph_snapshot():
    """Lock the complete production registry, edges, and Kahn layers."""
    workspace = Workspace.__new__(Workspace)
    manager = ServiceManager(workspace)
    workspace._service_manager = manager  # pylint: disable=protected-access

    Workspace._register_services(workspace)  # pylint: disable=protected-access

    assert tuple(manager.descriptors) == (
        "local_workspace",
        "session",
        "memory_manager",
        "driver_manager",
        "chat_manager",
        "channel_manager",
        "cron_manager",
        "agent_config_watcher",
        "driver_config_watcher",
    )
    actual_edges = {
        ("required", dependency, descriptor.name)
        for descriptor in manager.descriptors.values()
        for dependency in descriptor.dependencies
    } | {
        ("after", dependency, descriptor.name)
        for descriptor in manager.descriptors.values()
        for dependency in descriptor.after
    }
    assert actual_edges == {
        ("required", "local_workspace", "channel_manager"),
        ("required", "session", "channel_manager"),
        ("required", "chat_manager", "channel_manager"),
        ("required", "channel_manager", "cron_manager"),
        ("required", "chat_manager", "cron_manager"),
        ("after", "channel_manager", "agent_config_watcher"),
        ("after", "cron_manager", "agent_config_watcher"),
        ("after", "driver_manager", "driver_config_watcher"),
    }
    assert manager.startup_layers() == [
        [
            "local_workspace",
            "session",
            "memory_manager",
            "driver_manager",
            "chat_manager",
        ],
        ["channel_manager", "driver_config_watcher"],
        ["cron_manager"],
        ["agent_config_watcher"],
    ]
