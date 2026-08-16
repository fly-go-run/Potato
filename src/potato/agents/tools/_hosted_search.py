# -*- coding: utf-8 -*-
"""Web search run by the model host, via the Responses API.

Several hosts expose a server-side ``web_search`` tool: given a question the
host issues its own queries, opens the pages it wants, and answers from what
it read. That is a different shape of result from a keyword API — a
researched answer plus the pages consulted, not five snippets — so this
module wraps the call and presents it as an ordinary tool result.

Verified against two hosts with meaningfully different reporting habits:

* DeepSeek (``deepseek-v4-flash``) records every step as a
  ``web_search_call``, including ``open_page`` actions naming each URL.
* OpenAI-compatible gateways (tested through a sub2api relay on
  ``gpt-5.6``) usually only record ``search`` actions and attach the actual
  sources as ``url_citation`` annotations on the message.

Both are read here, because a backend that silently loses its citations is
worse than one with none: the caller is told to verify against sources that
were never listed.

Only the credentials come from the local provider config. The model doing
the searching is unrelated to the model calling the tool, so a session
running Claude or Qwen gets the same search quality.
"""

from __future__ import annotations

import asyncio
import json
import logging
from typing import Any, Optional, Tuple
from urllib.parse import urlsplit, urlunsplit

import httpx

logger = logging.getLogger(__name__)

# Tried in order when no provider is pinned. Both speak api.deepseek.com and
# either key works: the hosted search lives on /responses, served from the
# same host as the Chat Completions endpoint the other provider uses. Any
# other host — a gateway relaying OpenAI, say — is reached by naming it in
# ``web_search_provider_id``; guessing at gateway ids would be worse than
# making the choice explicit.
_DEFAULT_PROVIDER_IDS = ("deepseek-response", "deepseek")
_FALLBACK_BASE_URL = "https://api.deepseek.com"

# Long enough for a real multi-page search (we have measured 5-13 internal
# calls per question), short enough that a wedged request cannot hold the
# turn open indefinitely.
DEFAULT_TIMEOUT_SECONDS = 120

# Guard the caller's context window. The answer is prose written by the
# search model; past this length it is padding, not substance.
MAX_ANSWER_CHARS = 8000

# The search host probes freely — we have seen 16 queries for one
# question. Listing them all costs the caller tokens for little gain; a
# few are enough to judge whether the question was understood.
MAX_QUERIES_SHOWN = 6

# Upstream error text is attacker-adjacent (it comes from whatever host
# base_url names) and lands in both logs and the model's context.
MAX_ERROR_DETAIL_CHARS = 300

_SEARCH_INSTRUCTIONS = (
    "You are a research assistant. Search the web and answer the question "
    "from what you actually read. State dates and figures explicitly, and "
    "say plainly when sources disagree or when you could not confirm "
    "something. Do not pad the answer with advice the question did not ask "
    "for."
)


class HostedSearchUnavailable(RuntimeError):
    """No usable credentials for a hosted-search provider."""


class HostedSearchUpstreamError(RuntimeError):
    """The host answered, but with a failure.

    Distinct from local failures (no credentials, timeout, DNS) because the
    message quotes text the remote host wrote. Callers use that to decide
    whether the text needs an untrusted-content boundary — labelling a local
    timeout as web content is its own kind of lie.
    """


def _strip_ws_fragment(url: str) -> str:
    """Drop the ``#ws_call_id=...`` marker DeepSeek appends to page URLs."""
    try:
        parts = urlsplit(url)
    except ValueError:
        return url
    if parts.fragment.startswith("ws_call_id="):
        return urlunsplit(parts._replace(fragment=""))
    return url


def resolve_credentials(
    provider_id: str = "",
) -> Tuple[str, str]:
    """Return ``(base_url, api_key)`` for the search call.

    A specific ``provider_id`` is honoured as-is so a user can point this at
    a gateway; otherwise the known DeepSeek providers are tried in order.
    """
    from ...providers.provider_manager import ProviderManager

    manager = ProviderManager.get_instance()
    candidates = (provider_id,) if provider_id else _DEFAULT_PROVIDER_IDS
    for pid in candidates:
        try:
            provider = manager.get_provider(pid)
        except Exception:  # noqa: BLE001 - a bad id must not be fatal
            logger.debug("web_search: provider %s unavailable", pid)
            continue
        if provider is None:
            continue
        key = (getattr(provider, "api_key", "") or "").strip()
        if not key:
            continue
        base = (
            getattr(provider, "base_url", "") or ""
        ).strip() or _FALLBACK_BASE_URL
        return base.rstrip("/"), key

    raise HostedSearchUnavailable(
        "No API key is configured for hosted web search. Add a DeepSeek key "
        "under Settings → Models, point web_search_provider_id at another "
        "provider whose host runs the search, or switch the search backend "
        "to 'tavily'.",
    )


def is_available(provider_id: str = "") -> bool:
    """Whether a search call could be made right now."""
    try:
        resolve_credentials(provider_id)
        return True
    except Exception:  # noqa: BLE001
        return False


def _collect_actions(payload: dict) -> Tuple[list[str], list[str]]:
    """Return ``(queries, pages)`` describing what the search actually did.

    Sources are gathered from both conventions in use. DeepSeek names each
    page in an ``open_page`` action; OpenAI-compatible hosts often issue only
    ``search`` actions and put the pages they actually cited in
    ``url_citation`` annotations on the message. Reading just one convention
    loses every source on the other host, while still telling the caller to
    go verify against them.
    """
    queries: list[str] = []
    pages: list[str] = []

    def _add_page(raw: object) -> None:
        if not isinstance(raw, str) or not raw:
            return
        clean = _strip_ws_fragment(raw)
        if clean not in pages:
            pages.append(clean)

    for item in payload.get("output") or []:
        if not isinstance(item, dict):
            continue
        if item.get("type") == "web_search_call":
            action = item.get("action")
            if not isinstance(action, dict):
                continue
            for q in action.get("queries") or []:
                # DeepSeek smuggles its own call id in as a pseudo-query.
                if (
                    isinstance(q, str)
                    and q
                    and not q.startswith("ws_call_id=")
                    and q not in queries
                ):
                    queries.append(q)
            _add_page(action.get("url"))
        elif item.get("type") == "message":
            for block in item.get("content") or []:
                if not isinstance(block, dict):
                    continue
                for ann in block.get("annotations") or []:
                    if (
                        isinstance(ann, dict)
                        and ann.get("type") == "url_citation"
                    ):
                        _add_page(ann.get("url"))
    return queries, pages


def _extract_answer(payload: dict) -> str:
    """Pull the assistant's prose out of a Responses payload."""
    direct = payload.get("output_text")
    if isinstance(direct, str) and direct.strip():
        return direct.strip()
    parts: list[str] = []
    for item in payload.get("output") or []:
        if not isinstance(item, dict) or item.get("type") != "message":
            continue
        for block in item.get("content") or []:
            if not isinstance(block, dict):
                continue
            if block.get("type") in ("output_text", "text"):
                text = block.get("text")
                if isinstance(text, str) and text:
                    parts.append(text)
    return "".join(parts).strip()


def format_result(payload: dict) -> str:
    """Render a Responses payload as the text the calling model will read."""
    answer = _extract_answer(payload)
    queries, pages = _collect_actions(payload)

    if not answer and not pages:
        return "No results found."

    if len(answer) > MAX_ANSWER_CHARS:
        answer = (
            answer[:MAX_ANSWER_CHARS].rstrip()
            + "\n\n[truncated; ask a narrower question for the rest]"
        )

    sections: list[str] = [answer or "(the search returned no prose answer)"]
    if pages:
        sections.append(
            "\nSources consulted:\n" + "\n".join(f"- {url}" for url in pages),
        )
    if queries:
        shown = queries[:MAX_QUERIES_SHOWN]
        line = "\nQueries issued: " + "; ".join(shown)
        remaining = len(queries) - len(shown)
        if remaining > 0:
            line += f" (+{remaining} more)"
        sections.append(line)
    if pages:
        sections.append(
            "\nUse web_fetch on any source above to read it in full.",
        )
    return "\n".join(sections).strip()


async def search(
    query: str,
    *,
    provider_id: str = "",
    model: str = "deepseek-v4-flash",
    timeout_seconds: int = DEFAULT_TIMEOUT_SECONDS,
) -> str:
    """Run one server-side search and return formatted text.

    ``timeout_seconds`` is a wall-clock ceiling on the whole call, enforced
    here rather than left to the caller: the tool coordinator's deadline
    does not currently cancel anything (see the FIXME in
    ``tool_calls/_coordinator.py``), so without this a slow search holds the
    turn open for as long as it likes. httpx's own timeout only bounds each
    individual socket operation, which a server trickling bytes can dodge
    indefinitely.

    Raises ``HostedSearchUnavailable`` when unconfigured,
    ``asyncio.TimeoutError`` when over budget, ``httpx.HTTPError`` on a
    transport failure, and ``HostedSearchUpstreamError`` when the host
    itself reports one — the last of these quotes remote text, which the
    caller must treat accordingly.
    """
    base_url, api_key = resolve_credentials(provider_id)

    body: dict[str, Any] = {
        "model": model,
        "instructions": _SEARCH_INSTRUCTIONS,
        "input": query,
        "tools": [{"type": "web_search"}],
        # The caller asked for a search, so do not let the model decide it
        # already knows the answer — stale training data is the exact thing
        # this tool exists to avoid.
        "tool_choice": {"type": "web_search"},
        "stream": False,
    }

    async def _post_once() -> "httpx.Response":
        async with httpx.AsyncClient(timeout=timeout_seconds) as client:
            return await client.post(
                f"{base_url}/responses",
                headers={
                    "Authorization": f"Bearer {api_key}",
                    "Content-Type": "application/json",
                },
                json=body,
            )

    response = await asyncio.wait_for(_post_once(), timeout=timeout_seconds)

    if response.status_code != 200:
        detail = _error_detail(response)
        raise HostedSearchUpstreamError(
            f"Hosted web search failed "
            f"(HTTP {response.status_code}): {detail}",
        )

    try:
        payload = response.json()
    except json.JSONDecodeError as exc:
        raise HostedSearchUpstreamError(
            f"Hosted web search returned non-JSON: {exc}",
        ) from exc

    return format_result(payload)


def _error_detail(response: "httpx.Response") -> str:
    """Best-effort human-readable reason from an error response.

    Every branch is length-capped. This text ends up in a log line and, when
    the user pinned the backend explicitly, in the calling model's context —
    and it is written by whatever host ``base_url`` points at, which need not
    be DeepSeek. A gateway that echoes request headers or a stack trace back
    in its error body would otherwise put that straight into both.
    """
    try:
        body = response.json()
    except Exception:  # noqa: BLE001
        return _clip(response.text or "")
    if isinstance(body, dict):
        error = body.get("error")
        if isinstance(error, dict):
            message = error.get("message")
            if isinstance(message, str) and message:
                return _clip(message.strip())
    return _clip(json.dumps(body))


def _clip(text: str) -> str:
    if len(text) <= MAX_ERROR_DETAIL_CHARS:
        return text
    return text[:MAX_ERROR_DETAIL_CHARS].rstrip() + "…"


def describe_backend(provider_id: str = "") -> Optional[str]:
    """Return the base URL in use, or ``None`` when unconfigured."""
    try:
        base_url, _ = resolve_credentials(provider_id)
        return base_url
    except Exception:  # noqa: BLE001
        return None
