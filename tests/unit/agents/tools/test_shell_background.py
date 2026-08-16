# -*- coding: utf-8 -*-
"""execute_shell_command(run_in_background=True) returns a live job."""
from __future__ import annotations

import asyncio
import sys

import pytest

from potato.agents.tools.jobs import job_output
from potato.agents.tools.shell import execute_shell_command
from potato.runtime.jobs import JobRegistry, set_job_registry


@pytest.fixture
def registry():
    reg = JobRegistry()
    set_job_registry(reg)
    yield reg
    set_job_registry(None)


def _text(chunk) -> str:
    block = chunk.content[0]
    return block["text"] if isinstance(block, dict) else block.text


@pytest.mark.skipif(sys.platform == "win32", reason="uses POSIX sleep")
async def test_background_echo_is_collectable(registry, tmp_path):
    chunk = await execute_shell_command(
        "printf 'bg-ok\\n'",
        cwd=tmp_path,
        run_in_background=True,
    )
    text = _text(chunk)
    assert "started background job bash-1" in text
    for _ in range(40):
        out = await job_output("bash-1", wait=True, timeout_ms=100)
        body = _text(out)
        if "bg-ok" in body and "completed" in body:
            return
        await asyncio.sleep(0.05)
    pytest.fail(_text(await job_output("bash-1")))


async def test_background_rejects_unknown_flag(registry):
    chunk = await execute_shell_command("echo x", run_in_background="maybe")
    assert "run_in_background" in _text(chunk)
