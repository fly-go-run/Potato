# -*- coding: utf-8 -*-
# pylint: disable=redefined-outer-name,protected-access
"""Tests for ReMeLightMemoryManager auto memory search behavior."""
from types import SimpleNamespace
from unittest.mock import AsyncMock, patch

import pytest
from agentscope.message import Msg, TextBlock

from potato.agents.memory.reme_light_memory_manager import (
    ReMeLightMemoryManager,
)
from potato.config.config import (
    AgentProfileConfig,
    AgentsRunningConfig,
    AutoMemorySearchConfig,
    ReMeLightMemoryConfig,
)


@pytest.fixture
def manager():
    """A manager instance without the embedded ReMe app."""
    mgr = object.__new__(ReMeLightMemoryManager)
    mgr.working_dir = "/tmp/potato-agent"
    mgr.agent_id = "agent-1"
    mgr._reme = None
    return mgr


def _agent_config(enabled: bool = True) -> AgentProfileConfig:
    return AgentProfileConfig(
        id="agent-1",
        name="Agent One",
        running=AgentsRunningConfig(
            reme_light_memory_config=ReMeLightMemoryConfig(
                auto_memory_search_config=AutoMemorySearchConfig(
                    enabled=enabled,
                    max_results=2,
                ),
            ),
        ),
    )


def _user_msg(text: str = "what did we decide about the database?") -> Msg:
    return Msg(name="user", role="user", content=[TextBlock(text=text)])


class TestAutoMemorySearchDedup:
    @pytest.mark.asyncio
    async def test_passes_session_scoped_tool_context_id(self, manager):
        job = AsyncMock(
            return_value=SimpleNamespace(success=True, answer="hit"),
        )
        with (
            patch.object(manager, "_run_reme_job", job),
            patch(
                "potato.agents.memory.reme_light_memory_manager"
                ".load_agent_config",
                return_value=_agent_config(),
            ),
        ):
            result = await manager.auto_memory_search(
                _user_msg(),
                session_id="sess-42",
            )

        assert result is not None
        job.assert_awaited_once()
        assert (
            job.await_args.kwargs["tool_context_id"]
            == "auto_memory_search:sess-42"
        )

    @pytest.mark.asyncio
    async def test_empty_session_id_disables_dedup_scope(self, manager):
        job = AsyncMock(
            return_value=SimpleNamespace(success=True, answer="hit"),
        )
        with (
            patch.object(manager, "_run_reme_job", job),
            patch(
                "potato.agents.memory.reme_light_memory_manager"
                ".load_agent_config",
                return_value=_agent_config(),
            ),
        ):
            await manager.auto_memory_search(_user_msg())

        assert job.await_args.kwargs["tool_context_id"] == ""

    @pytest.mark.asyncio
    async def test_all_deduped_answer_is_not_injected(self, manager):
        job = AsyncMock(
            return_value=SimpleNamespace(success=True, answer=""),
        )
        with (
            patch.object(manager, "_run_reme_job", job),
            patch(
                "potato.agents.memory.reme_light_memory_manager"
                ".load_agent_config",
                return_value=_agent_config(),
            ),
        ):
            result = await manager.auto_memory_search(
                _user_msg(),
                session_id="sess-42",
            )

        assert result is None

    def test_reset_drops_session_bucket(self, manager):
        contexts = {
            "auto_memory_search:sess-42": {"search_seen_chunk_ids": {"c": 1}},
            "auto_memory_search:other": {"search_seen_chunk_ids": {"d": 1}},
        }
        manager._reme = SimpleNamespace(
            context=SimpleNamespace(metadata={"tool_contexts": contexts}),
        )

        manager.reset_auto_search_dedup("sess-42")

        assert "auto_memory_search:sess-42" not in contexts
        assert "auto_memory_search:other" in contexts

    def test_reset_without_reme_is_noop(self, manager):
        manager.reset_auto_search_dedup("sess-42")

    def test_prune_drops_expired_and_empty_buckets(self, manager):
        now = 1_000_000.0
        day = 24 * 60 * 60
        contexts = {
            "auto_memory_search:stale": {
                "search_seen_chunk_ids": {"a": now - day - 1},
            },
            "auto_memory_search:fresh": {
                "search_seen_chunk_ids": {"b": now - 60},
            },
            "auto_memory_search:empty": {"search_seen_chunk_ids": {}},
            "auto_memory_search:corrupt": {
                "search_seen_chunk_ids": {"c": "not-a-ts"},
            },
            "other_tool_ctx": {"search_seen_chunk_ids": {"d": 0}},
        }
        manager._reme = SimpleNamespace(
            context=SimpleNamespace(metadata={"tool_contexts": contexts}),
        )

        manager._prune_auto_search_dedup(now)

        assert set(contexts) == {"auto_memory_search:fresh", "other_tool_ctx"}

    @pytest.mark.asyncio
    async def test_disabled_config_skips_search(self, manager):
        job = AsyncMock()
        with (
            patch.object(manager, "_run_reme_job", job),
            patch(
                "potato.agents.memory.reme_light_memory_manager"
                ".load_agent_config",
                return_value=_agent_config(enabled=False),
            ),
        ):
            result = await manager.auto_memory_search(
                _user_msg(),
                session_id="sess-42",
            )

        assert result is None
        job.assert_not_awaited()
