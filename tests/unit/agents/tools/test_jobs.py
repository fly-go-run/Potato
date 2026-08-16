# -*- coding: utf-8 -*-
# flake8: noqa: E501
"""job_output / job_list / job_kill against the process registry."""
from __future__ import annotations

import asyncio

import pytest

from potato.agents.tools.jobs import job_kill, job_list, job_output
from potato.runtime.jobs import JobHooks, JobOutcome, JobRegistry, JobStart, set_job_registry


@pytest.fixture
def registry():
    reg = JobRegistry()
    set_job_registry(reg)
    yield reg
    set_job_registry(None)


def _text(chunk) -> str:
    return chunk.content[0]["text"] if isinstance(chunk.content[0], dict) else chunk.content[0].text


async def test_job_list_empty(registry):
    chunk = await job_list()
    assert "(no background jobs)" in _text(chunk)


async def test_job_output_and_kill(registry):
    loop = asyncio.get_running_loop()
    done = loop.create_future()

    def run() -> JobHooks:
        def cancel() -> None:
            if not done.done():
                done.set_result(JobOutcome(status="killed", detail="killed"))

        return JobHooks(cancel=cancel, done=done, read_output=lambda: "tick")

    job_id = await registry.start(JobStart(kind="bash", label="tick", run=run))
    listed = await job_list()
    assert job_id in _text(listed)
    out = await job_output(job_id)
    assert "tick" in _text(out)
    assert "[status: running]" in _text(out)
    killed = await job_kill(job_id)
    assert "requested cancellation" in _text(killed)
    await asyncio.sleep(0)
    await asyncio.sleep(0)
    finished = await job_output(job_id)
    assert "killed" in _text(finished)
