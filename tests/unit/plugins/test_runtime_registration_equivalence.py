# -*- coding: utf-8 -*-
"""Runtime-level equivalence tests for plugin registration ownership."""

from pathlib import Path
from types import SimpleNamespace
import inspect
import sys

import pytest
from fastapi import APIRouter, FastAPI

from potato.app.channels.command_registry import CommandRegistry
from potato.app.routers.plugins import _post_load_setup
from potato.app.workspace.workspace_plugins import WorkspacePlugins
from potato.governance.tool_registry import DEFAULT_REGISTRY
from potato.modes.base import AgentMode
from potato.plugins.api import PluginApi
from potato.plugins.architecture import (
    PluginEntryPoints,
    PluginManifest,
    PluginRecord,
)
from potato.plugins.loader import PluginLoader
from potato.plugins.registry import PluginRegistry
from potato.providers.provider_manager import ProviderManager
from potato.runtime.commands import control
from potato.runtime.hooks import HookBase, HookResult
from potato.runtime.phases import Phase
from potato.runtime.prompt_manager import SyncPromptContributor
from potato.runtime.slash_command_registry import CommandSpec
from potato.runtime.tool_registry import ToolDescriptor


def _fresh_registry() -> PluginRegistry:
    old = PluginRegistry._instance
    PluginRegistry._instance = None
    try:
        return PluginRegistry()
    finally:
        PluginRegistry._instance = old


def _workspace_state(workspace) -> tuple:
    plugins = workspace.plugins
    hooks = tuple(
        hook
        for phase_hooks in plugins.hook_registry._by_phase.values()
        for hook in phase_hooks
    )
    return (
        tuple(plugins.slash_command_registry.names()),
        hooks,
        tuple(plugins.tool_registry.names()),
        tuple(plugins.prompt_manager.names()),
        tuple(mode.name for mode in plugins.modes),
        tuple(plugins.stop_handlers),
    )


def _runtime_state(
    registry,
    provider_manager,
    priority_registry,
    app,
    workspaces,
    tool_name,
) -> tuple:
    from potato.agents import tools as tools_module

    return (
        registry.get_all_providers(),
        registry.get_startup_hooks(),
        registry.get_shutdown_hooks(),
        registry.get_uninstall_hooks(),
        registry.get_workspace_created_hooks(),
        registry.get_control_commands(),
        registry.get_middleware_factories(),
        registry.get_http_router_registrations(),
        registry.get_prompt_sections(),
        registry.get_registered_channels(),
        registry.get_all_plugin_manifests(),
        dict(provider_manager.plugin_providers),
        dict(control._COMMAND_REGISTRY),
        priority_registry.get_registered_commands(),
        DEFAULT_REGISTRY.get_owner(tool_name),
        DEFAULT_REGISTRY.get_type(
            DEFAULT_REGISTRY.python_to_policy_name(tool_name),
        ),
        getattr(tools_module, tool_name, None),
        tool_name in tools_module.__all__,
        tuple(app.router.routes),
        frozenset(app.openapi()["paths"]),
        tuple(_workspace_state(ws) for ws in workspaces),
    )


@pytest.mark.asyncio
async def test_load_unload_restores_all_runtime_registration_surfaces(
    monkeypatch,
    tmp_path,
) -> None:
    plugin_id = "__runtime_kernel_equivalence__"
    provider_id = "__runtime_kernel_provider__"
    command_name = "__runtime_kernel_command__"
    tool_name = "__runtime_kernel_tool__"
    registry = _fresh_registry()
    app = FastAPI()
    registry.set_plugin_http_app(app)
    loader = PluginLoader(plugin_dirs=[])
    loader.registry = registry

    provider_manager = ProviderManager.__new__(ProviderManager)
    provider_manager.plugin_providers = {}
    provider_manager.plugin_path = tmp_path
    priority_registry = CommandRegistry()
    monkeypatch.setattr(
        "potato.app.channels.command_registry.CommandRegistry",
        lambda: priority_registry,
    )

    first = SimpleNamespace(plugins=WorkspacePlugins(), agent_id="first")
    second = SimpleNamespace(plugins=WorkspacePlugins(), agent_id="second")
    manager = SimpleNamespace(
        agents={"first": first},
        _bootstrap_kwargs={"builtin_tool_funcs": []},
    )
    registry.set_workspace_manager(manager)
    monkeypatch.setattr(
        "potato.plugins.api._write_tool_config",
        lambda *args, **kwargs: None,
    )

    baseline = _runtime_state(
        registry,
        provider_manager,
        priority_registry,
        app,
        [first, second],
        tool_name,
    )

    async def command_handler(ctx, args):  # noqa: ANN001, ARG001
        return None

    async def tool_func() -> str:
        return "ok"

    tool_func.__name__ = tool_name

    class Provider:
        @staticmethod
        def get_default_models() -> list:
            return []

    class ControlHandler:
        command_name = "__runtime_kernel_command__"

        async def handle(self, context) -> str:  # noqa: ANN001, ARG002
            return "ok"

    class RuntimeHook(HookBase):
        name = "runtime-kernel-hook"
        phase = Phase.PRE_EXECUTE

        async def run(self, ctx) -> HookResult:  # noqa: ANN001, ARG002
            return HookResult()

    class ModePrompt(SyncPromptContributor):
        name = "runtime-kernel-mode-prompt"

        def contribute_sync(self, ctx) -> str:  # noqa: ANN001, ARG002
            return "mode"

    class RuntimeMode(AgentMode):
        name = "runtime-kernel-mode"

        def commands(self) -> list[CommandSpec]:
            return [CommandSpec(name="mode-command", handler=command_handler)]

        def tools(self) -> list[ToolDescriptor]:
            return [ToolDescriptor(name="mode-tool", func=tool_func)]

        def hooks(self) -> list[HookBase]:
            return [RuntimeHook()]

        def prompt_contributors(self) -> list[ModePrompt]:
            return [ModePrompt()]

    api = PluginApi(plugin_id, config={}, manifest={"id": plugin_id})
    api.set_registry(registry)
    api.scope.add(registry.register_plugin_manifest(plugin_id, api.manifest))
    api.register_provider(provider_id, Provider)
    api.register_control_command(ControlHandler())
    api.register_tool(tool_name, tool_func)
    api.register_slash_command("plugin-command", command_handler)
    api.register_mode(RuntimeMode)
    api.register_runtime_hook(RuntimeHook())
    api.register_agent_stop_handler(lambda ctx: None, name="plugin-stop")
    api.register_prompt_section("plugin-prompt", "workspace", lambda _: "x")
    api.register_middleware(lambda ctx, cfg: None)
    router = APIRouter()

    @router.get("/status")
    def status() -> dict[str, bool]:
        return {"ok": True}

    api.register_http_router(router, prefix="/runtime-kernel")

    def register_late_prompt() -> None:
        api.register_prompt_section(
            "late-startup-prompt",
            "workspace",
            lambda _: "late",
        )

    api.register_startup_hook("late-api-registration", register_late_prompt)

    manifest = PluginManifest(
        id=plugin_id,
        version="1.0.0",
        entry=PluginEntryPoints(backend="plugin.py"),
    )
    record = PluginRecord(
        manifest=manifest,
        source_path=Path("/fake-runtime-kernel"),
        enabled=True,
        api=api,
    )
    loader._loaded_plugins[plugin_id] = record
    request = SimpleNamespace(
        app=SimpleNamespace(
            state=SimpleNamespace(
                plugin_loader=loader,
                provider_manager=provider_manager,
            ),
        ),
    )

    await _post_load_setup(request, plugin_id)
    manager.agents["second"] = second
    for hook in registry.get_workspace_created_hooks():
        result = hook.callback({"agent_id": "second"})
        if inspect.isawaitable(result):
            await result
    assert "/api/runtime-kernel/status" in app.openapi()["paths"]

    await loader.unload_plugin(plugin_id)

    assert _runtime_state(
        registry,
        provider_manager,
        priority_registry,
        app,
        [first, second],
        tool_name,
    ) == baseline


@pytest.mark.asyncio
async def test_unloading_one_plugin_preserves_the_other() -> None:
    registry = _fresh_registry()
    app = FastAPI()
    registry.set_plugin_http_app(app)
    loader = PluginLoader(plugin_dirs=[])
    loader.registry = registry

    def register(plugin_id: str) -> PluginApi:
        api = PluginApi(plugin_id, config={})
        api.set_registry(registry)
        manifest_dict = {"id": plugin_id}
        api.scope.add(registry.register_plugin_manifest(plugin_id, manifest_dict))
        api.register_provider(f"provider-{plugin_id}", object)
        router = APIRouter()
        router.add_api_route("/", lambda: {"plugin": plugin_id})
        api.register_http_router(router, prefix=f"/{plugin_id}")
        manifest = PluginManifest(
            id=plugin_id,
            version="1.0.0",
            entry=PluginEntryPoints(backend="plugin.py"),
        )
        loader._loaded_plugins[plugin_id] = PluginRecord(
            manifest=manifest,
            source_path=Path(f"/fake-{plugin_id}"),
            enabled=True,
            api=api,
        )
        return api

    register("plugin-a")
    api_b = register("plugin-b")

    await loader.unload_plugin("plugin-a")

    assert registry.get_provider("provider-plugin-a") is None
    assert registry.get_provider("provider-plugin-b") is not None
    assert "/api/plugin-a/" not in app.openapi()["paths"]
    assert "/api/plugin-b/" in app.openapi()["paths"]
    assert api_b.scope.closed is False
    await loader.unload_plugin("plugin-b")


def test_mode_setup_failure_leaves_no_workspace_residue() -> None:
    plugins = WorkspacePlugins()
    workspace = SimpleNamespace(plugins=plugins)

    async def handler(ctx, args):  # noqa: ANN001, ARG001
        return None

    class FailingMode:
        name = "runtime-failing-mode"

        def setup(self, target) -> None:  # noqa: ANN001
            target.plugins.register_slash_command(
                CommandSpec(name="partial-runtime", handler=handler),
            )
            target.plugins.register_tool(
                ToolDescriptor(name="partial-tool", func=handler),
            )
            raise RuntimeError("mid-setup")

    baseline = _workspace_state(workspace)
    with pytest.raises(RuntimeError, match="mid-setup"):
        plugins.register_mode(FailingMode(), workspace)
    assert _workspace_state(workspace) == baseline
