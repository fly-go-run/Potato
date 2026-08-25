# -*- coding: utf-8 -*-
# pylint: disable=protected-access,unused-argument
"""Tests for Scroll's successful-model-input acknowledgement hook."""
from types import SimpleNamespace

import pytest
from agentscope.agent import Agent
from agentscope.event import ModelCallEndEvent
from agentscope.message import HintBlock, Msg, TextBlock
from agentscope.model import FinishedReason

from potato.agents.context.types import (
    ContextWindowUnfitError,
    is_context_window_error,
)
from potato.agents.react_agent import PotatoAgent
from potato.loop.gates import StopAction, StopHandlerResult


def test_is_context_window_error_markers():
    assert is_context_window_error(
        RuntimeError("This model's maximum context length was exceeded"),
    )
    assert is_context_window_error(RuntimeError("prompt is too long"))
    assert is_context_window_error(
        ContextWindowUnfitError(tokens=10, hard_limit=8),
    )
    assert not is_context_window_error(RuntimeError("invalid image_url"))


class SeenTracker:
    """Minimal Scroll manager surface used by ``PotatoAgent._reasoning``."""

    def __init__(self) -> None:
        self.acknowledged: list[set[str]] = []

    @staticmethod
    def model_input_tool_result_ids(agent) -> set[str]:
        return {"call-seen"}

    def acknowledge_model_input_tool_results(self, ids: set[str]) -> None:
        self.acknowledged.append(set(ids))


class CompressionTracker:
    """Capture context-manager compression delegation arguments."""

    def __init__(self) -> None:
        self.calls = []

    async def compress(self, agent, context_config=None, instructions=None):
        self.calls.append((agent, context_config, instructions))


def make_agent(tracker: SeenTracker) -> PotatoAgent:
    """Build only the attributes the reasoning wrapper reads."""
    agent = object.__new__(PotatoAgent)
    agent._context_manager = tracker
    agent._gate_pending_stop = None
    agent.model = SimpleNamespace(model_key=None)

    async def stop_handlers(final_msg):
        return StopHandlerResult(
            action=StopAction.TERMINATE,
            final_message=final_msg,
        )

    agent._run_stop_handlers = stop_handlers
    return agent


async def test_successful_model_call_acknowledges_input_results(monkeypatch):
    tracker = SeenTracker()
    agent = make_agent(tracker)

    async def successful_reasoning(self, tool_choice=None):
        yield ModelCallEndEvent(
            reply_id="reply",
            input_tokens=10,
            output_tokens=2,
            finished_reason=FinishedReason.COMPLETED,
        )
        yield Msg(
            name="agent",
            role="assistant",
            content=[TextBlock(type="text", text="done")],
        )

    monkeypatch.setattr(Agent, "_reasoning", successful_reasoning)
    events = [event async for event in agent._reasoning()]

    assert tracker.acknowledged == [{"call-seen"}]
    assert isinstance(events[-1], Msg)


async def test_failed_model_call_does_not_acknowledge_results(monkeypatch):
    tracker = SeenTracker()
    agent = make_agent(tracker)

    async def failed_reasoning(self, tool_choice=None):
        if tool_choice == "unreachable-test-sentinel":
            yield None
        raise RuntimeError("provider rejected request")

    monkeypatch.setattr(Agent, "_reasoning", failed_reasoning)

    with pytest.raises(RuntimeError, match="provider rejected request"):
        async for _ in agent._reasoning():
            pass

    assert not tracker.acknowledged


async def test_interrupted_model_call_does_not_acknowledge_results(
    monkeypatch,
):
    tracker = SeenTracker()
    agent = make_agent(tracker)

    async def interrupted_reasoning(self, tool_choice=None):
        yield ModelCallEndEvent(
            reply_id="reply",
            input_tokens=10,
            output_tokens=0,
            finished_reason=FinishedReason.INTERRUPTED,
        )

    monkeypatch.setattr(Agent, "_reasoning", interrupted_reasoning)
    events = [event async for event in agent._reasoning()]

    assert len(events) == 1
    assert not tracker.acknowledged


async def test_context_window_error_forces_compact_and_retries(monkeypatch):
    tracker = CompressionTracker()
    agent = make_agent(SeenTracker())
    agent._context_manager = tracker
    agent.context_config = SimpleNamespace(
        model_copy=lambda update: SimpleNamespace(
            trigger_ratio=update["trigger_ratio"],
        ),
    )
    agent.state = SimpleNamespace(context=[])
    calls = {"n": 0}

    async def flaky_reasoning(self, tool_choice=None):
        calls["n"] += 1
        if calls["n"] == 1:
            raise RuntimeError(
                "This model's maximum context length was exceeded",
            )
        yield ModelCallEndEvent(
            reply_id="reply",
            input_tokens=10,
            output_tokens=2,
            finished_reason=FinishedReason.COMPLETED,
        )
        yield Msg(
            name="agent",
            role="assistant",
            content=[TextBlock(type="text", text="recovered")],
        )

    monkeypatch.setattr(Agent, "_reasoning", flaky_reasoning)
    events = [event async for event in agent._reasoning()]

    assert tracker.calls
    assert tracker.calls[0][1].trigger_ratio == 1e-6
    assert isinstance(events[-1], Msg)
    assert events[-1].content[0].text == "recovered"


async def test_context_window_error_second_failure_is_unfit(monkeypatch):
    agent = make_agent(SeenTracker())
    agent._context_manager = CompressionTracker()
    agent.context_config = None
    agent.state = SimpleNamespace(context=[])
    agent.model = SimpleNamespace(context_size=128000, model_key=None)

    async def always_overflow(self, tool_choice=None):
        if tool_choice == "unreachable-test-sentinel":
            yield None
        raise RuntimeError("prompt is too long")

    monkeypatch.setattr(Agent, "_reasoning", always_overflow)

    with pytest.raises(ContextWindowUnfitError):
        async for _ in agent._reasoning():
            pass


async def test_compress_context_forwards_one_shot_instructions():
    tracker = CompressionTracker()
    agent = object.__new__(PotatoAgent)
    agent._context_manager = tracker
    agent.state = SimpleNamespace(context=[])
    config = SimpleNamespace(trigger_ratio=0.1)
    instructions = HintBlock(hint="prioritize failures", source="user")

    await agent.compress_context(config, instructions=instructions)

    assert tracker.calls == [(agent, config, instructions)]
