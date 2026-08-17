# -*- coding: utf-8 -*-
"""Thin Cua Driver CLI client. Never exposes foreground escalation."""

from __future__ import annotations

import asyncio
import contextlib
import json
import logging
import os
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Awaitable, Callable

from .bundle import (
    HOST_BUNDLE_ID,
    daemon_socket_path,
    ensure_driver_binary,
    resolve_cua_driver_binary,
)
from .constants import (
    ALLOWED_DRIVER_TOOLS,
    DAEMON_START_TIMEOUT_SECONDS,
    DRIVER_CALL_TIMEOUT_SECONDS,
    INPUT_DRIVER_TOOLS,
)
from .errors import ComputerUseError

logger = logging.getLogger(__name__)

Runner = Callable[[list[str], float], Awaitable["CuaCommandResult"]]


@dataclass(frozen=True)
class CuaCommandResult:
    returncode: int
    stdout: str
    stderr: str


@dataclass(frozen=True)
class CuaCallResult:
    ok: bool
    text: str
    data: Any = None
    screenshot_path: str | None = None
    raw_stdout: str = ""


@dataclass
class CuaDriverClient:
    """Call the Potato-owned cua-driver daemon over a private socket."""

    binary: str
    runner: Runner | None = None
    socket_path: str = ""
    _daemon_checked: bool = field(default=False, init=False)
    _daemon: asyncio.subprocess.Process | None = field(default=None, init=False, repr=False)
    _lock: asyncio.Lock | None = field(default=None, init=False, repr=False)

    def __post_init__(self) -> None:
        if not self.socket_path:
            self.socket_path = daemon_socket_path()
        self._lock = asyncio.Lock()

    @classmethod
    def from_config(cls, driver_path: str = "") -> "CuaDriverClient":
        try:
            resolved = ensure_driver_binary(driver_path)
        except Exception as exc:
            raise ComputerUseError(
                "DRIVER_MISSING",
                "Potato could not prepare its built-in computer-use driver. "
                "Check the network and try again.",
            ) from exc
        if not resolved:
            raise ComputerUseError(
                "DRIVER_MISSING",
                "Potato could not prepare its built-in computer-use driver.",
            )
        return cls(binary=resolved)

    async def call(
        self,
        tool: str,
        args: dict[str, Any] | None = None,
        *,
        screenshot_path: str | None = None,
        timeout: float = DRIVER_CALL_TIMEOUT_SECONDS,
    ) -> CuaCallResult:
        if tool not in ALLOWED_DRIVER_TOOLS:
            raise ComputerUseError(
                "DRIVER_TOOL_BLOCKED",
                f"Potato will not call Cua tool {tool!r}.",
            )
        payload = dict(args or {})
        if tool in INPUT_DRIVER_TOOLS:
            payload["delivery_mode"] = "background"
        await self.ensure_daemon()
        cmd = [self.binary, "call", tool]
        if payload:
            cmd.append(json.dumps(payload, ensure_ascii=False))
        cmd.extend(["--socket", self.socket_path])
        if screenshot_path:
            cmd.extend(["--screenshot-out-file", screenshot_path])
        result = await self._run(cmd, timeout)
        if result.returncode != 0:
            raise ComputerUseError(
                "DRIVER_CALL_FAILED",
                _first_line(result.stderr or result.stdout)
                or f"cua-driver call {tool} failed ({result.returncode})",
            )
        parsed = _parse_call_output(result.stdout)
        parsed["data"] = _unwrap_driver_payload(parsed.get("data"))
        shot = (
            screenshot_path
            if screenshot_path and Path(screenshot_path).is_file()
            else None
        )
        return CuaCallResult(
            ok=True,
            text=parsed.get("text") or result.stdout.strip(),
            data=parsed.get("data"),
            screenshot_path=shot,
            raw_stdout=result.stdout,
        )

    async def doctor(self) -> dict[str, Any]:
        result = await self._run(
            [self.binary, "doctor", "--json"],
            timeout=20.0,
        )
        data = _coerce_json(result.stdout)
        return data if isinstance(data, dict) else {"raw": result.stdout}

    async def version(self) -> str:
        result = await self._run([self.binary, "--version"], timeout=8.0)
        return (result.stdout or result.stderr).strip()

    async def end_session(self, session_id: str) -> None:
        """Revoke one named daemon session. Official 0.20.0 CLI verb."""
        session = (session_id or "").strip()
        if not session:
            return
        try:
            await self._run(
                [
                    self.binary,
                    "revoke",
                    "--session",
                    session,
                    "--socket",
                    self.socket_path,
                ],
                8.0,
            )
        except Exception:
            logger.debug("cua-driver revoke failed", exc_info=True)

    async def stop(self) -> None:
        try:
            await self._run(
                [self.binary, "stop", "--socket", self.socket_path],
                timeout=5.0,
            )
        except Exception:
            logger.debug("cua-driver stop failed", exc_info=True)
        daemon = self._daemon
        self._daemon = None
        self._daemon_checked = False
        if daemon is not None and daemon.returncode is None:
            daemon.terminate()
            try:
                await asyncio.wait_for(daemon.wait(), timeout=3.0)
            except TimeoutError:
                with contextlib.suppress(ProcessLookupError):
                    daemon.kill()
                with contextlib.suppress(TimeoutError, ProcessLookupError):
                    await asyncio.wait_for(daemon.wait(), timeout=1.0)
            except ProcessLookupError:
                pass

    async def ensure_daemon(self) -> None:
        lock = self._lock or asyncio.Lock()
        self._lock = lock
        async with lock:
            await self._ensure_daemon_locked()

    async def _ensure_daemon_locked(self) -> None:
        if self._daemon is not None and self._daemon.returncode is None:
            return
        if self.runner is not None:
            status = await self._run(
                [self.binary, "status", "--socket", self.socket_path],
                timeout=8.0,
            )
            if status.returncode == 0 and _status_looks_running(status.stdout):
                self._daemon_checked = True
                return
        elif self._daemon is not None and self._daemon.returncode is not None:
            self._daemon = None
        await self._start_daemon()
        deadline = asyncio.get_running_loop().time() + DAEMON_START_TIMEOUT_SECONDS
        while asyncio.get_running_loop().time() < deadline:
            status = await self._run(
                [self.binary, "status", "--socket", self.socket_path],
                timeout=5.0,
            )
            if status.returncode == 0 and _status_looks_running(status.stdout):
                self._daemon_checked = True
                return
            await asyncio.sleep(0.4)
        await self.stop()
        raise ComputerUseError(
            "DRIVER_DAEMON",
            "Potato's computer-use driver did not start. Grant Accessibility "
            "and Screen Recording to Potato in System Settings, then retry.",
        )

    async def _start_daemon(self) -> None:
        sock_parent = Path(self.socket_path).parent
        if not str(self.socket_path).startswith(r"\\.\pipe"):
            sock_parent.mkdir(parents=True, exist_ok=True)
            if Path(self.socket_path).exists():
                try:
                    Path(self.socket_path).unlink()
                except OSError:
                    pass
        if self.runner is not None:
            await self.runner(
                [
                    self.binary,
                    "serve",
                    "--embedded",
                    "--socket",
                    self.socket_path,
                    "--host-bundle-id",
                    HOST_BUNDLE_ID,
                ],
                8.0,
            )
            return
        try:
            self._daemon = await asyncio.create_subprocess_exec(
                self.binary,
                "serve",
                "--embedded",
                "--socket",
                self.socket_path,
                "--host-bundle-id",
                HOST_BUNDLE_ID,
                stdout=asyncio.subprocess.DEVNULL,
                stderr=asyncio.subprocess.DEVNULL,
                env=_driver_env(),
            )
        except OSError as exc:
            raise ComputerUseError(
                "DRIVER_DAEMON",
                f"Could not start Potato's computer-use driver: {exc}",
            ) from exc

    async def _run(self, cmd: list[str], timeout: float) -> CuaCommandResult:
        if self.runner is not None:
            return await self.runner(cmd, timeout)
        return await _run_subprocess(cmd, timeout)


def _driver_env() -> dict[str, str]:
    env = os.environ.copy()
    env["CUA_DRIVER_RS_TELEMETRY_ENABLED"] = "0"
    env["CUA_DRIVER_EMBEDDED"] = "1"
    env["CUA_DRIVER_HOST_BUNDLE_ID"] = HOST_BUNDLE_ID
    return env


async def _run_subprocess(cmd: list[str], timeout: float) -> CuaCommandResult:
    try:
        proc = await asyncio.create_subprocess_exec(
            *cmd,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            env=_driver_env(),
        )
        stdout_b, stderr_b = await asyncio.wait_for(
            proc.communicate(),
            timeout=timeout,
        )
    except TimeoutError as exc:
        with contextlib.suppress(ProcessLookupError):
            proc.kill()
        with contextlib.suppress(Exception):
            await proc.communicate()
        raise ComputerUseError(
            "DRIVER_TIMEOUT",
            f"Cua Driver timed out: {' '.join(cmd[:3])}",
        ) from exc
    except FileNotFoundError as exc:
        raise ComputerUseError(
            "DRIVER_MISSING",
            f"Could not execute {cmd[0]!r}.",
        ) from exc
    return CuaCommandResult(
        returncode=int(proc.returncode or 0),
        stdout=(stdout_b or b"").decode("utf-8", errors="replace"),
        stderr=(stderr_b or b"").decode("utf-8", errors="replace"),
    )


def _parse_call_output(stdout: str) -> dict[str, Any]:
    text = stdout.strip()
    data = _coerce_json(text)
    if isinstance(data, dict):
        return {"text": text, "data": data}
    if isinstance(data, list):
        return {"text": text, "data": data}
    embedded = _extract_json_object(text)
    if embedded is not None:
        return {"text": text, "data": embedded}
    return {"text": text, "data": None}


def _coerce_json(text: str) -> Any:
    blob = text.strip()
    if not blob:
        return None
    try:
        return json.loads(blob)
    except json.JSONDecodeError:
        return None


def _extract_json_object(text: str) -> Any:
    start = text.find("{")
    end = text.rfind("}")
    if start >= 0 and end > start:
        try:
            return json.loads(text[start : end + 1])
        except json.JSONDecodeError:
            return None
    start = text.find("[")
    end = text.rfind("]")
    if start >= 0 and end > start:
        try:
            return json.loads(text[start : end + 1])
        except json.JSONDecodeError:
            return None
    return None


def _unwrap_driver_payload(data: Any) -> Any:
    if isinstance(data, dict):
        structured = data.get("structuredContent")
        if isinstance(structured, (dict, list)):
            return structured
    return data


def _status_looks_running(stdout: str) -> bool:
    data = _coerce_json(stdout)
    if isinstance(data, dict) and "running" in data:
        return bool(data["running"])
    lowered = stdout.lower()
    if "not running" in lowered or "stopped" in lowered:
        return False
    return "daemon running" in lowered or "is running" in lowered


def _first_line(text: str) -> str:
    for line in text.splitlines():
        stripped = line.strip()
        if stripped:
            return stripped
    return ""


# Re-export for existing imports.
__all__ = [
    "CuaCallResult",
    "CuaCommandResult",
    "CuaDriverClient",
    "resolve_cua_driver_binary",
    "ensure_driver_binary",
]
