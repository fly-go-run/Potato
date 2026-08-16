# -*- coding: utf-8 -*-
# pylint: disable=redefined-outer-name,unused-argument
import asyncio
import hmac
import inspect
import mimetypes
import os
import sys
import time
from contextlib import asynccontextmanager, suppress
from pathlib import Path
from typing import Any

from fastapi import FastAPI, Header, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import FileResponse
from fastapi.staticfiles import StaticFiles

from ..__version__ import __version__
from ..backup._utils.safe_swap import cleanup_startup_restore_artifacts
from ..config import load_config  # pylint: disable=no-name-in-module
from ..config.utils import get_config_path, read_last_api
from ..constant import (
    CORS_ORIGINS,
    DOCS_ENABLED,
    LOG_LEVEL_ENV,
    PROJECT_NAME,
    WORKING_DIR,
)
from ..envs import load_envs_into_environ
from ..utils.logging import (
    LOG_FILE_PATH,
    add_project_file_handler,
    setup_logger,
)
from ..utils.console_static import resolve_web_static_dir
from ..utils.startup_display import AgentStartupDisplay
from ..utils.system_info import summarize_python_environment
from .auth import (
    AuthMiddleware,
    auto_register_from_env,
    check_proxy_config_sanity,
)
from .migration import (
    ensure_default_agent_exists,
    migrate_legacy_skills_to_skill_pool,
    migrate_legacy_workspace_to_default_agent,
    remove_builtin_qa_agent_profiles,
)
from .routers import create_agent_scoped_router
from .routers import router as api_router
from .routers.agent_scoped import AgentContextMiddleware
from .routers.approval import router as approval_router
from .routers.coding_mode import router as coding_mode_router
from .routers.healthz import router as healthz_router
from .routers.loops import router as loops_router
from .routers.tool_calls import router as tool_calls_router
from .routers.voice import voice_router

# Apply log level on load so reload child process gets same level as CLI.
logger = setup_logger(os.environ.get(LOG_LEVEL_ENV, "info"))

# Ensure static assets are served with browser-compatible MIME types across
# platforms (notably Windows may miss .js/.mjs mappings).
mimetypes.init()
mimetypes.add_type("application/javascript", ".js")
mimetypes.add_type("application/javascript", ".mjs")
mimetypes.add_type("text/css", ".css")
mimetypes.add_type("application/wasm", ".wasm")
mimetypes.add_type("image/svg+xml", ".svg")

# Load persisted env vars into os.environ at module import time
# so they are available before the lifespan starts.
load_envs_into_environ()


@asynccontextmanager
async def lifespan(  # pylint: disable=too-many-statements,too-many-branches
    app: FastAPI,
):
    startup_start_time = time.time()
    add_project_file_handler(LOG_FILE_PATH)
    app.state.startup_ready = asyncio.Event()
    app.state.startup_state = "starting"
    app.state.startup_error = None
    app.state.startup_time = startup_start_time
    app.state.runtime_managers_ready = asyncio.Event()
    app.state.runtime_manager_error = None

    # These managers import optional provider/local-runtime modules.  Keep
    # their references in the lifespan closure, but construct them in the
    # background phase after the server has yielded to Uvicorn.
    provider_manager = None
    local_model_manager = None

    # ================================================================
    # Fast synchronous setup (target < 100ms)
    # Everything here must be lightweight so the server starts quickly.
    # ================================================================

    try:
        cleanup_startup_restore_artifacts()
    except Exception as exc:
        message = (
            "Potato startup failed because restore artifact cleanup did not "
            "complete. Another restore or cleanup may still be running, or "
            "a previous restore may need recovery before startup can safely "
            "read restored files."
        )
        logger.error(message, exc_info=True)
        raise RuntimeError(f"{message} Original error: {exc}") from exc

    auto_register_from_env()
    check_proxy_config_sanity()

    try:
        from ..utils.telemetry import (
            collect_and_upload_telemetry,
            has_telemetry_been_collected,
            is_telemetry_opted_out,
        )

        if not is_telemetry_opted_out(
            WORKING_DIR,
        ) and not has_telemetry_been_collected(WORKING_DIR):
            collect_and_upload_telemetry(WORKING_DIR)
    except Exception:
        logger.debug(
            "Telemetry collection skipped due to error",
            exc_info=True,
        )

    logger.debug("Checking for legacy config migration...")
    migrate_legacy_workspace_to_default_agent()
    ensure_default_agent_exists()
    migrate_legacy_skills_to_skill_pool()
    remove_builtin_qa_agent_profiles()

    # Migrate old conversations from sessions/*.json into each scroll agent's
    # history.db, so chats from before scroll existed stay recallable. This is
    # a one-off backfill, not core startup work: if it fails, we log and keep
    # booting — that agent just won't have its old chats imported (scroll still
    # records new turns normally). The import sits inside the try for the same
    # reason — even a failed import must not block init.
    #
    # Note: being pure backfill, this could later run asynchronously (off the
    # boot path) to speed up startup.
    try:
        from ..agents.context.scroll.sync import sync_all_scroll_agents

        sync_all_scroll_agents()
    except Exception:  # noqa: BLE001 - session sync must never block startup
        logger.warning("session-sync: import/launch failed", exc_info=True)

    # --- AppServiceManager + WorkspaceRegistry ---
    app_services = None
    workspace_registry = None
    try:
        from .app_services import AppServiceManager
        from .workspace_registry import WorkspaceRegistry

        app_services = AppServiceManager()
        await app_services.start()
        app.state.app_services = app_services

        workspace_registry = WorkspaceRegistry(
            app_services=app_services,
        )
        app.state.workspace_registry = workspace_registry
        logger.debug("Runtime infrastructure initialized")

        # --- @api_action auto-registration ---
        _api_action_command_specs: list[Any] = []
        try:
            from ..api_action import ManagerRegistry
            from ._api_action_routes import (
                collect_slash_specs_from_api_actions,
                register_http_routes,
            )
            from .crons.manager import CronManager

            manager_registry = ManagerRegistry()

            def _get_default_cron_mgr(app_inst: Any) -> Any:
                mam = getattr(app_inst.state, "multi_agent_manager", None)
                if mam is None:
                    return None
                # pylint: disable-next=protected-access
                ws = mam._workspaces.get("default")
                return getattr(ws, "cron_manager", None) if ws else None

            manager_registry.register(CronManager, _get_default_cron_mgr)
            app.state.manager_registry = manager_registry

            n_routes = register_http_routes(app, manager_registry)
            logger.debug("Auto-registered %d HTTP routes", n_routes)

            _api_action_command_specs.extend(
                collect_slash_specs_from_api_actions(manager_registry),
            )
            logger.debug(
                "Collected %d slash specs from @api_action",
                len(_api_action_command_specs),
            )
        except Exception:
            logger.debug(
                "@api_action auto-registration skipped",
                exc_info=True,
            )

        # --- HITL slash commands ---
        try:
            from .app_services._builtin_tool_commands import (
                build_tool_command_specs,
            )

            _api_action_command_specs.extend(
                build_tool_command_specs(app_services.tool_coordinator),
            )
            logger.debug("HITL tool commands registered")
        except Exception:
            logger.debug(
                "HITL tool command registration skipped",
                exc_info=True,
            )

        # --- Use shared bootstrap factory ---
        from .workspace.bootstrap_factory import WorkspaceBootstrapFactory

        factory_kwargs = WorkspaceBootstrapFactory.build_bootstrap_kwargs(
            app_services,
            extra_command_specs=_api_action_command_specs
            if _api_action_command_specs
            else None,
        )
        # Merge factory output into workspace_registry._bootstrap_kwargs
        for key, value in factory_kwargs.items():
            # pylint: disable-next=protected-access
            workspace_registry._bootstrap_kwargs[key] = value

        # Warm descriptor-driven caches off the event loop so the first
        # /tools or agent-config path does not pay full import cost inline.
        def _warm_descriptor_caches() -> None:
            from ..config.config import _default_builtin_tools
            from ..governance.policy import get_default_user_rules
            from ..governance.tool_registry import DEFAULT_REGISTRY

            DEFAULT_REGISTRY.get_all_tool_names()
            _default_builtin_tools()
            get_default_user_rules()

        try:
            await asyncio.to_thread(_warm_descriptor_caches)
        except Exception:
            logger.debug(
                "Descriptor cache warm-up skipped",
                exc_info=True,
            )

    except Exception as exc:
        app.state.startup_state = "failed"
        app.state.startup_error = (
            f"Core runtime infrastructure failed: {type(exc).__name__}"
        )
        logger.critical(
            "Core runtime infrastructure initialization failed",
            exc_info=True,
        )
        if app_services is not None:
            with suppress(Exception):
                await app_services.stop()
        raise RuntimeError(
            "Core runtime infrastructure initialization failed",
        ) from exc

    # Start token usage manager background tasks
    logger.debug("Starting TokenUsageManager background tasks...")
    from ..token_usage import get_token_usage_manager

    token_usage_manager = get_token_usage_manager()
    token_usage_manager.start(flush_interval=10)

    # Expose to endpoints (must be set before first request arrives).
    # WorkspaceRegistry IS-A MultiAgentManager — backward compat for
    # routers / agent_context that read app.state.multi_agent_manager.
    app.state.multi_agent_manager = workspace_registry
    try:
        from .job_wakeup import attach_job_wakeup

        attach_job_wakeup(
            tool_coordinator=app_services.tool_coordinator,
            get_workspace=lambda agent_id: workspace_registry.agents.get(
                agent_id,
            ),
        )
    except Exception:
        logger.debug("job wakeup wiring skipped", exc_info=True)
    app.state.provider_manager = provider_manager
    app.state.local_model_manager = local_model_manager
    app.state.plugin_loader = None
    app.state.plugin_registry = None

    async def _get_agent_by_id(agent_id: str = None):
        """Get agent instance by ID, or active agent if not specified."""
        if agent_id is None:
            config = load_config(get_config_path())
            agent_id = config.agents.active_agent or "default"
        return await workspace_registry.get_agent(agent_id)

    app.state.get_agent_by_id = _get_agent_by_id

    fast_elapsed = time.time() - startup_start_time
    logger.info(
        f"Server ready in {fast_elapsed:.3f}s (agents loading in background)",
    )

    # ================================================================
    # Background heavy initialization
    # Agents, plugins, and services start in a background task so the
    # server can begin accepting HTTP requests immediately.
    # First API requests that need an agent will await its readiness
    # via MultiAgentManager.get_agent() lazy-loading / event wait.
    # ================================================================

    startup_display = AgentStartupDisplay(read_last_api()).start()

    async def _background_startup():  # pylint: disable=too-many-statements
        nonlocal provider_manager, local_model_manager
        try:
            # ProviderManager imports all concrete provider classes and the
            # local model manager imports llama.cpp helpers.  Their
            # constructors are synchronous, so do both in a worker thread to
            # keep the event loop available for healthz and static requests.
            def _create_runtime_managers():
                from ..local_models.manager import LocalModelManager
                from ..providers.provider_manager import ProviderManager

                return (
                    ProviderManager.get_instance(),
                    LocalModelManager.get_instance(),
                )

            provider_manager, local_model_manager = await asyncio.to_thread(
                _create_runtime_managers,
            )
            app.state.provider_manager = provider_manager
            app.state.local_model_manager = local_model_manager
            app.state.runtime_managers_ready.set()

            # First-run provisioning(家人分发预配置):发现 provision.json 就
            # 走设置页同款代码路径应用;任何失败只记日志,绝不阻塞启动。
            try:
                from .agent_context import get_active_agent_id
                from .provisioning import apply_provision_file

                await apply_provision_file(
                    provider_manager,
                    get_active_agent_id(),
                )
            except Exception:  # noqa: BLE001
                logger.warning("provisioning failed", exc_info=True)

            # ---- Plugin System (phase 1: channel plugins) ----
            # Load channel-type plugins *before* agents start so that
            # ChannelManager discovers them via get_channel_registry()
            # on first creation — no reload needed afterwards.
            logger.debug("Initializing plugin system...")

            # PawApps install into the plugins dir alongside other plugins
            # and load through the same pipeline as 'app'-type plugins
            # (plugin.json carrying meta.pawapp); surfaced only in the App
            # Center, hidden from the sidebar.
            def _create_plugin_loader():
                from ..config.utils import get_plugins_dir
                from ..plugins.loader import PluginLoader

                return PluginLoader([get_plugins_dir()])

            # The loader module imports packaging and plugin metadata models;
            # keep that optional stack off the event loop too.
            plugin_loader = await asyncio.to_thread(_create_plugin_loader)

            plugin_loader.registry.set_plugin_http_app(app)

            config = await asyncio.to_thread(
                load_config,
                get_config_path(),
            )
            plugin_configs = (
                config.plugins if hasattr(config, "plugins") else {}
            )
            logger.debug(
                f"Loading plugins with {len(plugin_configs)} config(s)",
            )

            # Phase 1: load channel plugins before agents start
            await plugin_loader.load_all_plugins(
                configs=plugin_configs,
                types=["channel"],
            )
            logger.debug("Phase 1: channel plugins loaded")

            def _mark_core_agents_ready(results: dict[str, bool]) -> None:
                """Publish readiness after the core agent phase."""
                core_elapsed = time.time() - startup_start_time
                if results.get("default") is False:
                    app.state.startup_state = "failed"
                    app.state.startup_error = "Default agent failed to start"
                    startup_display.mark_failed(app.state.startup_error)
                    return
                app.state.startup_state = "ready"
                startup_display.mark_core_ready(core_elapsed)
                app.state.startup_ready.set()

            startup_results = (
                await workspace_registry.start_all_configured_agents(
                    on_core_ready=_mark_core_agents_ready,
                    startup_display=startup_display,
                )
            )
            if startup_results.get("default") is False:
                app.state.startup_state = "failed"
                app.state.startup_error = "Default agent failed to start"
                startup_display.mark_failed(app.state.startup_error)
            elif app.state.startup_ready.is_set():
                startup_display.mark_finalizing()

            provider_manager.start_local_model_resume(local_model_manager)

            # Phase 2: load remaining plugins (channel plugins already
            # loaded — load_plugin skips them automatically)
            loaded_plugins = await plugin_loader.load_all_plugins(
                configs=plugin_configs,
            )
            logger.debug(f"Loaded {len(loaded_plugins)} plugin(s)")

            from ..plugins.runtime import RuntimeHelpers

            runtime_helpers = RuntimeHelpers(
                provider_manager=provider_manager,
            )
            plugin_loader.registry.set_runtime_helpers(runtime_helpers)
            plugin_loader.registry.set_workspace_manager(
                workspace_registry,
            )

            for (
                provider_id,
                provider_reg,
            ) in plugin_loader.registry.get_all_providers().items():
                handle = provider_manager.register_plugin_provider(
                    provider_id=provider_id,
                    provider_class=provider_reg.provider_class,
                    label=provider_reg.label,
                    base_url=provider_reg.base_url,
                    metadata=provider_reg.metadata,
                )
                record = loaded_plugins.get(provider_reg.plugin_id)
                if record is not None and record.api is not None:
                    record.api.scope.add(handle)
                logger.debug(
                    f"Registered plugin provider: {provider_id}",
                )

            app.state.plugin_loader = plugin_loader
            app.state.plugin_registry = plugin_loader.registry

            # ---- Plugin Control Commands ----
            logger.debug("Registering plugin control commands...")
            from potato.runtime.commands.control import register_command

            from ..app.channels.command_registry import CommandRegistry

            command_registry = CommandRegistry()

            control_commands = plugin_loader.registry.get_control_commands()
            for cmd_reg in control_commands:
                try:
                    handler_handle = register_command(cmd_reg.handler)
                    record = loaded_plugins.get(cmd_reg.plugin_id)
                    if record is not None and record.api is not None:
                        record.api.scope.add(handler_handle)

                    priority_handle = command_registry.register_command(
                        f"/{cmd_reg.handler.command_name}",
                        priority_level=cmd_reg.priority_level,
                    )
                    if record is not None and record.api is not None:
                        record.api.scope.add(priority_handle)

                    logger.debug(
                        f"Registered plugin control command: "
                        f"/{cmd_reg.handler.command_name} "
                        f"from plugin '{cmd_reg.plugin_id}' (priority"
                        f"={cmd_reg.priority_level})",
                    )
                except Exception as e:
                    logger.error(
                        f"✗ Failed to register control command "
                        f"'{cmd_reg.handler.command_name}' "
                        f"from plugin '{cmd_reg.plugin_id}': {e}",
                        exc_info=True,
                    )

            # ---- Startup Hooks ----
            logger.debug("Executing plugin startup hooks...")
            startup_hooks = plugin_loader.registry.get_startup_hooks()
            for hook in startup_hooks:
                try:
                    logger.debug(
                        f"Executing startup hook '{hook.hook_name}' "
                        f"from plugin '{hook.plugin_id}' "
                        f"(priority={hook.priority})",
                    )

                    result = hook.callback()
                    if inspect.iscoroutine(
                        result,
                    ) or inspect.isawaitable(result):
                        await result

                    logger.debug(
                        f"Completed startup hook '{hook.hook_name}' "
                        f"from plugin '{hook.plugin_id}'",
                    )
                except Exception as e:
                    logger.error(
                        f"✗ Failed to execute startup hook "
                        f"'{hook.hook_name}' "
                        f"from plugin '{hook.plugin_id}': {e}",
                        exc_info=True,
                    )

            # ---- Approval Service ----
            try:
                default_agent = await workspace_registry.get_agent(
                    "default",
                )
                if default_agent.channel_manager:
                    from .approvals import get_approval_service

                    get_approval_service().set_channel_manager(
                        default_agent.channel_manager,
                    )
            except Exception as e:
                logger.warning(f"Approval service setup skipped: {e}")

            # ---- Skill pool auto-update sync ----
            try:
                from ..agents.skill_system import run_pool_auto_update_sync

                await asyncio.to_thread(run_pool_auto_update_sync)
            except Exception:
                logger.warning(
                    "Skill pool auto-update sync skipped on startup",
                    exc_info=True,
                )

            startup_elapsed = time.time() - startup_start_time
            logger.info(
                "Background startup completed in "
                f"{startup_elapsed:.3f} seconds",
            )
            if app.state.startup_ready.is_set():
                startup_display.complete(startup_elapsed)

        except Exception as exc:
            if not app.state.runtime_managers_ready.is_set():
                app.state.runtime_manager_error = (
                    f"Runtime manager initialization failed: "
                    f"{type(exc).__name__}"
                )
                app.state.runtime_managers_ready.set()
            if app.state.startup_ready.is_set():
                app.state.startup_state = "degraded"
                app.state.startup_error = (
                    "Optional background startup failed: "
                    f"{type(exc).__name__}"
                )
            else:
                app.state.startup_state = "failed"
                app.state.startup_error = (
                    "Core background startup failed: " f"{type(exc).__name__}"
                )
                startup_display.mark_failed(app.state.startup_error)
            logger.error(
                "Background startup encountered an error",
                exc_info=True,
            )

    _bg_task = asyncio.create_task(_background_startup())

    try:
        yield
    finally:
        # Cancel background startup if still in progress
        if not _bg_task.done():
            _bg_task.cancel()
            with suppress(asyncio.CancelledError):
                await _bg_task

        # ==================== Execute Shutdown Hooks ====================
        plugin_registry = getattr(app.state, "plugin_registry", None)
        if plugin_registry is not None:
            logger.info("Executing plugin shutdown hooks...")
            shutdown_hooks = plugin_registry.get_shutdown_hooks()
            for hook in shutdown_hooks:
                try:
                    logger.info(
                        f"Executing shutdown hook '{hook.hook_name}' "
                        f"from plugin '{hook.plugin_id}' (priority"
                        f"={hook.priority})",
                    )

                    result = hook.callback()
                    if inspect.iscoroutine(result) or inspect.isawaitable(
                        result,
                    ):
                        await result

                    logger.info(
                        f"✓ Completed shutdown hook '{hook.hook_name}' "
                        f"from plugin '{hook.plugin_id}'",
                    )
                except Exception as e:
                    logger.error(
                        f"✗ Failed to execute shutdown hook "
                        f"'{hook.hook_name}' "
                        f"from plugin '{hook.plugin_id}': {e}",
                        exc_info=True,
                    )

        local_model_mgr = getattr(app.state, "local_model_manager", None)
        if local_model_mgr is not None:
            logger.info("Stopping local model server...")
            try:
                await local_model_mgr.shutdown_server()
            except Exception as exc:
                logger.error(
                    "Error shutting down local model server gracefully: %s",
                    exc,
                )
                with suppress(OSError, RuntimeError, ValueError):
                    local_model_mgr.shutdown_server_sync()

        # Stop AppServiceManager (ToolCoordinator shutdown, etc.)
        _app_svc = getattr(app.state, "app_services", None)
        if _app_svc is not None:
            try:
                await _app_svc.stop()
            except Exception as e:
                logger.error(f"Error stopping AppServiceManager: {e}")
        try:
            from ..runtime.jobs import get_job_registry

            await get_job_registry().shutdown()
        except Exception as e:
            logger.error(f"Error shutting down job registry: {e}")

        # Stop multi-agent manager (stops all agents and their components)
        multi_agent_mgr = getattr(app.state, "multi_agent_manager", None)
        if multi_agent_mgr is not None:
            logger.info("Stopping MultiAgentManager...")
            try:
                await multi_agent_mgr.stop_all()
            except Exception as e:
                logger.error(f"Error stopping MultiAgentManager: {e}")

        # These three cleanup tasks are independent; run in parallel.
        from ..agents.skill_system.hub import aclose_hub_client
        from ..agents.tools.browser_control import stop_all_browsers

        async def _stop_token_usage():
            logger.info("Stopping TokenUsageManager...")
            try:
                await token_usage_manager.stop()
            except Exception as e:
                logger.error(
                    f"Error stopping TokenUsageManager: {e}",
                )

        async def _stop_browsers():
            try:
                await stop_all_browsers()
            except Exception as e:
                logger.error(
                    f"Error stopping browsers: {e}",
                )

        async def _close_hub():
            try:
                await aclose_hub_client()
            except Exception as e:
                logger.error(
                    f"Error closing skills hub HTTP client: {e}",
                )

        await asyncio.gather(
            _stop_token_usage(),
            _stop_browsers(),
            _close_hub(),
        )

        # Destroy Windows sandbox artifacts (user accounts, profiles, ACLs,
        # firewall rules). Runs in a thread because it invokes subprocess
        # calls (takeown, icacls, net user, powershell) that may block.
        if sys.platform == "win32":
            try:
                from ..sandbox import shutdown_all_sandboxes

                logger.info("Cleaning up Windows sandbox artifacts...")
                await asyncio.to_thread(shutdown_all_sandboxes)
                logger.info("Windows sandbox cleanup complete.")
            except Exception as e:
                logger.error(f"Error during sandbox cleanup: {e}")

        logger.info("Application shutdown complete")
        startup_display.stop()


app = FastAPI(
    lifespan=lifespan,
    docs_url="/docs" if DOCS_ENABLED else None,
    redoc_url="/redoc" if DOCS_ENABLED else None,
    openapi_url="/openapi.json" if DOCS_ENABLED else None,
)

# Add agent context middleware for agent-scoped routes
app.add_middleware(AgentContextMiddleware)

app.add_middleware(AuthMiddleware)

# Apply CORS middleware if CORS_ORIGINS is set
if CORS_ORIGINS:
    origins = [o.strip() for o in CORS_ORIGINS.split(",") if o.strip()]
    app.add_middleware(
        CORSMiddleware,
        allow_origins=origins,
        allow_credentials=True,
        allow_methods=["*"],
        allow_headers=["*"],
        expose_headers=["Content-Disposition"],
    )


_WEB_STATIC_DIR = resolve_web_static_dir()
_WEB_INDEX = Path(_WEB_STATIC_DIR) / "index.html" if _WEB_STATIC_DIR else None
logger.info("WEB_STATIC_DIR: %s", _WEB_STATIC_DIR)

# The SPA entry (index.html) must never be cached: it references content-hashed
# JS/CSS bundles, so a stale cached index.html would keep pointing the WebView
# at old asset hashes after a rebuild (see desktop dev cache issue). The hashed
# assets under /assets remain safely cacheable because their name changes with
# their content.
_INDEX_NO_CACHE_HEADERS = {
    "Cache-Control": "no-cache, no-store, must-revalidate",
    # Pragma/Expires cover legacy proxies and older WebView caches that do not
    # honor Cache-Control on their own.
    "Pragma": "no-cache",
    "Expires": "0",
}


@app.get("/")
def read_root():
    if _WEB_INDEX and _WEB_INDEX.exists():
        return FileResponse(_WEB_INDEX, headers=_INDEX_NO_CACHE_HEADERS)
    return {
        "message": (
            f"{PROJECT_NAME} web UI is not available. "
            "If you installed the project from source code, please run "
            "`npm ci && npm run build` in the `app/` "
            f"directory, and restart {PROJECT_NAME} to enable the "
            "web UI."
        ),
    }


@app.get("/api/version")
def get_version():
    """Return the current application version (public-safe payload)."""
    return {
        "version": __version__,
    }


@app.get("/api/doctor/runtime")
def get_doctor_runtime():
    """Return server runtime diagnostics for authenticated troubleshooting."""
    return {
        "python_executable": sys.executable,
        "python_environment": summarize_python_environment(),
    }


@app.post("/api/desktop/shutdown")
async def post_desktop_shutdown(
    x_potato_desktop_shutdown_token: str | None = Header(default=None),
):
    """Gracefully stop the desktop sidecar before the Tauri app exits.

    The Tauri shell calls this on quit so uvicorn performs a normal shutdown
    (running the lifespan ``finally`` block that flushes memory/index) instead
    of being force-killed. Only available when running as the desktop sidecar.
    """
    from ..tauri.env import DESKTOP_APP_ENV, DESKTOP_SHUTDOWN_TOKEN_ENV

    expected_token = os.environ.get(DESKTOP_SHUTDOWN_TOKEN_ENV)
    if (
        os.environ.get(DESKTOP_APP_ENV) != "1"
        or not expected_token
        or x_potato_desktop_shutdown_token is None
        or not hmac.compare_digest(
            x_potato_desktop_shutdown_token,
            expected_token,
        )
    ):
        raise HTTPException(status_code=404, detail="Not Found")

    server = getattr(app.state, "uvicorn_server", None)
    if server is None:
        raise HTTPException(
            status_code=503,
            detail="Desktop backend is not ready",
        )

    server.should_exit = True
    return {"ok": True}


app.include_router(api_router, prefix="/api")

app.include_router(healthz_router, prefix="/api")

app.include_router(tool_calls_router, prefix="/api")

# Approval router: /api/approval/approve, /api/approval/deny, etc.
app.include_router(approval_router, prefix="/api")

# Coding Mode router: /api/coding-mode
app.include_router(coding_mode_router, prefix="/api")

# Loops router: /api/loops
app.include_router(loops_router, prefix="/api")

# Agent-scoped router: /api/agents/{agentId}/chats, etc.
agent_scoped_router = create_agent_scoped_router()
app.include_router(agent_scoped_router, prefix="/api")

# Voice channel: Twilio-facing endpoints at root level (not under /api/).
# POST /voice/incoming, WS /voice/ws, POST /voice/status-callback
app.include_router(voice_router, tags=["voice"])


# Console static files and SPA fallback
# Register these AFTER API routes to ensure proper routing priority
if os.path.isdir(_WEB_STATIC_DIR):
    _console_path = Path(_WEB_STATIC_DIR)

    def _serve_console_index():
        if _WEB_INDEX and _WEB_INDEX.exists():
            return FileResponse(
                _WEB_INDEX,
                headers=_INDEX_NO_CACHE_HEADERS,
            )

        raise HTTPException(status_code=404, detail="Not Found")

    _assets_dir = _console_path / "assets"
    if _assets_dir.is_dir():
        app.mount(
            "/assets",
            StaticFiles(directory=str(_assets_dir)),
            name="assets",
        )

    @app.get("/console")
    @app.get("/console/")
    @app.get("/console/{full_path:path}")
    def _console_spa_alias(full_path: str = ""):
        _ = full_path
        return _serve_console_index()

    # SPA fallback: catch-all route for frontend routing
    # Must be registered AFTER all API routes to avoid conflicts
    @app.get(
        "/{full_path:path}",
        name="potato_console_spa_catchall",
    )
    def _console_spa(full_path: str):
        # Prevent catching common system/special paths
        if full_path in ("docs", "redoc", "openapi.json"):
            raise HTTPException(status_code=404, detail="Not Found")
        # Skip API routes (should already be matched due to registration order)
        if full_path.startswith("api/") or full_path == "api":
            raise HTTPException(status_code=404, detail="Not Found")

        # Serve static files from the console build directory (e.g. logo SVGs,
        # favicons, images placed in public/).  Only serve regular files whose
        # path does not escape the console directory.
        if full_path and ".." not in full_path:
            # Security: Reject absolute paths to prevent path traversal bypass
            if not Path(full_path).is_absolute():
                static_file = _console_path / full_path
                if static_file.is_file():
                    return FileResponse(static_file)

        return _serve_console_index()
