# -*- coding: utf-8 -*-
from __future__ import annotations

import pytest

from potato.computer_use.client import CuaCallResult, CuaCommandResult, CuaDriverClient
from potato.computer_use.errors import ComputerUseError


def _ok(stdout: str = '{"running": true}') -> CuaCommandResult:
    return CuaCommandResult(returncode=0, stdout=stdout, stderr="")


@pytest.mark.asyncio
async def test_call_forces_background_and_blocks_unknown_tools() -> None:
    seen: list[list[str]] = []

    async def runner(cmd: list[str], _timeout: float) -> CuaCommandResult:
        seen.append(cmd)
        if cmd[1] == "status":
            return _ok("daemon running pid=1")
        return _ok('{"effect":"unverifiable"}')

    client = CuaDriverClient(binary="/usr/bin/cua-driver", runner=runner)
    result = await client.call(
        "click",
        {"pid": 3, "window_id": 9, "delivery_mode": "foreground"},
    )
    assert result.ok
    click = next(cmd for cmd in seen if cmd[1] == "call")
    assert "foreground" not in click[3]
    assert '"delivery_mode": "background"' in click[3]
    assert "--socket" in click

    with pytest.raises(ComputerUseError) as exc:
        await client.call("bring_to_front", {"pid": 3})
    assert exc.value.code == "DRIVER_TOOL_BLOCKED"
    with pytest.raises(ComputerUseError) as blocked:
        await client.call("launch_app", {"bundle_id": "com.apple.calculator"})
    assert blocked.value.code == "DRIVER_TOOL_BLOCKED"
    with pytest.raises(ComputerUseError) as ended:
        await client.call("end_session", {"session": "potato-obs"})
    assert ended.value.code == "DRIVER_TOOL_BLOCKED"

    await client.call(
        "set_value",
        {"pid": 3, "window_id": 9, "value": "42"},
    )
    set_value = [cmd for cmd in seen if cmd[1:3] == ["call", "set_value"]][-1]
    assert "delivery_mode" not in set_value[3]


@pytest.mark.asyncio
async def test_parse_embedded_json_from_banner() -> None:
    async def runner(cmd: list[str], _timeout: float) -> CuaCommandResult:
        if cmd[1] == "status":
            return _ok('{"running": true}')
        return _ok('✅ clicked\n{"effect":"unverifiable","pid":4}')

    client = CuaDriverClient(binary="cua-driver", runner=runner)
    result: CuaCallResult = await client.call("click", {"pid": 4, "window_id": 1})
    assert result.data["effect"] == "unverifiable"
    assert result.data["pid"] == 4


@pytest.mark.asyncio
async def test_starts_embedded_private_daemon() -> None:
    seen: list[list[str]] = []

    async def runner(cmd: list[str], _timeout: float) -> CuaCommandResult:
        seen.append(cmd)
        if cmd[1] == "status":
            if any(item == "serve" for item in (c[1] for c in seen)):
                return _ok("daemon running pid=9")
            return CuaCommandResult(returncode=1, stdout="not running", stderr="")
        if cmd[1] == "serve":
            return _ok("")
        return _ok("{}")

    client = CuaDriverClient(
        binary="/usr/bin/cua-driver",
        runner=runner,
        socket_path="/tmp/potato-test.sock",
    )
    await client.ensure_daemon()
    serve = next(cmd for cmd in seen if cmd[1] == "serve")
    assert "--embedded" in serve
    assert "--socket" in serve
    assert "/tmp/potato-test.sock" in serve
    assert "open" not in serve
    assert "CuaDriver" not in " ".join(serve)


@pytest.mark.asyncio
async def test_stop_asks_driver_to_quit() -> None:
    seen: list[list[str]] = []

    async def runner(cmd: list[str], _timeout: float) -> CuaCommandResult:
        seen.append(cmd)
        return _ok("")

    client = CuaDriverClient(
        binary="/usr/bin/cua-driver",
        runner=runner,
        socket_path="/tmp/potato-test.sock",
    )
    await client.stop()
    stop = next(cmd for cmd in seen if cmd[1] == "stop")
    assert "--socket" in stop
    assert "/tmp/potato-test.sock" in stop


@pytest.mark.asyncio
async def test_end_session_uses_official_revoke() -> None:
    seen: list[list[str]] = []

    async def runner(cmd: list[str], _timeout: float) -> CuaCommandResult:
        seen.append(cmd)
        return _ok("")

    client = CuaDriverClient(
        binary="/usr/bin/cua-driver",
        runner=runner,
        socket_path="/tmp/potato-test.sock",
    )
    await client.end_session("potato-obs_abc")
    revoke = next(cmd for cmd in seen if cmd[1] == "revoke")
    assert revoke == [
        "/usr/bin/cua-driver",
        "revoke",
        "--session",
        "potato-obs_abc",
        "--socket",
        "/tmp/potato-test.sock",
    ]
    assert not any(cmd[1] == "call" for cmd in seen)


@pytest.mark.asyncio
async def test_stop_waits_after_kill(monkeypatch) -> None:
    class _Process:
        returncode = None

        def __init__(self) -> None:
            self.wait_calls = 0
            self.killed = False

        def terminate(self) -> None:
            pass

        def kill(self) -> None:
            self.killed = True

        def wait(self):
            self.wait_calls += 1

            async def _wait() -> int:
                return -9

            return _wait()

    real_wait_for = __import__("asyncio").wait_for
    calls = 0

    async def _wait_for(awaitable, timeout):
        nonlocal calls
        calls += 1
        if calls == 1:
            awaitable.close()
            raise TimeoutError
        return await real_wait_for(awaitable, timeout)

    async def runner(_cmd: list[str], _timeout: float) -> CuaCommandResult:
        return _ok("")

    client = CuaDriverClient(binary="cua-driver", runner=runner)
    process = _Process()
    client._daemon = process  # type: ignore[assignment]
    monkeypatch.setattr("potato.computer_use.client.asyncio.wait_for", _wait_for)

    await client.stop()

    assert process.killed is True
    assert process.wait_calls == 2


@pytest.mark.asyncio
async def test_unwraps_structured_content() -> None:
    async def runner(cmd: list[str], _timeout: float) -> CuaCommandResult:
        if cmd[1] == "status":
            return _ok('{"running": true}')
        return _ok('{"structuredContent": {"elements": [{"element_index": 1}]}}')

    client = CuaDriverClient(binary="cua-driver", runner=runner)
    result = await client.call("get_window_state", {"pid": 4, "window_id": 1})
    assert result.data == {"elements": [{"element_index": 1}]}
