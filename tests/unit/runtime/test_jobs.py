# -*- coding: utf-8 -*-
# flake8: noqa: E501
"""Background job registry: start, read, kill, settle, admission."""
from __future__ import annotations

import asyncio

import pytest

from potato.runtime.jobs import (
    JobHooks,
    JobLimitError,
    JobOutcome,
    JobRegistry,
    JobStart,
    coerce_bool,
)


def test_coerce_bool_accepts_common_forms():
    assert coerce_bool(True) is True
    assert coerce_bool("true") is True
    assert coerce_bool("1") is True
    assert coerce_bool(0) is False
    assert coerce_bool("no") is False
    with pytest.raises(ValueError):
        coerce_bool("maybe", field_name="wait")


async def test_start_read_and_settle():
    registry = JobRegistry()
    loop = asyncio.get_running_loop()
    done = loop.create_future()
    chunks = ["hello", " world"]

    def run() -> JobHooks:
        state = {"i": 0}

        def read() -> str:
            if state["i"] >= len(chunks):
                return ""
            text = chunks[state["i"]]
            state["i"] += 1
            return text

        def cancel() -> None:
            if not done.done():
                done.set_result(JobOutcome(status="killed", detail="killed"))

        return JobHooks(cancel=cancel, done=done, read_output=read)

    job_id = await registry.start(
        JobStart(kind="bash", label="echo", run=run, owner_session="s1"),
    )
    assert job_id == "bash-1"
    text, snap = await registry.read(job_id, session_id="s1")
    assert text == "hello"
    assert snap.status == "running"
    done.set_result(JobOutcome(status="completed", detail="exit code: 0"))
    await asyncio.sleep(0)
    await asyncio.sleep(0)
    snap = await registry.get(job_id, session_id="s1")
    assert snap.status == "completed"
    assert snap.detail == "exit code: 0"


async def test_kill_is_idempotent_and_marks_reported():
    registry = JobRegistry()
    loop = asyncio.get_running_loop()
    done = loop.create_future()

    def run() -> JobHooks:
        def cancel() -> None:
            if not done.done():
                done.set_result(JobOutcome(status="killed", detail="signal: SIGTERM"))

        return JobHooks(cancel=cancel, done=done, read_output=lambda: "")

    job_id = await registry.start(JobStart(kind="bash", label="sleep", run=run))
    assert await registry.kill(job_id) == "requested"
    await asyncio.sleep(0)
    await asyncio.sleep(0)
    assert await registry.kill(job_id) == "already-finished"
    snap = await registry.get(job_id)
    assert snap.status == "killed"
    assert snap.reported is True


async def test_owner_admission_limit():
    registry = JobRegistry(max_concurrent=1)
    loop = asyncio.get_running_loop()
    parked = loop.create_future()

    def run() -> JobHooks:
        return JobHooks(
            cancel=lambda: None,
            done=parked,
            read_output=lambda: "",
        )

    await registry.start(
        JobStart(kind="bash", label="one", run=run, owner_session="s1"),
    )
    with pytest.raises(JobLimitError):
        await registry.start(
            JobStart(kind="bash", label="two", run=run, owner_session="s1"),
        )


async def test_foreign_session_cannot_read():
    registry = JobRegistry()
    loop = asyncio.get_running_loop()
    parked = loop.create_future()

    def run() -> JobHooks:
        return JobHooks(cancel=lambda: None, done=parked, read_output=lambda: "x")

    job_id = await registry.start(
        JobStart(kind="bash", label="secret", run=run, owner_session="alice"),
    )
    with pytest.raises(Exception):
        await registry.read(job_id, session_id="bob")


async def test_wait_timeout_leaves_job_running():
    registry = JobRegistry()
    loop = asyncio.get_running_loop()
    parked = loop.create_future()

    def run() -> JobHooks:
        return JobHooks(cancel=lambda: None, done=parked, read_output=lambda: "")

    job_id = await registry.start(JobStart(kind="bash", label="wait", run=run))
    snap = await registry.wait(job_id, 20)
    assert snap.status == "running"
    assert snap.reported is False
