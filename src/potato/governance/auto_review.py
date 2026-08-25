# -*- coding: utf-8 -*-
"""Bounded model review for AUTO tool-approval mode.

This is the small, provider-agnostic part of Codex's Guardian design:
governance stays local, while a fresh model request evaluates one concrete
tool action.  The reviewer receives a bounded, redacted request and has no
tools or approval API of its own.  It can only return a decision.

The explicit ``AgentProfileConfig.auto_review.review_model`` slot is preferred
when configured.  For backwards compatibility, provider-advertised
``*-command-review``/``*-review`` companions are discovered next, and the
active chat model is the final fallback.

Review is deliberately fail-closed and time-bounded: malformed output,
missing models, and API errors resolve to DENY rather than opening a human
approval wait that can stall an otherwise unattended AUTO task.
"""

from __future__ import annotations

import asyncio
import json
import logging
import re
from dataclasses import dataclass
from typing import Any, Iterable, Mapping

logger = logging.getLogger(__name__)

AUTO_REVIEW_TIMEOUT_SECONDS = 12.0
_REVIEW_MARKERS = (
    "command-review",
    "command_review",
    "commandreview",
    "auto-review",
    "auto_review",
)
_SECRET_KEY_MARKERS = (
    "api_key",
    "apikey",
    "access_token",
    "auth_token",
    "authorization",
    "cookie",
    "credential",
    "password",
    "secret",
    "token",
    # Security detector findings use these generic field names for the exact
    # text that triggered a rule.  Treat them as sensitive even though the
    # field name itself does not identify the credential type.
    "matched_value",
    "matchedvalue",
    "snippet",
)
_MAX_REVIEW_VALUE_CHARS = 4_000
_MAX_REVIEW_COLLECTION_ITEMS = 128
_MAX_REVIEW_PAYLOAD_CHARS = 32_000
_MAX_RATIONALE_CHARS = 1_000
_KNOWN_RISK_LEVELS = {"low", "medium", "high", "critical", "unknown"}
_KNOWN_AUTH_LEVELS = {
    "explicit",
    "implied",
    "unknown",
    "none",
}

# ``review_context`` is commonly a plain user-message string, so it does not
# carry a key that the recursive key-based redactor can inspect.  Scrub the
# common credential forms in *all* text values before the payload is sent to a
# reviewer.  This is intentionally small and conservative; the structural
# key-based redaction above remains the primary defence for arbitrary fields.
_SECRET_TEXT_PATTERNS = (
    (
        re.compile(
            # Keep the lower bound at one character: short bearer tokens are
            # uncommon but still credentials, and review context is not a
            # place to make a length-based exception.
            r"(?i)(\bbearer\s+)[A-Za-z0-9._~+/=-]{1,}",
        ),
        r"\1[REDACTED]",
    ),
    (
        re.compile(
            r"(?i)(\b(?:api[_-]?key|access[_-]?token|auth(?:orization)?|"
            r"password|secret|token)\b\s*[:=]\s*)[^\s,;]+",
        ),
        r"\1[REDACTED]",
    ),
    (
        re.compile(
            r"\b(?:sk|rk|pk|ghp|github_pat|xox[baprs]-)[A-Za-z0-9_-]{12,}\b",
        ),
        "[REDACTED]",
    ),
)

_SYSTEM_PROMPT = """You are a conservative command and tool-call reviewer.
You have no tools and cannot ask for permission. Review only the supplied
action, findings, and bounded user context.

Return exactly one JSON object and no markdown:
{
  "outcome": "allow", "deny", or "require_human",
  "risk_level": "low", "medium", "high", "critical", or "unknown",
  "user_authorization": "explicit", "implied", "unknown", or "none",
  "rationale": "short explanation"
}

Allow only when the requested tool call is clearly in scope, routine,
reversible, and does not expose secrets or materially expand access.
Use require_human when risk or user authorization is not clearly
established. Deny destructive deletion, privilege escalation, credential
access, exfiltration, obfuscation, sandbox escape, ambiguous targets,
or anything whose safety cannot be established from the supplied facts.
Bare APPROVE/DENY text is not a valid decision.
"""


@dataclass(frozen=True)
class ReviewDecision:
    """Parsed model decision before it is recorded by governance."""

    approved: bool
    require_human: bool = False
    risk_level: str = "unknown"
    user_authorization: str = "unknown"
    rationale: str = ""


@dataclass(frozen=True)
class AutoReviewResult:
    """Resolved AUTO review outcome and the model used for auditing."""

    approved: bool
    model_id: str
    used_dedicated_model: bool
    reason: str
    risk_level: str = "unknown"
    user_authorization: str = "unknown"
    rationale: str = ""
    require_human: bool = False


def select_review_model_id(
    main_model_id: str,
    available_model_ids: Iterable[str],
) -> str:
    """Return a provider-advertised review model, else ``main_model_id``.

    Exact companions win over generic reviewer entries so multi-family
    gateways do not accidentally review one model family's command with
    another family's hidden policy model.
    """
    main = str(main_model_id or "").strip()
    available = [str(item or "").strip() for item in available_model_ids]
    available = [item for item in available if item]
    by_lower = {item.lower(): item for item in available}

    for suffix in ("command-review", "command_review", "review"):
        candidate = f"{main}-{suffix}".lower()
        if candidate in by_lower:
            return by_lower[candidate]

    for item in available:
        lowered = item.lower()
        if main.lower() in lowered and any(
            marker in lowered for marker in _REVIEW_MARKERS
        ):
            return item

    for item in available:
        lowered = item.lower()
        if any(marker in lowered for marker in _REVIEW_MARKERS):
            return item
    return main


def _normalise_choice(value: Any, allowed: set[str]) -> str:
    choice = str(value or "").strip().lower()
    return choice if choice in allowed else "unknown"


def parse_review_response(  # pylint: disable=too-many-return-statements
    text: str,
) -> ReviewDecision | None:
    """Parse a structured reviewer response.

    Bare ``APPROVE``/``DENY`` tokens are rejected. Empty or malformed
    output returns ``None`` so the caller can fail closed.
    """
    raw = str(text or "").strip()
    if not raw:
        return None

    decoded: Any = None
    if raw.startswith("{"):
        try:
            decoded = json.loads(raw)
        except (TypeError, ValueError, json.JSONDecodeError):
            decoded = None
    if not isinstance(decoded, dict):
        return None
    outcome = str(decoded.get("outcome") or "").strip().lower()
    rationale = str(decoded.get("rationale") or "").strip()
    risk_level = _normalise_choice(
        decoded.get("risk_level"),
        _KNOWN_RISK_LEVELS,
    )
    user_authorization = _normalise_choice(
        decoded.get("user_authorization"),
        _KNOWN_AUTH_LEVELS,
    )
    if outcome in {"require_human", "ask", "human"}:
        return ReviewDecision(
            approved=False,
            require_human=True,
            risk_level=risk_level,
            user_authorization=user_authorization,
            rationale=rationale[:_MAX_RATIONALE_CHARS],
        )
    if outcome in {"allow", "approve", "approved"}:
        approved = True
    elif outcome in {"deny", "denied", "block", "blocked"}:
        approved = False
    else:
        return None
    return ReviewDecision(
        approved=approved,
        risk_level=risk_level,
        user_authorization=user_authorization,
        rationale=rationale[:_MAX_RATIONALE_CHARS],
    )


def parse_review_decision(text: str) -> bool | None:
    """Backward-compatible boolean parser used by existing callers/tests."""
    parsed = parse_review_response(text)
    return parsed.approved if parsed is not None else None


def _redact_review_value(value: Any, *, key: str = "") -> Any:
    """Keep the review prompt useful without forwarding credentials.

    Tool parameters can contain uploaded headers, bearer tokens, or provider
    credentials. The reviewer only needs the shape of those values, never the
    secret itself. Bound strings as well so a large file/blob cannot turn an
    approval request into an unbounded second context.
    """
    lowered_key = key.lower().replace("-", "_")
    if any(marker in lowered_key for marker in _SECRET_KEY_MARKERS):
        return "[REDACTED]"
    if isinstance(value, Mapping):
        items = list(value.items())
        redacted = {
            str(child_key): _redact_review_value(
                child_value,
                key=str(child_key),
            )
            for child_key, child_value in items[:_MAX_REVIEW_COLLECTION_ITEMS]
        }
        if len(items) > _MAX_REVIEW_COLLECTION_ITEMS:
            redacted[
                "__truncated__"
            ] = f"{len(items) - _MAX_REVIEW_COLLECTION_ITEMS} items omitted"
        return redacted
    if isinstance(value, (list, tuple)):
        redacted = [
            _redact_review_value(item)
            for item in value[:_MAX_REVIEW_COLLECTION_ITEMS]
        ]
        if len(value) > _MAX_REVIEW_COLLECTION_ITEMS:
            redacted.append(
                (
                    f"…[{len(value) - _MAX_REVIEW_COLLECTION_ITEMS} "
                    "items omitted]"
                ),
            )
        return redacted
    if isinstance(value, str):
        redacted_text = value
        for pattern, replacement in _SECRET_TEXT_PATTERNS:
            redacted_text = pattern.sub(replacement, redacted_text)
        if len(redacted_text) > _MAX_REVIEW_VALUE_CHARS:
            return redacted_text[:_MAX_REVIEW_VALUE_CHARS] + "…[truncated]"
        return redacted_text
    return value


def _bounded_review_context(value: Any, max_chars: int) -> Any:
    """Redact and bound optional context before serialising the payload."""
    redacted = _redact_review_value(value)
    if isinstance(redacted, str):
        return redacted[:max_chars] + (
            "…[truncated]" if len(redacted) > max_chars else ""
        )
    try:
        encoded = json.dumps(redacted, ensure_ascii=False, default=str)
    except (TypeError, ValueError):
        encoded = str(redacted)
    if len(encoded) <= max_chars:
        return redacted
    return encoded[:max_chars] + "…[truncated]"


def _json_char_count(value: Any) -> int:
    """Return serialized size without allowing exotic values to escape."""
    try:
        return len(json.dumps(value, ensure_ascii=False, default=str))
    except (TypeError, ValueError):
        return len(str(value))


def _compact_review_value(value: Any, max_chars: int) -> Any:
    """Bound an already-redacted value while preserving its shape.

    The key-based redactor caps individual strings and collection cardinality,
    but a payload can still contain many medium-sized fields.  This second
    pass enforces a serialized budget without turning the complete reviewer
    request into an unbounded model context.
    """
    max_chars = max(64, int(max_chars))
    if isinstance(value, str):
        return value[:max_chars] + (
            "…[truncated]" if len(value) > max_chars else ""
        )
    if isinstance(value, Mapping):
        compacted: dict[str, Any] = {}
        for key, child in list(value.items())[:_MAX_REVIEW_COLLECTION_ITEMS]:
            key_text = str(key)[:256]
            compacted[key_text] = _compact_review_value(
                child,
                max_chars // max(1, len(compacted) + 1),
            )
            if _json_char_count(compacted) > max_chars:
                compacted.pop(key_text, None)
                break
        if len(compacted) < len(value):
            compacted["__truncated__"] = "additional values omitted"
            while _json_char_count(compacted) > max_chars and compacted:
                compacted.pop(next(reversed(compacted)))
        return compacted
    if isinstance(value, (list, tuple)):
        compacted_list: list[Any] = []
        for child in list(value)[:_MAX_REVIEW_COLLECTION_ITEMS]:
            compacted_list.append(
                _compact_review_value(
                    child,
                    max_chars // max(1, len(compacted_list) + 1),
                ),
            )
            if _json_char_count(compacted_list) > max_chars:
                compacted_list.pop()
                break
        if len(compacted_list) < len(value):
            compacted_list.append("…[additional values omitted]")
            while (
                _json_char_count(compacted_list) > max_chars and compacted_list
            ):
                compacted_list.pop(-2 if len(compacted_list) > 1 else -1)
        return compacted_list
    return value


def _bound_review_payload(payload: dict[str, Any]) -> dict[str, Any]:
    """Apply the final serialized budget to one reviewer request.

    Action identity and parameters are retained first. Optional context,
    metadata, and findings are compacted aggressively before the fallback
    model receives a request with an unexpectedly large prompt.
    """
    bounded = dict(payload)
    field_budgets = {
        "parameters": _MAX_REVIEW_PAYLOAD_CHARS // 2,
        "findings": _MAX_REVIEW_PAYLOAD_CHARS // 5,
        "context": _MAX_REVIEW_PAYLOAD_CHARS // 8,
        "request_metadata": _MAX_REVIEW_PAYLOAD_CHARS // 16,
    }
    for field, budget in field_budgets.items():
        if field in bounded:
            bounded[field] = _compact_review_value(bounded[field], budget)

    # Shrink optional fields in order until the complete JSON object fits.
    if _json_char_count(bounded) > _MAX_REVIEW_PAYLOAD_CHARS:
        for field in (
            "context",
            "request_metadata",
            "findings",
            "governance_reason",
            "sandbox_violation",
        ):
            if field in bounded:
                bounded[field] = "[truncated]"
            if _json_char_count(bounded) <= _MAX_REVIEW_PAYLOAD_CHARS:
                return bounded

    if _json_char_count(bounded) > _MAX_REVIEW_PAYLOAD_CHARS:
        bounded["parameters"] = _compact_review_value(
            bounded.get("parameters", {}),
            _MAX_REVIEW_PAYLOAD_CHARS // 3,
        )
    if _json_char_count(bounded) > _MAX_REVIEW_PAYLOAD_CHARS:
        bounded["target"] = _compact_review_value(
            bounded.get("target", ""),
            _MAX_REVIEW_PAYLOAD_CHARS // 8,
        )
    if _json_char_count(bounded) > _MAX_REVIEW_PAYLOAD_CHARS:
        # The fields above are now bounded independently; retain the action
        # identity and a compact parameter summary as the final invariant.
        bounded = {
            "tool_name": _compact_review_value(
                payload.get("tool_name", ""),
                512,
            ),
            "target": _compact_review_value(
                payload.get("target", ""),
                _MAX_REVIEW_PAYLOAD_CHARS // 8,
            ),
            "parameters": _compact_review_value(
                payload.get("parameters", {}),
                _MAX_REVIEW_PAYLOAD_CHARS // 2,
            ),
            "findings": "[truncated]",
            "context": "[truncated]",
        }
    return bounded


def _load_review_config(agent_id: str | None) -> Any:
    from ..config.config import AutoReviewConfig

    if agent_id:
        try:
            from ..config.config import load_agent_config

            config = getattr(load_agent_config(agent_id), "auto_review", None)
            if config is not None:
                return config
        except Exception:
            logger.debug(
                "AUTO review could not load review config: agent=%s",
                agent_id,
                exc_info=True,
            )
    return AutoReviewConfig()


def _active_slot(agent_id: str | None) -> Any:
    from ..providers import ProviderManager

    if agent_id:
        try:
            from ..config.config import load_agent_config

            slot = load_agent_config(agent_id).active_model
            if slot is not None and slot.provider_id and slot.model:
                return slot
        except Exception:
            logger.debug(
                "AUTO review could not load agent model slot: agent=%s",
                agent_id,
                exc_info=True,
            )
    return ProviderManager.get_instance().get_active_model()


def _review_slots(
    agent_id: str | None,
    review_config: Any | None = None,
) -> list[tuple[Any, bool]]:
    """Return ``[(dedicated, True), (main, False)]`` without duplicates."""
    from ..config.config import ModelSlotConfig
    from ..providers import ProviderManager

    review_config = review_config or _load_review_config(agent_id)
    main_slot = _active_slot(agent_id)
    if main_slot is None or not main_slot.provider_id or not main_slot.model:
        return []

    main = ModelSlotConfig(
        provider_id=main_slot.provider_id,
        model=main_slot.model,
    )

    explicit = getattr(review_config, "review_model", None)
    if explicit is not None and explicit.provider_id and explicit.model:
        explicit_slot = ModelSlotConfig(
            provider_id=explicit.provider_id,
            model=explicit.model,
        )
        if explicit_slot == main:
            return [(main, False)]
        return [(explicit_slot, True), (main, False)]

    manager = ProviderManager.get_instance()
    provider = manager.get_provider(main_slot.provider_id)
    model_ids: list[str] = []
    if provider is not None:
        for model in [
            *(getattr(provider, "models", None) or []),
            *(getattr(provider, "extra_models", None) or []),
        ]:
            model_id = str(getattr(model, "id", "") or "").strip()
            if model_id:
                model_ids.append(model_id)

    review_model = select_review_model_id(main_slot.model, model_ids)
    if review_model == main_slot.model:
        return [(main, False)]
    return [
        (
            ModelSlotConfig(
                provider_id=main_slot.provider_id,
                model=review_model,
            ),
            True,
        ),
        (main, False),
    ]


async def _review_once(
    slot: Any,
    *,
    agent_id: str | None,
    payload: dict[str, Any],
) -> ReviewDecision | None:
    """Run one reviewer request with no tools or approval callback."""
    from agentscope.message import Msg, TextBlock

    from ..agents.model_factory import create_model_and_formatter
    from ..utils.model_response import consume_model_response

    model, _ = create_model_and_formatter(
        agent_id=agent_id,
        model_slot_override=slot,
    )
    messages = [
        Msg(
            name="system",
            role="system",
            content=[TextBlock(type="text", text=_SYSTEM_PROMPT)],
        ),
        Msg(
            name="user",
            role="user",
            content=[
                TextBlock(
                    type="text",
                    text=json.dumps(payload, ensure_ascii=False, default=str),
                ),
            ],
        ),
    ]
    text = await consume_model_response(
        model,
        messages,
        disable_thinking=True,
    )
    return parse_review_response(text)


async def review_tool_call(
    *,
    tool_name: str,
    target: str,
    params: dict[str, Any],
    agent_id: str | None,
    governance_reason: str | None,
    policy_findings: list[Any] | None,
    violation_msg: str | None,
    review_context: Any = None,
    request_metadata: Mapping[str, Any] | None = None,
) -> AutoReviewResult:
    """Review one guarded tool call without ever opening a human wait."""
    review_config = _load_review_config(agent_id)
    if not bool(getattr(review_config, "enabled", True)):
        return AutoReviewResult(
            approved=False,
            model_id="",
            used_dedicated_model=False,
            reason="automatic review is disabled",
        )

    slots = _review_slots(agent_id, review_config)
    if not slots:
        return AutoReviewResult(
            approved=False,
            model_id="",
            used_dedicated_model=False,
            reason="no active model available for automatic review",
        )

    findings = [
        {
            "severity": getattr(item, "severity", None),
            "title": getattr(item, "title", None),
            "description": getattr(item, "description", None),
            "matched_value": getattr(item, "matched_value", None),
        }
        for item in (policy_findings or [])
    ]
    max_context_chars = int(
        getattr(review_config, "max_context_chars", _MAX_REVIEW_VALUE_CHARS),
    )
    payload = {
        "tool_name": _redact_review_value(tool_name, key="tool_name"),
        "target": _redact_review_value(target, key="target"),
        "parameters": _redact_review_value(params or {}),
        "governance_reason": _redact_review_value(
            governance_reason or "",
            key="governance_reason",
        ),
        "sandbox_violation": _redact_review_value(
            violation_msg or "",
            key="sandbox_violation",
        ),
        "findings": _redact_review_value(findings, key="findings"),
        "context": _bounded_review_context(
            review_context or {},
            max_context_chars,
        ),
        "request_metadata": _bounded_review_context(
            dict(request_metadata or {}),
            max_context_chars,
        ),
    }
    payload = _bound_review_payload(payload)

    async def _run_candidates() -> AutoReviewResult:
        errors: list[str] = []
        for slot, dedicated in slots:
            try:
                decision = await _review_once(
                    slot,
                    agent_id=agent_id,
                    payload=payload,
                )
            except Exception as exc:  # noqa: BLE001 - fallback is intentional
                errors.append(f"{slot.model}: {exc.__class__.__name__}")
                logger.info(
                    "AUTO review model failed; trying fallback: model=%s",
                    slot.model,
                    exc_info=True,
                )
                continue
            if decision is None:
                errors.append(f"{slot.model}: invalid decision")
                continue
            if decision.require_human:
                return AutoReviewResult(
                    approved=False,
                    require_human=True,
                    model_id=slot.model,
                    used_dedicated_model=dedicated,
                    reason=decision.rationale or "reviewer requested a human",
                    risk_level=decision.risk_level,
                    user_authorization=decision.user_authorization,
                    rationale=decision.rationale,
                )
            # Do not let a contradictory or underspecified response open
            # the gate. High/critical or explicit none is DENY. Allow
            # with unknown risk/authorization escalates to a human.
            if decision.approved and (
                decision.risk_level in {"high", "critical"}
                or decision.user_authorization == "none"
            ):
                return AutoReviewResult(
                    approved=False,
                    model_id=slot.model,
                    used_dedicated_model=dedicated,
                    reason=(
                        "reviewer returned allow with unsafe risk or "
                        "authorization"
                    ),
                    risk_level=decision.risk_level,
                    user_authorization=decision.user_authorization,
                    rationale=decision.rationale,
                )
            if decision.approved and (
                decision.risk_level == "unknown"
                or decision.user_authorization == "unknown"
            ):
                return AutoReviewResult(
                    approved=False,
                    require_human=True,
                    model_id=slot.model,
                    used_dedicated_model=dedicated,
                    reason=(
                        "reviewer returned allow with unknown risk or "
                        "authorization"
                    ),
                    risk_level=decision.risk_level,
                    user_authorization=decision.user_authorization,
                    rationale=decision.rationale,
                )
            reason = decision.rationale or (
                "model approved" if decision.approved else "model denied"
            )
            return AutoReviewResult(
                approved=decision.approved,
                model_id=slot.model,
                used_dedicated_model=dedicated,
                reason=reason,
                risk_level=decision.risk_level,
                user_authorization=decision.user_authorization,
                rationale=decision.rationale,
            )
        return AutoReviewResult(
            approved=False,
            model_id=slots[-1][0].model,
            used_dedicated_model=False,
            reason="automatic review unavailable: " + ", ".join(errors),
        )

    timeout_seconds = float(
        getattr(review_config, "timeout_seconds", AUTO_REVIEW_TIMEOUT_SECONDS),
    )
    try:
        return await asyncio.wait_for(
            _run_candidates(),
            timeout=max(1.0, timeout_seconds),
        )
    except TimeoutError:
        return AutoReviewResult(
            approved=False,
            model_id=slots[-1][0].model,
            used_dedicated_model=False,
            reason="automatic review timed out",
        )


__all__ = [
    "AUTO_REVIEW_TIMEOUT_SECONDS",
    "AutoReviewResult",
    "ReviewDecision",
    "parse_review_decision",
    "parse_review_response",
    "review_tool_call",
    "select_review_model_id",
]
