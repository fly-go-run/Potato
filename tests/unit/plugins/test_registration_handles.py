# -*- coding: utf-8 -*-
"""Ownership-handle tests for plugin and runtime registries."""

from fastapi import APIRouter, FastAPI
from types import SimpleNamespace

from qwenpaw.app.workspace.workspace_plugins import WorkspacePlugins
from qwenpaw.plugins.api import PluginApi
from qwenpaw.plugins.registry import PluginRegistry, ProviderRegistration
from qwenpaw.runtime.hooks import HookBase, HookRegistry, HookResult
from qwenpaw.runtime.phases import Phase


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
