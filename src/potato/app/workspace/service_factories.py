# -*- coding: utf-8 -*-
"""Service factory functions for workspace components.

Factory functions are used by Workspace._register_services() to create
and initialize service components. Extracted from local functions to
improve testability and code organization.
"""

import asyncio
import logging
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .workspace import Workspace

logger = logging.getLogger(__name__)


async def create_driver_service(ws: "Workspace", _service):
    """Create and initialize the per-workspace DriverManager.

    DriverManager is the runtime for external capabilities.  MCP is wired as
    the first concrete Driver protocol; legacy MCP config is migrated into
    DriverCard storage and is not exposed through the old MCP runtime path.
    """
    # pylint: disable=protected-access
    await asyncio.sleep(0)

    def _build_driver_manager():
        from ...drivers.adapters.mcp_legacy_config import (
            migrate_legacy_mcp_if_needed,
        )
        from ...drivers.credentials.store import AsyncCredentialStore
        from ...drivers.handlers import MCPDriverHandler
        from ...drivers.handlers.mcp import validate_mcp_endpoint
        from ...drivers.manager import DriverManager
        from ..approvals.driver_gate import PotatoDriverApprovalGate

        credential_store = AsyncCredentialStore(
            ws.workspace_dir / "credentials.yaml",
        )
        driver_manager = DriverManager(
            ws.workspace_dir / "drivers",
            credential_store,
            approval_gate=PotatoDriverApprovalGate(),
        )
        driver_manager.register_handler_type(
            "mcp",
            MCPDriverHandler,
            endpoint_validator=validate_mcp_endpoint,
        )
        return driver_manager, migrate_legacy_mcp_if_needed

    driver_manager, migrate_legacy_mcp_if_needed = await asyncio.to_thread(
        _build_driver_manager,
    )
    # Future Driver protocols should be registered here together with their
    # endpoint validator and tests.  This PR intentionally keeps the concrete
    # runtime surface to MCP while leaving DriverManager protocol-neutral.
    await migrate_legacy_mcp_if_needed(ws, driver_manager)
    await driver_manager.start()
    ws._service_manager.services["driver_manager"] = driver_manager
    logger.debug(
        "DriverManager external capability runtime initialized for agent: %s",
        ws.agent_id,
    )
    return driver_manager
    # pylint: enable=protected-access


async def create_driver_config_watcher(ws: "Workspace", _service):
    """Create watcher for manual DriverCard edits.

    Console/API updates call ``DriverConfigService.reload_driver_best_effort``
    immediately.  This watcher covers the manual-edit path and works for all
    Driver protocols instead of only MCP.
    """
    # pylint: disable=protected-access
    await asyncio.sleep(0)
    driver_manager = ws._service_manager.services.get("driver_manager")
    if driver_manager is None:
        return None

    def _build_watcher():
        from ..driver_config_watcher import DriverConfigWatcher

        return DriverConfigWatcher(
            driver_manager,
            ws.workspace_dir / "drivers",
        )

    watcher = await asyncio.to_thread(
        _build_watcher,
    )
    ws._service_manager.services["driver_config_watcher"] = watcher
    return watcher
    # pylint: enable=protected-access


async def create_chat_service(ws: "Workspace", service):
    """Create chat manager, or reuse existing one.

    Args:
        ws: Workspace instance
        service: Existing ChatManager if reused, None if creating new
    """
    # pylint: disable=protected-access
    await asyncio.sleep(0)

    if service is not None:
        cm = service
        logger.info(f"Reusing ChatManager for {ws.agent_id}")
    else:

        def _build_chat_manager():
            from ..chats.manager import ChatManager
            from ..chats.repo.json_repo import JsonChatRepository

            chats_path = str(ws.workspace_dir / "chats.json")
            chat_repo = JsonChatRepository(chats_path)
            return ChatManager(repo=chat_repo), chats_path

        cm, chats_path = await asyncio.to_thread(_build_chat_manager)
        ws._service_manager.services["chat_manager"] = cm
        logger.info(f"ChatManager created: {chats_path}")
    # pylint: enable=protected-access


async def create_channel_service(ws: "Workspace", _):
    """Create channel manager if configured.

    Args:
        ws: Workspace instance
        _: Unused service parameter

    Returns:
        ChannelManager instance or None if not configured
    """
    # pylint: disable=protected-access
    await asyncio.sleep(0)
    if not ws._config.channels:
        return None

    def _build_channel_manager():
        from ...config import Config, load_config, update_last_dispatch
        from ..channels.access_control import init_access_control_store
        from ..channels.manager import ChannelManager

        init_access_control_store(ws.workspace_dir)
        root_config = load_config()
        temp_config = Config(
            channels=ws._config.channels,
            show_tool_details=root_config.show_tool_details,
        )

        def on_last_dispatch(channel, user_id, session_id):
            update_last_dispatch(
                channel=channel,
                user_id=user_id,
                session_id=session_id,
                agent_id=ws.agent_id,
            )

        return ChannelManager.from_config(
            process=ws.stream_query,
            config=temp_config,
            on_last_dispatch=on_last_dispatch,
            workspace_dir=ws.workspace_dir,
        )

    cm = await asyncio.to_thread(
        _build_channel_manager,
    )
    ws._service_manager.services["channel_manager"] = cm

    cm.set_workspace(ws)
    from ..approvals import get_approval_service

    get_approval_service().set_channel_manager(cm, agent_id=ws.agent_id)

    agent_language = getattr(ws._config, "language", "zh") or "zh"
    for ch in cm.channels:
        ch._language = agent_language

    return cm
    # pylint: enable=protected-access


async def create_agent_config_watcher(ws: "Workspace", _):
    """Create agent config watcher if channel/cron exists.

    The watcher only triggers reloads via ``MultiAgentManager`` and
    does not need direct references to channel/cron managers anymore.
    Creation is still gated on having at least one of them, since
    workspaces with neither have no externally-visible state that
    benefits from auto-reload.

    Args:
        ws: Workspace instance
        _: Unused service parameter

    Returns:
        AgentConfigWatcher instance or None if not needed
    """
    # pylint: disable=protected-access
    await asyncio.sleep(0)
    channel_mgr = ws._service_manager.services.get("channel_manager")
    cron_mgr = ws._service_manager.services.get("cron_manager")

    if not (channel_mgr or cron_mgr):
        return None

    def _build_watcher():
        from ..agent_config_watcher import AgentConfigWatcher

        return AgentConfigWatcher(
            agent_id=ws.agent_id,
            workspace_dir=ws.workspace_dir,
            workspace=ws,
        )

    watcher = await asyncio.to_thread(
        _build_watcher,
    )
    ws._service_manager.services["agent_config_watcher"] = watcher
    return watcher
    # pylint: enable=protected-access
