# -*- coding: utf-8 -*-
"""Codex-sized computer-use facade. Cua Driver does the clicking."""

from __future__ import annotations

import asyncio
import json
import logging
import mimetypes
import secrets
from typing import Any

from agentscope.message import DataBlock, TextBlock, URLSource
from agentscope.message import ToolResultState
from agentscope.tool import ToolChunk

from ...computer_use.bundle import runtime_home
from ...computer_use.client import CuaDriverClient
from ...computer_use.constants import (
    COMPUTER_USE_TOOL_NAMES,
    INPUT_DRIVER_TOOLS,
    OBSERVATION_TTL_SECONDS,
)
from ...computer_use.errors import ComputerUseError
from ...computer_use.protect import (
    assert_observation_matches_claim,
    is_protected_app,
)
from ...computer_use.session import (
    Observation,
    new_observation_id,
    observation_session_id,
    observation_store,
)
from ...computer_use.settings import computer_use_enabled
from ...runtime.tool_registry import tool_descriptor
from .file_io import _path_to_file_url

logger = logging.getLogger(__name__)

_CLIENT: CuaDriverClient | None = None
_CLIENT_LOCK: asyncio.Lock | None = None
_REAP_TASKS: set[asyncio.Task[None]] = set()


def reset_computer_use_client() -> None:
    """Drop the cached driver client (tests)."""
    global _CLIENT, _CLIENT_LOCK
    _CLIENT = None
    _CLIENT_LOCK = None
    for task in _REAP_TASKS:
        task.cancel()
    _REAP_TASKS.clear()
    observation_store().drain_reaped_session_ids()


async def shutdown_computer_use() -> None:
    """Stop the owned driver daemon. Called on app shutdown."""
    global _CLIENT, _CLIENT_LOCK
    client = _CLIENT
    _CLIENT = None
    _CLIENT_LOCK = None
    if client is not None:
        observation_store().clear()
        await _reap_discarded_sessions(client)
        if _REAP_TASKS:
            await asyncio.gather(*tuple(_REAP_TASKS), return_exceptions=True)
        await client.stop()
    else:
        observation_store().clear()
        observation_store().drain_reaped_session_ids()


async def _client() -> CuaDriverClient:
    global _CLIENT, _CLIENT_LOCK
    if not computer_use_enabled():
        raise ComputerUseError(
            "DISABLED",
            "Computer Use is off. Enable it in Settings → Security.",
        )
    if _CLIENT is not None:
        return _CLIENT
    if _CLIENT_LOCK is None:
        _CLIENT_LOCK = asyncio.Lock()
    async with _CLIENT_LOCK:
        if _CLIENT is None:
            _CLIENT = await asyncio.to_thread(CuaDriverClient.from_config)
        return _CLIENT


async def _end_driver_session(
    client: CuaDriverClient,
    session_id: str,
) -> None:
    if not session_id:
        return
    try:
        await client.end_session(session_id)
    except Exception:
        logger.debug(
            "failed to revoke cua-driver session %s",
            session_id,
            exc_info=True,
        )


async def _reap_discarded_sessions(client: CuaDriverClient) -> None:
    for session_id in observation_store().drain_reaped_session_ids():
        await _end_driver_session(client, session_id)


def _schedule_discarded_session_reap() -> None:
    """End expired/replaced observation sessions on the active event loop."""
    client = _CLIENT
    if client is None:
        return
    try:
        loop = asyncio.get_running_loop()
    except RuntimeError:
        return
    task = loop.create_task(_reap_discarded_sessions(client))
    _REAP_TASKS.add(task)
    task.add_done_callback(_REAP_TASKS.discard)


observation_store().set_reap_notifier(_schedule_discarded_session_reap)


def _tool_error(code: str, message: str) -> ToolChunk:
    return ToolChunk(
        is_last=True,
        state=ToolResultState.SUCCESS,
        content=[
            TextBlock(
                type="text",
                text=json.dumps(
                    {"ok": False, "code": code, "error": message},
                    ensure_ascii=False,
                    indent=2,
                ),
            ),
        ],
    )


def _tool_json(payload: dict[str, Any], screenshot_path: str = "") -> ToolChunk:
    blocks: list[Any] = []
    if screenshot_path:
        mime_type, _ = mimetypes.guess_type(screenshot_path)
        blocks.append(
            DataBlock(
                source=URLSource(
                    url=_path_to_file_url(screenshot_path),
                    media_type=mime_type or "image/png",
                ),
                name=screenshot_path.rsplit("/", 1)[-1],
            ),
        )
    blocks.append(
        TextBlock(
            type="text",
            text=json.dumps(payload, ensure_ascii=False, indent=2),
        ),
    )
    return ToolChunk(
        is_last=True,
        state=ToolResultState.SUCCESS,
        content=blocks,
    )


def _catch(exc: Exception) -> ToolChunk:
    if isinstance(exc, ComputerUseError):
        return _tool_error(exc.code, exc.message)
    logger.exception("computer use failed")
    return _tool_error("INTERNAL", str(exc))


def _as_list(value: Any) -> list[Any]:
    if isinstance(value, list):
        return value
    if isinstance(value, dict):
        for key in ("apps", "windows", "elements", "items"):
            if isinstance(value.get(key), list):
                return value[key]
    return []


def _as_dict(value: Any) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}


async def _list_app_records(client: CuaDriverClient) -> list[dict[str, Any]]:
    result = await client.call("list_apps")
    return [
        item
        for item in _as_list(result.data)
        if isinstance(item, dict)
    ]


def _match_app(apps: list[dict[str, Any]], app: str) -> dict[str, Any]:
    needle = app.strip().lower()
    if not needle:
        raise ComputerUseError("APP_REQUIRED", "Pass an app name or bundle id.")

    def _bundle(item: dict[str, Any]) -> str:
        return str(
            item.get("bundle_id") or item.get("bundleId") or item.get("id") or "",
        )

    def _name(item: dict[str, Any]) -> str:
        return str(item.get("name") or item.get("display_name") or item.get("app_name") or "")

    exact = [
        item
        for item in apps
        if _bundle(item).lower() == needle or _name(item).lower() == needle
    ]
    if len(exact) == 1:
        return exact[0]
    if len(exact) > 1:
        running = [item for item in exact if item.get("running") or item.get("pid")]
        if len(running) == 1:
            return running[0]
        raise ComputerUseError(
            "APP_AMBIGUOUS",
            f"{app!r} matched multiple apps. Use the exact bundle id.",
        )
    partial = [
        item
        for item in apps
        if needle in _name(item).lower() or needle in _bundle(item).lower()
    ]
    if len(partial) == 1:
        return partial[0]
    if len(partial) > 1:
        raise ComputerUseError(
            "APP_AMBIGUOUS",
            f"{app!r} is ambiguous. Use a bundle id from computer_list_apps.",
        )
    raise ComputerUseError(
        "APP_NOT_FOUND",
        f"No installed app matches {app!r}. Call computer_list_apps.",
    )


def _pid_of(app_rec: dict[str, Any]) -> int:
    try:
        return int(app_rec.get("pid") or 0)
    except (TypeError, ValueError):
        return 0


def _bundle_of(app_rec: dict[str, Any]) -> str:
    return str(
        app_rec.get("bundle_id")
        or app_rec.get("bundleId")
        or app_rec.get("id")
        or "",
    )


def _name_of(app_rec: dict[str, Any]) -> str:
    return str(
        app_rec.get("name")
        or app_rec.get("display_name")
        or app_rec.get("app_name")
        or _bundle_of(app_rec),
    )


async def _pick_window(
    client: CuaDriverClient,
    pid: int,
) -> dict[str, Any]:
    result = await client.call("list_windows", {"pid": pid})
    windows = [item for item in _as_list(result.data) if isinstance(item, dict)]
    if not windows:
        raise ComputerUseError(
            "WINDOW_NOT_FOUND",
            f"Process {pid} has no windows Potato can address.",
        )

    def _score(item: dict[str, Any]) -> tuple[int, int, int]:
        on_space = 1 if item.get("on_current_space") else 0
        on_screen = 1 if item.get("is_on_screen") else 0
        try:
            z_index = int(item.get("z_index") or 0)
        except (TypeError, ValueError):
            z_index = 0
        return (on_space, on_screen, z_index)

    return max(windows, key=_score)


def _elements_from_state(data: dict[str, Any]) -> list[dict[str, Any]]:
    raw = data.get("elements")
    if not isinstance(raw, list):
        structured = data.get("structuredContent")
        if isinstance(structured, dict) and isinstance(structured.get("elements"), list):
            raw = structured["elements"]
        else:
            raw = []
    elements: list[dict[str, Any]] = []
    for item in raw:
        if not isinstance(item, dict):
            continue
        try:
            index = int(item.get("element_index"))
        except (TypeError, ValueError):
            continue
        elements.append(
            {
                "element_index": index,
                "element_token": item.get("element_token"),
                "role": item.get("role") or "",
                "label": item.get("label") or item.get("name") or "",
                "value": item.get("value") or "",
            },
        )
    return elements


def _tree_text(data: dict[str, Any], fallback: str) -> str:
    for key in ("tree_markdown", "text", "tree"):
        value = data.get(key)
        if isinstance(value, str) and value.strip():
            return value
    structured = data.get("structuredContent")
    if isinstance(structured, dict):
        value = structured.get("tree_markdown")
        if isinstance(value, str) and value.strip():
            return value
    return fallback


def _snapshot_id_of(data: dict[str, Any]) -> str:
    for key in ("snapshot_id", "snapshotId"):
        value = data.get(key)
        if value:
            return str(value)
    structured = data.get("structuredContent")
    if isinstance(structured, dict) and structured.get("snapshot_id"):
        return str(structured["snapshot_id"])
    return ""


def _screenshot_path() -> str:
    shots = runtime_home() / "shots"
    shots.mkdir(parents=True, exist_ok=True)
    return str(shots / f"observe_{secrets.token_hex(8)}.png")


async def _observe_app(app: str, include_screenshot: bool) -> ToolChunk:
    client = await _client()
    apps = await _list_app_records(client)
    rec = _match_app(apps, app)
    if is_protected_app(
        bundle_id=_bundle_of(rec),
        app_name=_name_of(rec),
        pid=_pid_of(rec),
    ):
        raise ComputerUseError(
            "APP_PROTECTED",
            "Computer Use cannot operate Potato, Terminal, or System Settings.",
        )
    pid = _pid_of(rec)
    if pid <= 0:
        raise ComputerUseError(
            "APP_NOT_RUNNING",
            f"{app!r} is not running. Open it first, then observe.",
        )
    window = await _pick_window(client, pid)
    try:
        window_id = int(window.get("window_id") or window.get("id"))
    except (TypeError, ValueError) as exc:
        raise ComputerUseError(
            "WINDOW_NOT_FOUND",
            "Cua Driver returned a window without an id.",
        ) from exc
    shot = _screenshot_path() if include_screenshot else None
    observation_id = new_observation_id()
    session_id = observation_session_id(observation_id)
    try:
        state = await client.call(
            "get_window_state",
            {
                "pid": pid,
                "window_id": window_id,
                "include_screenshot": bool(include_screenshot),
                "session": session_id,
            },
            screenshot_path=shot,
        )
    except Exception:
        await _end_driver_session(client, session_id)
        raise
    data = _as_dict(state.data)
    elements = _elements_from_state(data)
    observation = observation_store().put(
        Observation(
            observation_id=observation_id,
            app=_name_of(rec),
            bundle_id=_bundle_of(rec),
            pid=pid,
            window_id=window_id,
            snapshot_id=_snapshot_id_of(data),
            session_id=session_id,
            elements=elements,
        ),
    )
    await _reap_discarded_sessions(client)
    payload = {
        "ok": True,
        "observation_id": observation.observation_id,
        "app": observation.app,
        "bundle_id": observation.bundle_id,
        "pid": pid,
        "window_id": window_id,
        "element_count": len(elements),
        "ttl_seconds": OBSERVATION_TTL_SECONDS,
        "text": _tree_text(data, state.text),
        "note": (
            "Use this observation_id and element_index for the next action. "
            "Do not reuse them after another observe."
        ),
    }
    return _tool_json(payload, state.screenshot_path or "")


def _target_payload(
    tool: str,
    observation: Observation,
    *,
    element_index: int | None = None,
) -> dict[str, Any]:
    payload: dict[str, Any] = {
        "pid": observation.pid,
        "window_id": observation.window_id,
        "session": observation.session_id,
    }
    if tool in INPUT_DRIVER_TOOLS:
        payload["delivery_mode"] = "background"
    if observation.snapshot_id:
        payload["snapshot_id"] = observation.snapshot_id
    if element_index is not None:
        element = observation.element(element_index)
        token = element.get("element_token")
        if token:
            payload["element_token"] = token
        payload["element_index"] = element_index
    return payload


async def _run_action(
    tool: str,
    observation_id: str,
    args: dict[str, Any],
    *,
    element_index: int | None = None,
    claimed_app: str = "",
) -> ToolChunk:
    client = await _client()
    observation: Observation | None = None
    try:
        peeked = observation_store().get(observation_id)
        assert_observation_matches_claim(peeked, claimed_app)
        observation = observation_store().take(observation_id)
        payload = _target_payload(
            tool,
            observation,
            element_index=element_index,
        )
        payload.update(args)
        result = await client.call(tool, payload)
        data = _as_dict(result.data)
        return _tool_json(
            {
                "ok": True,
                "app": observation.app,
                "bundle_id": observation.bundle_id,
                "observation_id": observation.observation_id,
                "delivery_mode": "background",
                "effect": data.get("effect") or "unverifiable",
                "driver": data or result.text,
                "note": (
                    "Call computer_observe before the next action. "
                    "Foreground upgrade is not available."
                ),
            },
        )
    finally:
        if observation is not None:
            await _end_driver_session(client, observation.session_id)
        await _reap_discarded_sessions(client)


@tool_descriptor(
    enabled_by_default=True,
    async_execution=True,
    tool_type="computer",
    policy_name="ComputerListApps",
    default_policy="allow",
    policy_reason="List installed desktop apps (read-only)",
    ui_description="List desktop apps for Computer Use",
    ui_icon="💻",
)
async def computer_list_apps() -> ToolChunk:
    """List installed desktop apps Potato can observe or control.

    Prefer this before guessing an app name. Use the returned bundle id
    with computer_observe. This does not click or type.

    Returns:
        JSON list of apps with name, bundle_id, pid, and running flag.
    """
    try:
        apps = []
        for item in await _list_app_records(await _client()):
            apps.append(
                {
                    "name": _name_of(item),
                    "bundle_id": _bundle_of(item),
                    "pid": _pid_of(item) or None,
                    "running": bool(_pid_of(item)),
                    "active": bool(item.get("active")),
                },
            )
        return _tool_json({"ok": True, "apps": apps})
    except Exception as exc:
        return _catch(exc)


@tool_descriptor(
    enabled_by_default=True,
    async_execution=True,
    tool_type="computer",
    target_param="app",
    policy_name="ComputerObserve",
    default_policy="allow",
    policy_reason="Read one app's accessibility tree",
    ui_description="Observe a desktop app",
    ui_icon="💻",
)
async def computer_observe(
    app: str,
    include_screenshot: bool = True,
) -> ToolChunk:
    """Observe one desktop app without bringing it to the front.

    Returns an accessibility tree and an observation_id. Later
    computer_click / type / key / scroll / drag calls must use that id.
    Prefer element_index from this tree over screenshot coordinates.
    Use browser_use for web pages when a DOM is available.

    Args:
        app: App display name or bundle id, e.g. "Calculator"
            or "com.apple.calculator".
        include_screenshot: When true, also return a window screenshot.

    Returns:
        observation_id, AX text, and optional screenshot.
    """
    try:
        return await _observe_app(app, include_screenshot)
    except Exception as exc:
        return _catch(exc)


@tool_descriptor(
    enabled_by_default=True,
    async_execution=True,
    tool_type="computer",
    target_param="app",
    policy_name="ComputerClick",
    default_policy="ask",
    policy_reason="Click in a desktop app",
    ui_description="Click in a desktop app",
    ui_icon="💻",
)
async def computer_click(
    observation_id: str,
    app: str = "",
    element_index: int | None = None,
    x: float | None = None,
    y: float | None = None,
    button: str = "left",
) -> ToolChunk:
    """Click in the observed app without moving the real mouse.

    Prefer element_index from the latest computer_observe. Use x,y only
    for canvas / video / other surfaces missing from the AX tree.
    Coordinates are window-local pixels from that observation.

    Args:
        observation_id: Fresh id from computer_observe.
        app: Same app name/bundle used to observe; used for approval.
        element_index: AX element from the observation.
        x: Optional window-local pixel X.
        y: Optional window-local pixel Y.
        button: left, right, or middle.
    """
    try:
        args: dict[str, Any] = {"button": button or "left"}
        if x is not None and y is not None:
            args["x"] = x
            args["y"] = y
            return await _run_action(
                "click",
                observation_id,
                args,
                claimed_app=app,
            )
        if element_index is None:
            raise ComputerUseError(
                "TARGET_REQUIRED",
                "Pass element_index from computer_observe, or x and y.",
            )
        return await _run_action(
            "click",
            observation_id,
            args,
            element_index=element_index,
            claimed_app=app,
        )
    except Exception as exc:
        return _catch(exc)


@tool_descriptor(
    enabled_by_default=True,
    async_execution=True,
    tool_type="computer",
    target_param="app",
    policy_name="ComputerSetValue",
    default_policy="ask",
    policy_reason="Set a desktop control value",
    ui_description="Set a desktop control value",
    ui_icon="💻",
)
async def computer_set_value(
    observation_id: str,
    element_index: int,
    value: str,
    app: str = "",
) -> ToolChunk:
    """Set an accessibility value on one observed element.

    Prefer this over typing for native fields, sliders, and popups.

    Args:
        observation_id: Fresh id from computer_observe.
        element_index: AX element from the observation.
        value: New value.
        app: Same app name/bundle used to observe; used for approval.
    """
    try:
        return await _run_action(
            "set_value",
            observation_id,
            {"value": value},
            element_index=element_index,
            claimed_app=app,
        )
    except Exception as exc:
        return _catch(exc)


@tool_descriptor(
    enabled_by_default=True,
    async_execution=True,
    tool_type="computer",
    target_param="app",
    policy_name="ComputerTypeText",
    default_policy="ask",
    policy_reason="Type into a desktop app",
    ui_description="Type into a desktop app",
    ui_icon="💻",
)
async def computer_type_text(
    observation_id: str,
    text: str,
    app: str = "",
    element_index: int | None = None,
) -> ToolChunk:
    """Type into the observed app without stealing keyboard focus.

    Prefer computer_set_value for native fields. This is the fallback
    for fields that only accept inserted text.

    Args:
        observation_id: Fresh id from computer_observe.
        text: Text to insert. Avoid trailing newlines unless you mean Return.
        app: Same app name/bundle used to observe; used for approval.
        element_index: Optional field from the observation.
    """
    try:
        return await _run_action(
            "type_text",
            observation_id,
            {"text": text},
            element_index=element_index,
            claimed_app=app,
        )
    except Exception as exc:
        return _catch(exc)


@tool_descriptor(
    enabled_by_default=True,
    async_execution=True,
    tool_type="computer",
    target_param="app",
    policy_name="ComputerPressKey",
    default_policy="ask",
    policy_reason="Press a key in a desktop app",
    ui_description="Press a key in a desktop app",
    ui_icon="💻",
)
async def computer_press_key(
    observation_id: str,
    key: str,
    app: str = "",
    element_index: int | None = None,
) -> ToolChunk:
    """Press one key in the observed app. Cannot send global shortcuts.

    Args:
        observation_id: Fresh id from computer_observe.
        key: Key name such as return, tab, escape, up, space.
        app: Same app name/bundle used to observe; used for approval.
        element_index: Optional field to focus first.
    """
    try:
        return await _run_action(
            "press_key",
            observation_id,
            {"key": key},
            element_index=element_index,
            claimed_app=app,
        )
    except Exception as exc:
        return _catch(exc)


@tool_descriptor(
    enabled_by_default=True,
    async_execution=True,
    tool_type="computer",
    target_param="app",
    policy_name="ComputerScroll",
    default_policy="ask",
    policy_reason="Scroll a desktop app",
    ui_description="Scroll a desktop app",
    ui_icon="💻",
)
async def computer_scroll(
    observation_id: str,
    direction: str,
    app: str = "",
    element_index: int | None = None,
    amount: int = 3,
) -> ToolChunk:
    """Scroll the observed app in the background.

    Args:
        observation_id: Fresh id from computer_observe.
        direction: up, down, left, or right.
        app: Same app name/bundle used to observe; used for approval.
        element_index: Optional element to scroll.
        amount: Scroll steps. Default 3.
    """
    try:
        return await _run_action(
            "scroll",
            observation_id,
            {"direction": direction, "amount": amount},
            element_index=element_index,
            claimed_app=app,
        )
    except Exception as exc:
        return _catch(exc)


@tool_descriptor(
    enabled_by_default=True,
    async_execution=True,
    tool_type="computer",
    target_param="app",
    policy_name="ComputerDrag",
    default_policy="ask",
    policy_reason="Drag in a desktop app",
    ui_description="Drag in a desktop app",
    ui_icon="💻",
)
async def computer_drag(
    observation_id: str,
    from_x: float,
    from_y: float,
    to_x: float,
    to_y: float,
    app: str = "",
) -> ToolChunk:
    """Drag inside the observed window. Coordinates are window-local.

    Args:
        observation_id: Fresh id from computer_observe.
        from_x: Start X in the observation screenshot.
        from_y: Start Y in the observation screenshot.
        to_x: End X.
        to_y: End Y.
        app: Same app name/bundle used to observe; used for approval.
    """
    try:
        return await _run_action(
            "drag",
            observation_id,
            {
                "from_x": from_x,
                "from_y": from_y,
                "to_x": to_x,
                "to_y": to_y,
            },
            claimed_app=app,
        )
    except Exception as exc:
        return _catch(exc)


assert COMPUTER_USE_TOOL_NAMES == {
    computer_list_apps.__name__,
    computer_observe.__name__,
    computer_click.__name__,
    computer_set_value.__name__,
    computer_type_text.__name__,
    computer_press_key.__name__,
    computer_scroll.__name__,
    computer_drag.__name__,
}
