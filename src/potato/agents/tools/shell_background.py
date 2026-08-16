# -*- coding: utf-8 -*-
"""Detach a shell process from the producing tool call.

The process is owned by the job registry after start(). Tool-call cancel
does not kill it; only job_kill / owner teardown / shutdown do.
"""
from __future__ import annotations

import asyncio
import signal
import sys
import threading
from typing import Callable

from . import shell as shell_mod
from ...runtime.jobs import JobHooks, JobOutcome


class BackgroundShell:
    """Live process + consuming output cursor."""

    def __init__(self) -> None:
        self._stdout = bytearray()
        self._stderr = bytearray()
        self._cursor = 0
        self._status = "running"
        self._exit_code: int | None = None
        self._signal: str | None = None
        self._done: asyncio.Future[JobOutcome] | None = None
        self._kill: Callable[[], None] = lambda: None

    def hooks(self) -> JobHooks:
        if self._done is None:
            raise RuntimeError("background shell has not started")
        return JobHooks(
            cancel=self.kill,
            done=self._done,
            read_output=self.read_output,
        )

    def read_output(self) -> str:
        preview = shell_mod._live_preview(self._stdout, self._stderr)
        delta = preview[self._cursor:]
        self._cursor = len(preview)
        return delta

    def kill(self) -> None:
        self._kill()

    def _finish(self, outcome: JobOutcome) -> None:
        if self._done is not None and not self._done.done():
            self._done.set_result(outcome)


async def start_background_shell(
    cmd: str,
    working_dir: str,
    env: dict,
    shell_executable: str | None,
) -> BackgroundShell:
    """Spawn a process that outlives the current tool call."""
    handle = BackgroundShell()
    loop = asyncio.get_running_loop()
    handle._done = loop.create_future()
    if sys.platform == "win32":
        _start_windows(handle, cmd, working_dir, env, shell_executable, loop)
    else:
        await _start_posix(handle, cmd, working_dir, env, shell_executable)
    return handle


async def _start_posix(
    handle: BackgroundShell,
    cmd: str,
    working_dir: str,
    env: dict,
    shell_executable: str | None,
) -> None:
    proc = await asyncio.create_subprocess_shell(
        cmd,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
        bufsize=0,
        cwd=working_dir,
        env=env,
        start_new_session=True,
        executable=shell_executable,
    )

    def _kill() -> None:
        asyncio.create_task(shell_mod._kill_posix_pg(proc))

    handle._kill = _kill

    async def _collect() -> None:
        readers = [
            asyncio.create_task(
                shell_mod._read_pipe(proc.stdout, handle._stdout),
            ),
            asyncio.create_task(
                shell_mod._read_pipe(proc.stderr, handle._stderr),
            ),
        ]
        returncode = await proc.wait()
        await asyncio.gather(*readers, return_exceptions=True)
        if returncode is None:
            outcome = JobOutcome(status="killed", detail="killed before exit")
        elif returncode < 0:
            try:
                name = signal.Signals(-returncode).name
            except ValueError:
                name = str(-returncode)
            outcome = JobOutcome(status="killed", detail=f"signal: {name}")
        else:
            outcome = JobOutcome(
                status="completed",
                detail=f"exit code: {returncode}",
            )
        handle._exit_code = returncode
        handle._finish(outcome)

    asyncio.create_task(_collect(), name=f"bg-shell-{proc.pid}")


def _start_windows(
    handle: BackgroundShell,
    cmd: str,
    working_dir: str,
    env: dict,
    shell_executable: str | None,
    loop: asyncio.AbstractEventLoop,
) -> None:
    cancel_event = threading.Event()

    def _kill() -> None:
        cancel_event.set()

    handle._kill = _kill

    def _worker() -> None:
        returncode, stdout, stderr = shell_mod._execute_subprocess_sync(
            cmd,
            working_dir,
            timeout=24 * 3600,
            env=env,
            shell_executable=shell_executable,
            cancel_event=cancel_event,
        )
        handle._stdout.extend(stdout.encode("utf-8", errors="replace"))
        handle._stderr.extend(stderr.encode("utf-8", errors="replace"))
        handle._exit_code = returncode
        if cancel_event.is_set() and returncode != 0:
            outcome = JobOutcome(status="killed", detail="killed before exit")
        else:
            outcome = JobOutcome(
                status="completed",
                detail=f"exit code: {returncode}",
            )
        loop.call_soon_threadsafe(handle._finish, outcome)

    threading.Thread(target=_worker, name="bg-shell-win", daemon=True).start()
