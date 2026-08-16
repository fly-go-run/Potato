# -*- coding: utf-8 -*-
# flake8: noqa: E501
"""Process-local background job registry (DeepSeek Harness-shaped).

Producers (shell, later subagents) register work here. The registry owns
ids, isolation, incremental reads, kill, and settlement. Process lifetime
is independent of the producing tool call.
"""
from __future__ import annotations

import asyncio
import inspect
import logging
import time
from dataclasses import dataclass
from typing import Any, Awaitable, Callable, Literal

logger = logging.getLogger(__name__)

JobStatus = Literal["running", "stopping", "completed", "killed", "failed"]
JobKind = str

DEFAULT_MAX_CONCURRENT = 10
MAX_CONSECUTIVE_WAKES = 3


def coerce_bool(value: Any, *, default: bool = False, field_name: str = "flag") -> bool:
    """Accept real bools and common LLM string/int forms."""
    if value is None:
        return default
    if isinstance(value, bool):
        return value
    if isinstance(value, int) and value in (0, 1):
        return bool(value)
    if isinstance(value, str):
        lowered = value.strip().lower()
        if lowered in {"true", "1", "yes", "on"}:
            return True
        if lowered in {"false", "0", "no", "off", ""}:
            return False
    raise ValueError(f"{field_name} must be a boolean, got {value!r}")


@dataclass(frozen=True)
class JobOutcome:
    status: Literal["completed", "killed", "failed"]
    detail: str = ""
    output: str = ""


@dataclass
class JobHooks:
    cancel: Callable[[], None]
    done: Awaitable[JobOutcome]
    read_output: Callable[[], str] | None = None


@dataclass
class JobStart:
    kind: JobKind
    label: str
    run: Callable[[], JobHooks | Awaitable[JobHooks]]
    owner_session: str = ""
    owner_agent: str = ""
    owner_user: str = ""
    owner_channel: str = "console"


@dataclass(frozen=True)
class JobSnapshot:
    id: str
    kind: JobKind
    label: str
    status: JobStatus
    detail: str
    started_at: float
    finished_at: float | None
    reported: bool
    owner_session: str
    owner_agent: str
    owner_user: str
    owner_channel: str


@dataclass
class _Tracked:
    id: str
    kind: JobKind
    label: str
    owner_session: str
    owner_agent: str
    owner_user: str
    owner_channel: str
    cancel: Callable[[], None]
    read_output: Callable[[], str] | None
    status: JobStatus
    detail: str
    output: str
    started_at: float
    finished_at: float | None
    reported: bool
    settled: asyncio.Future
    waiters: int = 0


JobDoneListener = Callable[[JobSnapshot], Awaitable[None] | None]


class JobLimitError(RuntimeError):
    """Owner already has the maximum number of live jobs."""


class JobAccessError(RuntimeError):
    """Caller is not allowed to see this job."""


class JobRegistry:
    """In-memory job table. Snapshots are copies; never leak live records."""

    def __init__(self, *, max_concurrent: int = DEFAULT_MAX_CONCURRENT) -> None:
        self._max_concurrent = max_concurrent
        self._store: dict[str, _Tracked] = {}
        self._counters: dict[str, int] = {}
        self._listeners: list[JobDoneListener] = []
        self._lock = asyncio.Lock()
        self._spent_wakes: dict[str, int] = {}

    def on_job_done(self, listener: JobDoneListener) -> None:
        self._listeners.append(listener)

    def reset_wake_budget(self, session_id: str) -> None:
        if session_id:
            self._spent_wakes.pop(session_id, None)

    def consume_wake(self, session_id: str, *, budget: int = MAX_CONSECUTIVE_WAKES) -> bool:
        if not session_id:
            return False
        spent = self._spent_wakes.get(session_id, 0)
        if spent >= budget:
            return False
        self._spent_wakes[session_id] = spent + 1
        return True

    async def start(self, spec: JobStart) -> str:
        if not spec.kind or not spec.label:
            raise ValueError("job kind and label are required")
        async with self._lock:
            active = sum(
                1
                for job in self._store.values()
                if job.owner_session == spec.owner_session
                and job.status in ("running", "stopping")
            )
            if spec.owner_session and active >= self._max_concurrent:
                raise JobLimitError(
                    f"background job limit reached ({self._max_concurrent}); "
                    "use job_kill, wait for a job to finish, then retry",
                )
        hooks = spec.run()
        if inspect.isawaitable(hooks):
            hooks = await hooks
        async with self._lock:
            count = self._counters.get(spec.kind, 0) + 1
            self._counters[spec.kind] = count
            job_id = f"{spec.kind}-{count}"
            loop = asyncio.get_running_loop()
            job = _Tracked(
                id=job_id,
                kind=spec.kind,
                label=spec.label,
                owner_session=spec.owner_session,
                owner_agent=spec.owner_agent,
                owner_user=spec.owner_user,
                owner_channel=spec.owner_channel or "console",
                cancel=hooks.cancel,
                read_output=hooks.read_output,
                status="running",
                detail="",
                output="",
                started_at=time.time(),
                finished_at=None,
                reported=False,
                settled=loop.create_future(),
            )
            self._store[job_id] = job
        asyncio.create_task(self._watch(job, hooks.done), name=f"job-{job_id}")
        return job_id

    async def list(self, *, session_id: str = "") -> list[JobSnapshot]:
        async with self._lock:
            jobs = [
                job
                for job in self._store.values()
                if not session_id or job.owner_session == session_id
            ]
            return [self._snapshot(job) for job in jobs]

    async def get(self, job_id: str, *, session_id: str = "") -> JobSnapshot:
        async with self._lock:
            return self._snapshot(self._expect(job_id, session_id))

    async def read(self, job_id: str, *, session_id: str = "") -> tuple[str, JobSnapshot]:
        async with self._lock:
            job = self._expect(job_id, session_id)
            if job.read_output is not None:
                text = job.read_output()
            elif _is_terminal(job.status):
                text = job.output
            else:
                text = ""
            if _is_terminal(job.status):
                job.reported = True
            return text, self._snapshot(job)

    async def kill(self, job_id: str, *, session_id: str = "") -> str:
        async with self._lock:
            job = self._expect(job_id, session_id)
            if _is_terminal(job.status):
                job.reported = True
                return "already-finished"
            job.cancel()
            job.status = "stopping"
            job.reported = True
            return "requested"

    async def wait(
        self,
        job_id: str,
        timeout_ms: float,
        *,
        session_id: str = "",
    ) -> JobSnapshot:
        if timeout_ms <= 0:
            raise ValueError("timeout_ms must be positive")
        async with self._lock:
            job = self._expect(job_id, session_id)
            if _is_terminal(job.status):
                job.reported = True
                return self._snapshot(job)
            job.waiters += 1
            settled = job.settled
        try:
            await asyncio.wait_for(asyncio.shield(settled), timeout=timeout_ms / 1000.0)
        except asyncio.TimeoutError:
            async with self._lock:
                job.waiters = max(0, job.waiters - 1)
                return self._snapshot(self._expect(job_id, session_id))
        async with self._lock:
            job.waiters = max(0, job.waiters - 1)
            job.reported = True
            return self._snapshot(job)

    async def shutdown(self) -> None:
        async with self._lock:
            live = [job for job in self._store.values() if not _is_terminal(job.status)]
        for job in live:
            try:
                job.cancel()
            except Exception:  # pylint: disable=broad-except
                logger.warning("job %s cancel during shutdown failed", job.id, exc_info=True)
            job.reported = True
        if live:
            await asyncio.sleep(0)

    async def _watch(self, job: _Tracked, done: Awaitable[JobOutcome]) -> None:
        try:
            outcome = await done
        except Exception as exc:  # pylint: disable=broad-except
            logger.warning("job %s producer done rejected: %s", job.id, exc)
            outcome = JobOutcome(status="failed", detail=str(exc))
        await self._settle(job, outcome)

    async def _settle(self, job: _Tracked, outcome: JobOutcome) -> None:
        async with self._lock:
            if _is_terminal(job.status):
                return
            job.status = outcome.status
            job.detail = outcome.detail
            job.output = outcome.output
            job.finished_at = time.time()
            if job.waiters > 0:
                job.reported = True
            snapshot = self._snapshot(job)
            if not job.settled.done():
                job.settled.set_result(None)
        for listener in list(self._listeners):
            try:
                result = listener(snapshot)
                if inspect.isawaitable(result):
                    await result
            except Exception:  # pylint: disable=broad-except
                logger.warning("job done listener failed for %s", job.id, exc_info=True)

    def _expect(self, job_id: str, session_id: str) -> _Tracked:
        job = self._store.get(job_id)
        if job is None:
            raise KeyError(f"unknown job {job_id}")
        if session_id and job.owner_session and job.owner_session != session_id:
            raise JobAccessError(f"job {job_id} belongs to another session")
        return job

    @staticmethod
    def _snapshot(job: _Tracked) -> JobSnapshot:
        return JobSnapshot(
            id=job.id,
            kind=job.kind,
            label=job.label,
            status=job.status,
            detail=job.detail,
            started_at=job.started_at,
            finished_at=job.finished_at,
            reported=job.reported,
            owner_session=job.owner_session,
            owner_agent=job.owner_agent,
            owner_user=job.owner_user,
            owner_channel=job.owner_channel,
        )


def _is_terminal(status: JobStatus) -> bool:
    return status in ("completed", "killed", "failed")


_registry: JobRegistry | None = None


def get_job_registry() -> JobRegistry:
    global _registry
    if _registry is None:
        _registry = JobRegistry()
    return _registry


def set_job_registry(registry: JobRegistry | None) -> None:
    global _registry
    _registry = registry
