# -*- coding: utf-8 -*-
"""Ownership-handle tests for plugin and runtime registries."""

from types import SimpleNamespace
from pathlib import Path

import pytest
from fastapi import APIRouter, FastAPI

from potato.app.workspace.workspace_plugins import WorkspacePlugins
from potato.app.channels.command_registry import CommandRegistry
from potato.plugins.api import PluginApi
from potato.plugins.architecture import (
    PluginEntryPoints,
    PluginManifest,
    PluginRecord,
)
from potato.plugins.loader import PluginLoader
from potato.plugins.registry import PluginRegistry, ProviderRegistration
from potato.runtime.hooks import HookBase, HookRegistry, HookResult
from potato.runtime.phases import Phase
from potato.runtime.slash_command_registry import CommandSpec


def _fresh_plugin_registry() -> PluginRegistry:
    old = PluginRegistry._instance
    PluginRegistry._instance = None
    try:
        return PluginRegistry()
    finally:
        PluginRegistry._instance = old


def test_plugin_registry_handle_removes_only_captured_identity() -> None:
    registry = _fresh_plugin_registry()
    handle = registry.register_provider(
        "plugin-a",
        "provider-a",
        object,
        "Provider A",
        "https://example.test",
        {},
    )
    replacement = ProviderRegistration(
        plugin_id="plugin-b",
        provider_id="provider-a",
        provider_class=object,
        label="Provider B",
        base_url="https://replacement.test",
    )
    registry._providers["provider-a"] = replacement

    handle.dispose_sync()

    assert registry.get_provider("provider-a") is replacement


def test_http_router_handle_removes_routes_prefix_and_openapi_cache() -> None:
    registry = _fresh_plugin_registry()
    app = FastAPI()
    registry.set_plugin_http_app(app)
    router = APIRouter()

    @router.get("/status")
    def status() -> dict[str, bool]:
        return {"ok": True}

    baseline_routes = list(app.router.routes)
    handle = registry.register_http_router(
        "plugin-a",
        router,
        prefix="/owned",
    )
    assert "/api/owned/status" in app.openapi()["paths"]
    assert app.openapi_schema is not None

    handle.dispose_sync()

    assert app.router.routes == baseline_routes
    assert registry.get_http_router_registrations() == []
    assert "/owned" not in registry._http_prefix_to_plugin
    assert app.openapi_schema is None
    assert "/api/owned/status" not in app.openapi()["paths"]


class _TestHook(HookBase):
    name = "owned-hook"
    phase = Phase.PRE_EXECUTE

    async def run(self, ctx) -> HookResult:  # noqa: ANN001, ARG002
        return HookResult()


def test_hook_registry_disposer_invalidates_sorted_cache() -> None:
    registry = HookRegistry()
    hook = _TestHook()
    handle = registry.register(hook)
    assert registry.hooks_for(Phase.PRE_EXECUTE) == [hook]
    assert Phase.PRE_EXECUTE in registry._sorted_cache

    handle.dispose_sync()

    assert Phase.PRE_EXECUTE not in registry._sorted_cache
    assert registry.hooks_for(Phase.PRE_EXECUTE) == []


async def test_plugin_api_scope_owns_delayed_workspace_registrations() -> None:
    registry = _fresh_plugin_registry()
    api = PluginApi("plugin-a", config={})
    api.set_registry(registry)
    first = SimpleNamespace(plugins=WorkspacePlugins(), agent_id="first")
    second = SimpleNamespace(plugins=WorkspacePlugins(), agent_id="second")
    api._get_all_workspaces = lambda: [first]
    api._get_workspace_from_info = lambda info: (
        second if info["agent_id"] == "second" else None
    )

    async def handler(ctx, args):  # noqa: ANN001, ARG001
        return None

    disposed: list[str] = []
    api.add_disposer(lambda: disposed.append("custom"), tag="custom")
    api.register_slash_command("owned", handler)
    for hook in registry.get_startup_hooks():
        hook.callback()
    for hook in registry.get_workspace_created_hooks():
        hook.callback({"agent_id": "second"})

    assert first.plugins.slash_command_registry.names() == ["owned"]
    assert second.plugins.slash_command_registry.names() == ["owned"]

    await api.scope.aclose()

    assert first.plugins.slash_command_registry.names() == []
    assert second.plugins.slash_command_registry.names() == []
    assert registry.get_startup_hooks() == []
    assert registry.get_workspace_created_hooks() == []
    assert disposed == ["custom"]


def test_control_command_handles_preserve_replacements() -> None:
    from potato.runtime.commands import control

    first = SimpleNamespace(command_name="owned")
    second = SimpleNamespace(command_name="owned")
    original = control._COMMAND_REGISTRY.get("owned")
    try:
        first_handle = control.register_command(first)
        second_handle = control.register_command(second)

        first_handle.dispose_sync()

        assert control._COMMAND_REGISTRY["owned"] is second
        second_handle.dispose_sync()
        assert "owned" not in control._COMMAND_REGISTRY
    finally:
        if original is None:
            control._COMMAND_REGISTRY.pop("owned", None)
        else:
            control._COMMAND_REGISTRY["owned"] = original


def test_priority_command_handle_preserves_replacements() -> None:
    registry = CommandRegistry()
    first = registry.register_command("/owned", priority_level=5)
    second = registry.register_command("/owned", priority_level=10)

    first.dispose_sync()

    assert registry.get_registered_commands()["/owned"] == 10
    second.dispose_sync()
    assert "/owned" not in registry.get_registered_commands()


def test_workspace_fallback_and_stop_handler_are_reversible() -> None:
    plugins = WorkspacePlugins()

    async def fallback(raw, ctx):  # noqa: ANN001, ARG001
        return None

    stop = SimpleNamespace(name="owned-stop")
    fallback_handle = plugins.register_fallback(fallback)
    stop_handle = plugins.register_stop_handler(stop)

    fallback_handle.dispose_sync()
    stop_handle.dispose_sync()

    assert plugins.slash_command_registry._fallback is None
    assert plugins.stop_handlers == []


def test_register_mode_rolls_back_partial_setup() -> None:
    plugins = WorkspacePlugins()
    workspace = SimpleNamespace(plugins=plugins)

    async def handler(ctx, args):  # noqa: ANN001, ARG001
        return None

    class FailingMode:
        name = "failing"

        def setup(self, target) -> None:  # noqa: ANN001
            target.plugins.register_slash_command(
                CommandSpec(name="partial", handler=handler),
            )
            raise RuntimeError("setup failed")

    with pytest.raises(RuntimeError, match="setup failed"):
        plugins.register_mode(FailingMode(), workspace)

    assert plugins.modes == []
    assert plugins.slash_command_registry.names() == []


async def test_loader_unload_awaits_plugin_scope() -> None:
    registry = _fresh_plugin_registry()
    loader = PluginLoader(plugin_dirs=[])
    loader.registry = registry
    api = PluginApi("owned-plugin", config={})
    api.set_registry(registry)
    api.register_provider("owned-provider", object)
    disposed: list[str] = []

    async def dispose_async() -> None:
        disposed.append("async")

    api.add_disposer(dispose_async, tag="async-resource")
    manifest = PluginManifest(
        id="owned-plugin",
        version="1.0.0",
        entry=PluginEntryPoints(backend="plugin.py"),
    )
    loader._loaded_plugins[manifest.id] = PluginRecord(
        manifest=manifest,
        source_path=Path("/fake-owned-plugin"),
        enabled=True,
        api=api,
    )

    await loader.unload_plugin(manifest.id)

    assert disposed == ["async"]
    assert registry.get_provider("owned-provider") is None
    assert loader.get_loaded_plugin(manifest.id) is None
