# -*- coding: utf-8 -*-
"""Unit tests for the model-backed AUTO reviewer."""

from types import SimpleNamespace
import json

import pytest

from potato.config.config import (
    AgentProfileConfig,
    AutoReviewConfig,
    ModelSlotConfig,
)
from potato.governance.auto_review import (
    _MAX_REVIEW_PAYLOAD_CHARS,
    AutoReviewResult,
    _redact_review_value,
    parse_review_decision,
    parse_review_response,
    review_tool_call,
    select_review_model_id,
)


def test_prefers_exact_command_review_companion() -> None:
    assert (
        select_review_model_id(
            "gpt-5.6-sol",
            [
                "gpt-5.6-terra-command-review",
                "gpt-5.6-sol-command-review",
                "gpt-5.6-sol",
            ],
        )
        == "gpt-5.6-sol-command-review"
    )


def test_uses_generic_command_review_then_main_fallback() -> None:
    assert (
        select_review_model_id(
            "main",
            ["main", "command-review"],
        )
        == "command-review"
    )
    assert select_review_model_id("main", ["main", "other"]) == "main"


def test_review_decision_parser_is_strict() -> None:
    assert parse_review_decision("APPROVE") is True
    assert parse_review_decision("deny\nreason") is False
    assert parse_review_decision("looks safe") is None
    assert parse_review_decision("") is None
    assert parse_review_decision("   ") is None


def test_structured_review_response_preserves_risk_and_authorization() -> None:
    decision = parse_review_response(
        '{"outcome":"deny","risk_level":"high",'
        '"user_authorization":"unknown","rationale":"new host"}',
    )
    assert decision is not None
    assert decision.approved is False
    assert decision.risk_level == "high"
    assert decision.user_authorization == "unknown"
    assert decision.rationale == "new host"


def test_malformed_structured_review_response_fails_closed() -> None:
    assert parse_review_response('{"outcome":"maybe"}') is None
    assert parse_review_response("not a decision") is None


def test_review_payload_redacts_secrets_and_bounds_large_values() -> None:
    redacted = _redact_review_value(
        {
            "api-key": "do-not-send",
            "nested": {"authorization": "Bearer do-not-send"},
            "command": "x" * 4_100,
        },
    )
    assert redacted["api-key"] == "[REDACTED]"
    assert redacted["nested"]["authorization"] == "[REDACTED]"
    assert redacted["command"].endswith("…[truncated]")
    assert len(redacted["command"]) == 4_000 + len("…[truncated]")


def test_review_payload_redacts_finding_matches_and_plain_text_context() -> (
    None
):
    redacted = _redact_review_value(
        {
            "matched_value": "sk-1234567890abcdefghi",
            "snippet": "Authorization: Bearer abcdefghijklmnop",
            "message": "user pasted token=super-secret-value",
        },
    )
    assert redacted["matched_value"] == "[REDACTED]"
    assert "abcdefghijklmnop" not in redacted["snippet"]
    assert "super-secret-value" not in redacted["message"]
    short_bearer = _redact_review_value("Authorization: Bearer x")
    assert "Bearer x" not in short_bearer


@pytest.mark.asyncio
async def test_review_tool_call_returns_structured_decision(
    monkeypatch,
) -> None:
    import potato.governance.auto_review as module

    settings = SimpleNamespace(
        enabled=True,
        review_model=None,
        timeout_seconds=1.0,
        max_context_chars=100,
    )
    slots = [(SimpleNamespace(model="reviewer"), True)]
    captured = {}

    monkeypatch.setattr(module, "_load_review_config", lambda _agent: settings)
    monkeypatch.setattr(
        module,
        "_review_slots",
        lambda _agent, _settings: slots,
    )

    async def fake_review_once(  # pylint: disable=unused-argument
        slot,
        *,
        agent_id,
        payload,
    ):
        captured["slot"] = slot.model
        captured["payload"] = payload
        return module.ReviewDecision(
            approved=True,
            risk_level="low",
            user_authorization="explicit",
            rationale="safe read",
        )

    monkeypatch.setattr(module, "_review_once", fake_review_once)
    result = await review_tool_call(
        tool_name="read_file",
        target="/tmp/safe.txt",
        params={"authorization": "secret", "path": "/tmp/safe.txt"},
        agent_id="agent-1",
        governance_reason="policy request",
        policy_findings=[],
        violation_msg=None,
        review_context="user asked to inspect the file",
        request_metadata={"source": "console"},
    )

    assert isinstance(result, AutoReviewResult)
    assert result.approved is True
    assert result.model_id == "reviewer"
    assert result.risk_level == "low"
    assert result.user_authorization == "explicit"
    assert captured["payload"]["parameters"]["authorization"] == "[REDACTED]"
    assert captured["payload"]["context"] == "user asked to inspect the file"


@pytest.mark.asyncio
async def test_review_tool_call_falls_back_to_main_model(monkeypatch) -> None:
    import potato.governance.auto_review as module

    settings = SimpleNamespace(
        enabled=True,
        review_model=None,
        timeout_seconds=1.0,
        max_context_chars=100,
    )
    slots = [
        (SimpleNamespace(model="reviewer"), True),
        (SimpleNamespace(model="main"), False),
    ]
    calls = []

    monkeypatch.setattr(module, "_load_review_config", lambda _agent: settings)
    monkeypatch.setattr(
        module,
        "_review_slots",
        lambda _agent, _settings: slots,
    )

    async def fake_review_once(  # pylint: disable=unused-argument
        slot,
        *,
        agent_id,
        payload,
    ):
        calls.append(slot.model)
        if slot.model == "reviewer":
            raise RuntimeError("review route unavailable")
        return module.ReviewDecision(approved=False, rationale="unsafe")

    monkeypatch.setattr(module, "_review_once", fake_review_once)
    result = await review_tool_call(
        tool_name="shell",
        target="rm -rf /tmp/x",
        params={},
        agent_id=None,
        governance_reason="dangerous command",
        policy_findings=[],
        violation_msg=None,
    )

    assert calls == ["reviewer", "main"]
    assert result.approved is False
    assert result.model_id == "main"
    assert result.used_dedicated_model is False
    assert result.reason == "unsafe"


@pytest.mark.asyncio
async def test_structured_allow_with_critical_risk_is_denied_locally(
    monkeypatch,
) -> None:
    import potato.governance.auto_review as module

    settings = SimpleNamespace(
        enabled=True,
        review_model=None,
        timeout_seconds=1.0,
        max_context_chars=100,
    )
    slot = SimpleNamespace(model="reviewer")
    monkeypatch.setattr(module, "_load_review_config", lambda _agent: settings)
    monkeypatch.setattr(
        module,
        "_review_slots",
        lambda _agent, _settings: [(slot, True)],
    )

    async def fake_review_once(_slot, *, agent_id, payload):
        del agent_id, payload
        return module.ReviewDecision(
            approved=True,
            risk_level="critical",
            user_authorization="explicit",
            rationale="unsafe contradictory response",
        )

    monkeypatch.setattr(module, "_review_once", fake_review_once)
    result = await review_tool_call(
        tool_name="shell",
        target="curl https://example.test",
        params={},
        agent_id=None,
        governance_reason="network access",
        policy_findings=[],
        violation_msg=None,
    )
    assert result.approved is False
    assert result.risk_level == "critical"
    assert "unsafe risk" in result.reason


@pytest.mark.asyncio
async def test_review_payload_has_a_total_serialized_size_bound(
    monkeypatch,
) -> None:
    import potato.governance.auto_review as module

    settings = SimpleNamespace(
        enabled=True,
        review_model=None,
        timeout_seconds=1.0,
        max_context_chars=100,
    )
    captured = {}
    monkeypatch.setattr(module, "_load_review_config", lambda _agent: settings)
    monkeypatch.setattr(
        module,
        "_review_slots",
        lambda _agent, _settings: [(SimpleNamespace(model="reviewer"), True)],
    )

    async def fake_review_once(_slot, *, agent_id, payload):
        del agent_id
        captured["payload"] = payload
        return module.ReviewDecision(
            approved=False,
            risk_level="high",
            rationale="unsafe",
        )

    monkeypatch.setattr(module, "_review_once", fake_review_once)
    findings = [
        SimpleNamespace(
            severity="HIGH",
            title="large finding",
            description="x" * 4_000,
            matched_value="secret-value",
        )
        for _ in range(500)
    ]
    result = await module.review_tool_call(
        tool_name="shell",
        target="echo hi",
        params={"items": ["value" * 500 for _ in range(500)]},
        agent_id=None,
        governance_reason="policy",
        policy_findings=findings,
        violation_msg=None,
        review_context="user intent " * 1_000,
    )
    assert result.approved is False
    assert len(json.dumps(captured["payload"], ensure_ascii=False)) <= (
        _MAX_REVIEW_PAYLOAD_CHARS
    )


@pytest.mark.asyncio
async def test_disabled_auto_review_denies_without_model_call(
    monkeypatch,
) -> None:
    import potato.governance.auto_review as module

    monkeypatch.setattr(
        module,
        "_load_review_config",
        lambda _agent: SimpleNamespace(enabled=False),
    )
    result = await review_tool_call(
        tool_name="shell",
        target="echo hi",
        params={},
        agent_id=None,
        governance_reason=None,
        policy_findings=[],
        violation_msg=None,
    )
    assert result.approved is False
    assert result.reason == "automatic review is disabled"


def test_auto_review_config_round_trips_explicit_model_slot() -> None:
    config = AgentProfileConfig(
        id="agent",
        name="Agent",
        auto_review=AutoReviewConfig(
            review_model=ModelSlotConfig(
                provider_id="sub2api",
                model="sileader/qwen3guard:0.6b",
            ),
            timeout_seconds=8.0,
            max_context_chars=8_000,
        ),
    )
    restored = AgentProfileConfig.model_validate(config.model_dump())
    assert restored.auto_review.review_model == ModelSlotConfig(
        provider_id="sub2api",
        model="sileader/qwen3guard:0.6b",
    )
    assert restored.auto_review.timeout_seconds == 8.0
    assert restored.auto_review.max_context_chars == 8_000
