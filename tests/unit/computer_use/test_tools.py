# -*- coding: utf-8 -*-
from __future__ import annotations

import json
from typing import Any
from unittest.mock import patch

import pytest

from potato.agents.tools import computer_use as cu
from potato.computer_use.client import CuaCallResult
from potato.computer_use.protect import policy_target_for_computer
from potato.computer_use.session import Observation, observation_store


class _FakeSettings:
    enabled = True
    driver_path = ""
    always_allowed_apps: list[str] = []


class _FakeClient:
    def __init__(self) -> None:
        self.calls: list[tuple[str, dict[str, Any]]] = []

    async def call(
        self,
        tool: str,
        args: dict[str, Any] | None = None,
        *,
        screenshot_path: str | None = None,
        timeout: float = 0,
    ) -> CuaCallResult:
        payload = dict(args or {})
        self.calls.append((tool, payload))
        if tool == "list_apps":
            return CuaCallResult(
                ok=True,
                text="",
                data=[
                    {
                        "name": "Calculator",
                        "bundle_id": "com.apple.calculator",
                        "pid": 42,
                        "running": True,
                    },
                    {
                        "name": "Notes",
                        "bundle_id": "com.apple.Notes",
                        "pid": 0,
                        "running": False,
                    },
                    {
                        "name": "Potato",
                        "bundle_id": "io.agentscope.qwenpaw.desktop",
                        "pid": 9,
                        "running": True,
                    },
                    {
                        "name": "Terminal",
                        "bundle_id": "com.apple.Terminal",
                        "pid": 11,
                        "running": True,
                    },
                ],
            )
        if tool == "list_windows":
            return CuaCallResult(
                ok=True,
                text="",
                data=[
                    {
                        "window_id": 7,
                        "pid": 42,
                        "is_on_screen": True,
                        "on_current_space": True,
                        "z_index": 3,
                    },
                ],
            )
        if tool == "get_window_state":
            return CuaCallResult(
                ok=True,
                text="",
                data={
                    "snapshot_id": "snap-1",
                    "tree_markdown": "- [element_index 1] button Equals",
                    "elements": [
                        {
                            "element_index": 1,
                            "element_token": "tok-1",
                            "role": "button",
                            "label": "Equals",
                        },
                    ],
                },
                screenshot_path=screenshot_path,
            )
        return CuaCallResult(
            ok=True,
            text="",
            data={"effect": "unverifiable", "delivery_mode": payload.get("delivery_mode")},
        )

    async def end_session(self, session_id: str) -> None:
        self.calls.append(("revoke", {"session": session_id}))


@pytest.fixture
def fake_driver() -> _FakeClient:
    observation_store().clear()
    cu.reset_computer_use_client()
    client = _FakeClient()
    with (
        patch.object(cu, "computer_use_enabled", return_value=True),
        patch.object(cu, "_client", return_value=client),
    ):
        yield client
    observation_store().clear()
    cu.reset_computer_use_client()


def _payload(chunk: Any) -> dict[str, Any]:
    text = chunk.content[-1].text
    return json.loads(text)


@pytest.mark.asyncio
async def test_observe_then_click_uses_token_and_background(
    fake_driver: _FakeClient,
) -> None:
    observed = _payload(await cu.computer_observe(app="Calculator"))
    assert observed["ok"] is True
    assert observed["bundle_id"] == "com.apple.calculator"
    observed_state = next(
        call for call in fake_driver.calls if call[0] == "get_window_state"
    )
    driver_session = observed_state[1]["session"]
    assert driver_session.startswith("potato-obs_")
    clicked = _payload(
        await cu.computer_click(
            observation_id=observed["observation_id"],
            app="com.apple.calculator",
            element_index=1,
        ),
    )
    assert clicked["ok"] is True
    click = next(call for call in fake_driver.calls if call[0] == "click")
    assert click[1]["delivery_mode"] == "background"
    assert click[1]["element_token"] == "tok-1"
    assert click[1]["pid"] == 42
    assert click[1]["window_id"] == 7
    assert click[1]["session"] == driver_session
    ended = [call for call in fake_driver.calls if call[0] == "revoke"]
    assert ended[-1][1] == {"session": driver_session}
    reused = _payload(
        await cu.computer_click(
            observation_id=observed["observation_id"],
            app="com.apple.calculator",
            element_index=1,
        ),
    )
    assert reused["ok"] is False
    assert reused["code"] == "STALE_OBSERVATION"


@pytest.mark.asyncio
async def test_click_rejects_app_mismatch(fake_driver: _FakeClient) -> None:
    observed = _payload(await cu.computer_observe(app="Calculator"))
    result = _payload(
        await cu.computer_click(
            observation_id=observed["observation_id"],
            app="com.apple.mail",
            element_index=1,
        ),
    )
    assert result["ok"] is False
    assert result["code"] == "APP_MISMATCH"
    assert not any(call[0] == "click" for call in fake_driver.calls)
    retried = _payload(
        await cu.computer_click(
            observation_id=observed["observation_id"],
            app="com.apple.calculator",
            element_index=1,
        ),
    )
    assert retried["ok"] is True


@pytest.mark.asyncio
async def test_set_value_omits_delivery_mode(fake_driver: _FakeClient) -> None:
    observed = _payload(await cu.computer_observe(app="Calculator"))
    result = _payload(
        await cu.computer_set_value(
            observation_id=observed["observation_id"],
            app="com.apple.calculator",
            element_index=1,
            value="42",
        ),
    )
    assert result["ok"] is True
    set_value = next(
        call for call in fake_driver.calls if call[0] == "set_value"
    )
    assert "delivery_mode" not in set_value[1]


@pytest.mark.asyncio
async def test_observe_rejects_protected_apps(fake_driver: _FakeClient) -> None:
    _ = fake_driver
    potato = _payload(await cu.computer_observe(app="Potato"))
    assert potato["ok"] is False
    assert potato["code"] == "APP_PROTECTED"
    terminal = _payload(await cu.computer_observe(app="com.apple.Terminal"))
    assert terminal["ok"] is False
    assert terminal["code"] == "APP_PROTECTED"


@pytest.mark.asyncio
async def test_observe_does_not_launch_stopped_app(fake_driver: _FakeClient) -> None:
    result = _payload(await cu.computer_observe(app="Notes"))
    assert result["ok"] is False
    assert result["code"] == "APP_NOT_RUNNING"
    assert not any(call[0] == "launch_app" for call in fake_driver.calls)


@pytest.mark.asyncio
async def test_stale_observation_is_rejected(fake_driver: _FakeClient) -> None:
    _ = fake_driver
    result = _payload(
        await cu.computer_click(
            observation_id="obs_missing",
            app="Calculator",
            element_index=1,
        ),
    )
    assert result["ok"] is False
    assert result["code"] == "STALE_OBSERVATION"


@pytest.mark.asyncio
async def test_expired_observation_session_is_reaped(
    fake_driver: _FakeClient,
) -> None:
    store = observation_store()
    store.put(
        Observation(
            observation_id="obs_expired",
            session_id="potato-obs_expired",
            app="Calculator",
            bundle_id="com.apple.calculator",
            pid=42,
            window_id=7,
            snapshot_id="snap-expired",
            created_at=0,
        ),
    )
    cu._CLIENT = fake_driver  # type: ignore[assignment]
    try:
        policy_target_for_computer(
            {"observation_id": "obs_expired", "app": "Calculator"},
        )
        await __import__("asyncio").sleep(0)
        ended = [
            call for call in fake_driver.calls if call[0] == "revoke"
        ]
        assert ended[-1][1] == {"session": "potato-obs_expired"}
    finally:
        cu._CLIENT = None


@pytest.mark.asyncio
async def test_disabled_feature_fails_closed() -> None:
    observation_store().clear()
    cu.reset_computer_use_client()
    with patch.object(cu, "computer_use_enabled", return_value=False):
        result = _payload(await cu.computer_list_apps())
    assert result["ok"] is False
    assert result["code"] == "DISABLED"
