# -*- coding: utf-8 -*-
# flake8: noqa: E501
# pylint: disable=line-too-long
"""The shell command tool."""

import asyncio
import locale
import os
import re
import signal
import subprocess
import sys
import tempfile
import threading
import time
from dataclasses import replace
from pathlib import Path
from typing import Any, AsyncGenerator, Optional

from agentscope.message import TextBlock, ToolResultState
from agentscope.tool import ToolChunk

from ...config.context import (
    get_current_shell_command_executable,
    get_current_shell_command_timeout,
    get_current_workspace_dir,
)
from ...constant import WORKING_DIR
from ...runtime.tool_registry import tool_descriptor
from ...runtime.tool_meta import build_qp_meta
from ...sandbox import ExecutionResult


def _shell_metadata(
    *,
    sandboxed: bool,
    ok: bool,
    exit_code: int | None = None,
    violation: str | None = None,
) -> dict:
    data: dict[str, Any] = {"sandboxed": sandboxed}
    if exit_code is not None:
        data["exit_code"] = exit_code
    if violation is not None:
        data["violation"] = violation
    return {"qp": build_qp_meta("shell", ok, data)}


def _windows_taskkill_args(pid: int) -> list[str]:
    """Return the native command that force-kills a Windows process tree."""
    return ["taskkill", "/F", "/T", "/PID", str(pid)]


def _kill_process_tree_win32(pid: int) -> None:
    """Kill a process and all its descendants on Windows via taskkill.

    Uses ``taskkill /F /T`` which forcefully terminates the entire process
    tree, including grandchild processes that ``Popen.kill()`` would miss.
    """
    try:
        subprocess.call(
            _windows_taskkill_args(pid),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=10,
        )
    except Exception:
        pass


def _windows_shell_creationflags() -> int:
    """Return Windows process flags for shell commands."""
    return getattr(subprocess, "CREATE_NEW_PROCESS_GROUP", 0)


def _wait_process_win32(
    proc: subprocess.Popen,
    timeout: float,
    cancel_event: threading.Event | None,
) -> str:
    """Wait for a Windows process while observing cross-thread cancellation."""
    deadline = time.monotonic() + max(timeout, 0.0)
    while True:
        if cancel_event is not None and cancel_event.is_set():
            return "cancelled"
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return "timed_out"
        try:
            proc.wait(timeout=min(0.1, remaining))
            return "completed"
        except subprocess.TimeoutExpired:
            continue


def _collapse_newlines_outside_quotes(cmd: str) -> str:
    r"""Collapse newlines outside quoted strings; preserve those inside.

    Used only on Unix where sh/bash correctly handles newlines in quotes.
    Handles backslash-newline (line continuation) by removing both chars,
    and treats single-quoted content as fully literal per POSIX.
    """
    result: list[str] = []
    in_single_quote = False
    in_double_quote = False
    i = 0
    length = len(cmd)

    while i < length:
        char = cmd[i]

        # Toggle quote state
        if char == "'" and not in_double_quote:
            in_single_quote = not in_single_quote
            result.append(char)
            i += 1
            continue

        if char == '"' and not in_single_quote:
            in_double_quote = not in_double_quote
            result.append(char)
            i += 1
            continue

        # Inside single quotes: everything is literal (POSIX)
        if in_single_quote:
            result.append(char)
            i += 1
            continue

        # Backslash-newline (line continuation): remove both chars
        if char == "\\" and i + 1 < length and cmd[i + 1] in ("\r", "\n"):
            i += 2
            # \r\n sequence: skip the \n as well
            if i < length and cmd[i - 1] == "\r" and cmd[i] == "\n":
                i += 1
            continue

        # Backslash escape (non-newline): keep both chars
        if char == "\\" and i + 1 < length:
            result.append(char)
            result.append(cmd[i + 1])
            i += 2
            continue

        # Newlines
        if char in ("\r", "\n"):
            if in_double_quote:
                # Preserve newlines inside double quotes
                result.append(char)
            else:
                # Collapse \r\n as a single space
                if char == "\r" and i + 1 < length and cmd[i + 1] == "\n":
                    i += 1
                result.append(" ")
            i += 1
            continue

        result.append(char)
        i += 1

    return "".join(result)


def _collapse_embedded_newlines(
    cmd: str,
    shell_executable: str | None = None,
) -> str:
    r"""Normalize embedded newlines for the configured shell.

    LLMs produce tool-call arguments in JSON where ``\n`` is parsed as an
    actual newline character.  In the original shell command the user
    intended the *literal* two-character sequence ``\n`` (e.g. inside a
    ``--content`` flag), but after JSON decoding it becomes a real line
    break.  When passed to a shell:

    * **Windows** ``cmd.exe`` truncates the command at the first newline
      regardless of quoting context, so all newlines must be collapsed.
      PowerShell supports multiline scripts, so its newlines are preserved.
    * **Unix** ``sh -c`` treats an unquoted newline as a command separator,
      but correctly handles newlines inside quoted strings.

    On Unix/macOS, newlines inside quoted strings are preserved so that
    downstream commands receive the correct multi-line content (e.g.
    ``--text "Hello\nWorld"``).  On Windows, a missing or unrecognized shell
    uses the conservative ``cmd.exe``-compatible behavior.
    """
    if "\n" not in cmd:
        return cmd
    if sys.platform == "win32":
        if shell_executable and _is_powershell(shell_executable):
            return cmd
        # cmd.exe (and unknown cmd-like Windows shells) truncate at newlines.
        return cmd.replace("\r\n", " ").replace("\n", " ")
    return _collapse_newlines_outside_quotes(cmd)


def _sanitize_win_cmd(cmd: str) -> str:
    """Fix common LLM escaping artefacts for Windows ``cmd.exe``.

    LLMs sometimes produce commands with backslash-escaped double quotes
    (``\\"``) — valid in bash/JSON but meaningless to ``cmd.exe``.  When
    *every* double-quote in the command is preceded by a backslash, it is
    almost certainly a double-escape artefact, so we strip them.
    """
    if '\\"' in cmd and '"' not in cmd.replace('\\"', ""):
        return cmd.replace('\\"', '"')
    return cmd


def _read_temp_file(path: str) -> str:
    """Read a temporary output file and return its decoded content."""
    try:
        with open(path, "rb") as f:
            return smart_decode(f.read())
    except OSError:
        return ""


def _shell_basename(executable: str) -> str:
    """Extract lowercase basename from a path using both / and \\ separators."""
    return executable.replace("\\", "/").rsplit("/", 1)[-1].lower()


def _is_powershell(executable: str) -> bool:
    """Check if the given executable path is a PowerShell variant."""
    return _shell_basename(executable) in (
        "powershell",
        "powershell.exe",
        "pwsh",
        "pwsh.exe",
    )


def _is_cmd(executable: str) -> bool:
    """Check if the given executable path is cmd.exe."""
    return _shell_basename(executable) in ("cmd", "cmd.exe")


_PS_CMD_RE = re.compile(
    r"^(powershell(?:\.exe)?|pwsh(?:\.exe)?)"
    r"((?:\s+-(?:NoProfile|NonInteractive|NoLogo))*)"
    r"(?:\s+-ExecutionPolicy\s+\S+)?"
    r"\s+-Command\s+",
    re.IGNORECASE,
)


def _extract_powershell_command(cmd: str) -> tuple[str | None, str]:
    """Detect ``powershell -Command <body>`` and return (exe, inner_body).

    When *cmd* starts with a PowerShell invocation followed by ``-Command``,
    extract the executable name and the inner command body (with a single
    layer of surrounding double-quotes removed if present).

    Returns ``(None, cmd)`` unchanged when no PowerShell prefix is found.
    """
    m = _PS_CMD_RE.match(cmd)
    if not m:
        return None, cmd
    ps_exe = m.group(1)
    inner = cmd[m.end() :]
    if len(inner) >= 2 and inner[0] == '"' and inner[-1] == '"':
        inner = inner[1:-1]
    return ps_exe, inner


# pylint: disable=too-many-branches, too-many-statements
def _execute_subprocess_sync(
    cmd: str,
    cwd: str,
    timeout: float,
    env: dict | None = None,
    shell_executable: str | None = None,
    cancel_event: threading.Event | None = None,
) -> tuple[int, str, str]:
    """Execute subprocess synchronously in a thread.

    This function runs in a separate thread to avoid Windows asyncio
    subprocess limitations.

    stdout/stderr are redirected to temporary files instead of pipes.
    On Windows, child processes inherit pipe handles and keep them open
    even after the parent exits, which causes ``communicate()`` to block
    until *all* holders close (e.g. a Chrome process launched via
    ``Start-Process``).  With temp-file redirection, ``proc.wait()``
    only waits for the direct child (``cmd.exe``) to exit, so commands
    that spawn background processes return immediately.

    .. note::

       Callers must pre-process *cmd* through
       :func:`_collapse_embedded_newlines` before passing it here.
       ``execute_shell_command`` already does this.

    Args:
        cmd (`str`):
            The shell command to execute. PowerShell commands may contain
            embedded newlines; other Windows shell commands are normalized
            by the caller as described above.
        cwd (`str`):
            The working directory for the command execution.
        timeout (`float`):
            The maximum time (in seconds) allowed for the command to run.
        env (`dict | None`):
            Environment variables for the subprocess.
        shell_executable (`str | None`):
            Path to the shell executable. When ``None``, defaults to
            ``cmd.exe``.
        cancel_event (`threading.Event | None`):
            Cross-thread signal used to terminate the spawned process tree.

    Returns:
        `tuple[int, str, str]`:
            A tuple containing the return code, standard output, and
            standard error of the executed command. If timeout occurs, the
            return code will be -1 and stderr will contain timeout information.
    """
    stdout_path: str | None = None
    stderr_path: str | None = None
    stdout_file = None
    stderr_file = None

    try:
        if shell_executable and _is_powershell(shell_executable):
            # Strip redundant powershell/pwsh -Command wrapper that the
            # LLM may emit even though the shell is already PowerShell.
            _, cmd = _extract_powershell_command(cmd)
            wrapped = [
                shell_executable,
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                cmd,
            ]
        elif not shell_executable or _is_cmd(shell_executable):
            cmd = _sanitize_win_cmd(cmd)
            shell_name = shell_executable or "cmd"
            wrapped = f'{shell_name} /D /S /C "{cmd}"'
        else:
            # POSIX-like shell on Windows (e.g. Git Bash, MSYS2)
            wrapped = [shell_executable, "-c", cmd]

        stdout_fd, stdout_path = tempfile.mkstemp(prefix="potato_out_")
        stderr_fd, stderr_path = tempfile.mkstemp(prefix="potato_err_")
        stdout_file = os.fdopen(stdout_fd, "wb")
        stderr_file = os.fdopen(stderr_fd, "wb")

        proc = subprocess.Popen(  # pylint: disable=consider-using-with
            wrapped,
            shell=False,
            stdout=stdout_file,
            stderr=stderr_file,
            text=False,
            cwd=cwd,
            env=env,
            creationflags=_windows_shell_creationflags(),
        )

        # Parent copies are no longer needed — the child inherited its own
        # handles via CreateProcess.  Closing here avoids holding the files
        # open longer than necessary.
        stdout_file.close()
        stdout_file = None
        stderr_file.close()
        stderr_file = None

        wait_state = _wait_process_win32(proc, timeout, cancel_event)
        if wait_state != "completed":
            _kill_process_tree_win32(proc.pid)
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                try:
                    proc.kill()
                except OSError:
                    pass

        stdout_str = _read_temp_file(stdout_path)
        stderr_str = _read_temp_file(stderr_path)

        if wait_state != "completed":
            stop_message = (
                "Command execution was cancelled."
                if wait_state == "cancelled"
                else (
                    "Command execution exceeded the timeout of "
                    f"{timeout} seconds."
                )
            )
            if stderr_str:
                stderr_str = f"{stderr_str}\n{stop_message}"
            else:
                stderr_str = stop_message
            return -1, stdout_str, stderr_str

        returncode = proc.returncode if proc.returncode is not None else -1
        return returncode, stdout_str, stderr_str

    except Exception as e:
        return -1, "", str(e)
    finally:
        for f in (stdout_file, stderr_file):
            if f is not None:
                try:
                    f.close()
                except OSError:
                    pass
        for path in (stdout_path, stderr_path):
            if path is not None:
                try:
                    os.unlink(path)
                except OSError:
                    pass


# Extra seconds added to the tool-call deadline to accommodate first-time
# sandbox creation (user provisioning, profile creation, firewall rules, ACLs).
# Subsequent calls hit the cache and need no extension.
_SANDBOX_SETUP_DEADLINE_EXTENSION = 180.0

# Live shell preview: yield on newline or this interval, whichever first.
# Incremental chunks are true deltas (toolkit/envelope accumulate). Each
# SSE in_progress frame then carries the full output so stream.ts can
# replace. qp meta is reserved for the terminal chunk.
_STREAM_INTERVAL_S = 0.2
_STREAM_PREVIEW_CHARS = 16_384


async def _execute_in_sandbox(
    cmd: str,
    sandbox_config: Any,
    timeout: float,
    cwd: str,
    env: dict[str, str],
) -> ExecutionResult:
    """Execute a shell command inside the sandbox and return raw result.

    On first invocation the sandbox setup (user creation, profile, ACLs,
    firewall rules) can take 5-100+ seconds. To prevent the ToolCoordinator's
    deadline from expiring during this one-time setup, we temporarily extend
    the deadline by _SANDBOX_SETUP_DEADLINE_EXTENSION seconds. The extension
    is only applied when the call context has a deadline set.
    """
    from ...sandbox import create_sandbox
    from ...tool_calls import get_call_context

    # Sandbox backends rebuild their environment from os.environ. Carry over
    # the PATH adjusted by the shell entrypoint unless policy set one itself.
    sandbox_env = dict(sandbox_config.env_vars)
    if not any(key.upper() == "PATH" for key in sandbox_env):
        path_key = next(
            (key for key in env if key.upper() == "PATH"),
            "PATH",
        )
        sandbox_env[path_key] = env[path_key]

    effective_config = replace(
        sandbox_config,
        timeout_seconds=int(timeout),
        env_vars=sandbox_env,
    )

    # Temporarily extend the tool-call deadline so that sandbox creation
    # does not consume the user's command timeout budget.
    ctx = get_call_context()
    original_deadline = None
    if ctx is not None and ctx.deadline is not None:
        original_deadline = ctx.deadline
        ctx.deadline += _SANDBOX_SETUP_DEADLINE_EXTENSION

    try:
        async with create_sandbox(effective_config) as sandbox:
            # Restore the original deadline (plus only the command timeout)
            # now that sandbox setup is complete.
            if ctx is not None and original_deadline is not None:
                now = asyncio.get_event_loop().time()
                ctx.deadline = now + timeout
            result = await sandbox.execute(cmd, cwd=cwd)
    except BaseException:
        # On failure, restore original deadline to avoid permanent extension
        if ctx is not None and original_deadline is not None:
            ctx.deadline = original_deadline
        raise

    return result


def _format_shell_output(
    returncode: int,
    stdout_str: str,
    stderr_str: str,
) -> str:
    if returncode == 0:
        if stdout_str:
            response_text = stdout_str
        else:
            response_text = "Command executed successfully (no output)."
        if stderr_str:
            response_text += f"\n[stderr]\n{stderr_str}"
        return response_text
    parts = [f"Command failed with exit code {returncode}."]
    if stdout_str:
        parts.append(f"\n[stdout]\n{stdout_str}")
    if stderr_str:
        parts.append(f"\n[stderr]\n{stderr_str}")
    return "".join(parts)


def _live_preview(stdout_buf: bytearray, stderr_buf: bytearray) -> str:
    stdout_str = smart_decode(bytes(stdout_buf))
    stderr_str = smart_decode(bytes(stderr_buf))
    if stdout_str and stderr_str:
        return f"{stdout_str}\n[stderr]\n{stderr_str}"
    return stdout_str or (
        f"[stderr]\n{stderr_str}" if stderr_str else ""
    )


def _running_chunk(text: str) -> ToolChunk:
    return ToolChunk(
        is_last=False,
        state=ToolResultState.RUNNING,
        content=[TextBlock(type="text", text=text)],
    )


def _final_shell_chunk(
    text: str,
    *,
    sandboxed: bool,
    ok: bool,
    exit_code: int | None,
) -> ToolChunk:
    return ToolChunk(
        is_last=True,
        state=ToolResultState.SUCCESS,
        content=[TextBlock(type="text", text=text)],
        metadata=_shell_metadata(
            sandboxed=sandboxed,
            ok=ok,
            exit_code=exit_code,
        ),
    )


async def _read_pipe(stream: Any, buf: bytearray) -> None:
    while True:
        chunk = await stream.read(4096)
        if not chunk:
            return
        buf.extend(chunk)


async def _kill_posix_pg(proc: Any) -> None:
    try:
        pgid = os.getpgid(proc.pid)
        os.killpg(pgid, signal.SIGTERM)
        try:
            await asyncio.wait_for(proc.wait(), timeout=2)
        except asyncio.TimeoutError:
            os.killpg(pgid, signal.SIGKILL)
            await asyncio.wait_for(proc.wait(), timeout=2)
    except (ProcessLookupError, OSError):
        try:
            proc.kill()
            await proc.wait()
        except (ProcessLookupError, OSError):
            pass


async def _stream_posix_shell(
    cmd: str,
    working_dir: str,
    timeout: float,
    env: dict,
    shell_executable: str | None,
) -> AsyncGenerator[ToolChunk, None]:
    """Yield stdout/stderr deltas while the process runs, then a final chunk.

    Incremental chunks never carry qp meta. Oversized bursts are held
    back so only a tail window rides the live stream; the terminal
    chunk still delivers everything that was not sent.
    """
    from ...tool_calls import get_call_context

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
    stdout_buf = bytearray()
    stderr_buf = bytearray()
    readers = [
        asyncio.create_task(_read_pipe(proc.stdout, stdout_buf)),
        asyncio.create_task(_read_pipe(proc.stderr, stderr_buf)),
    ]
    wait_task = asyncio.create_task(proc.wait())
    ctx = get_call_context()
    loop = asyncio.get_running_loop()
    deadline = loop.time() + timeout
    sent = ""
    last_yield = 0.0
    timed_out = False
    cancelled = False

    try:
        while not wait_task.done():
            if ctx is not None and ctx.is_cancelled:
                cancelled = True
                break
            remaining = deadline - loop.time()
            if remaining <= 0:
                timed_out = True
                break
            await asyncio.wait({wait_task}, timeout=min(_STREAM_INTERVAL_S, remaining))
            preview = _live_preview(stdout_buf, stderr_buf)
            new = preview[len(sent) :]
            now = time.monotonic()
            due = (now - last_yield) >= _STREAM_INTERVAL_S
            if new and ("\n" in new or due) and len(new) <= _STREAM_PREVIEW_CHARS:
                yield _running_chunk(new)
                sent = preview
                last_yield = now
    finally:
        if timed_out or cancelled:
            await _kill_posix_pg(proc)
            if not wait_task.done():
                await asyncio.gather(wait_task, return_exceptions=True)
        await asyncio.gather(*readers, return_exceptions=True)
        if not wait_task.done():
            await asyncio.gather(wait_task, return_exceptions=True)

    stdout_str = smart_decode(bytes(stdout_buf))
    stderr_str = smart_decode(bytes(stderr_buf))
    if timed_out:
        stderr_suffix = (
            f"⚠️ TimeoutError: The command execution exceeded "
            f"the timeout of {timeout} seconds. "
            f"Please consider increasing the timeout value if this command "
            f"requires more time to complete."
        )
        returncode = -1
        stderr_str = f"{stderr_str}\n{stderr_suffix}" if stderr_str else stderr_suffix
    elif cancelled:
        returncode = -1
        stop = "Command execution was cancelled."
        stderr_str = f"{stderr_str}\n{stop}" if stderr_str else stop
    else:
        returncode = proc.returncode if proc.returncode is not None else -1

    response_text = _format_shell_output(returncode, stdout_str, stderr_str)
    if response_text.startswith(sent):
        remaining_text = response_text[len(sent) :]
    else:
        remaining_text = response_text
    yield _final_shell_chunk(
        remaining_text,
        sandboxed=False,
        ok=returncode == 0,
        exit_code=returncode,
    )


_DANGER_NAMES = {
    "python",
    "pythonw",
    "cmd",
    "powershell",
    "pwsh",
    "conhost",
}

# Prefix: kill/taskkill at command start or after &&, ;, |
_KILL_PREFIX = r"(?:^|[;&|]\s*)\s*"

# Matches PID-based kills: taskkill /PID 123, kill -9 123, kill 123.
_KILL_PID_RE = re.compile(
    rf"{_KILL_PREFIX}(?:taskkill|kill|stop-process)\b"
    rf".*(?:/PID|-p|-pid|\b)\s*(\d+)",
    re.IGNORECASE,
)

# Matches dangerous process names as /IM targets or bare kill targets.
_DANGER_NAME_RE = re.compile(
    rf"{_KILL_PREFIX}(?:taskkill|kill|stop-process)\b"
    rf".*?\b({'|'.join(_DANGER_NAMES)})(?:\.exe)?\b",
    re.IGNORECASE,
)

# Shell variables that reference the current/parent PID.
_SHELL_PID_VARS = {"$$", "$ppid", "$pid"}


def _is_dangerous_self_kill(cmd: str) -> bool:
    """Return True if *cmd* would kill the current process or its parent.

    Uses token-based regex matching to avoid false positives from
    substring matching (e.g. ``echo "do not kill python"`` is safe).

    Blocks three patterns:
    1. ``taskkill /IM <dangerous_name>`` — kills by image name.
    2. ``kill <pid>`` / ``taskkill /PID <pid>`` targeting our PID or
       parent.
    3. Shell variable self-kill: ``kill -9 $$``, ``kill $PPID``.
    """
    lower = cmd.lower()

    if _DANGER_NAME_RE.search(lower):
        return True

    if "kill" in lower or "stop-process" in lower:
        if any(var in lower for var in _SHELL_PID_VARS):
            return True

    m = _KILL_PID_RE.search(lower)
    if m:
        try:
            target_pid = int(m.group(1))
            protected_pids = {os.getpid()}
            if hasattr(os, "getppid"):
                protected_pids.add(os.getppid())
            if target_pid in protected_pids:
                return True
        except ValueError:
            pass

    return False


# pylint: disable=too-many-branches, too-many-statements
@tool_descriptor(
    requires_sandbox=("shell_exec",),
    async_execution=True,
    tool_type="shell",
    target_param="command",
    policy_name="Bash",
    ui_description="Execute shell commands",
    ui_icon="💻",
)
async def execute_shell_command(
    command: str,
    timeout: float = 60.0,
    cwd: Optional[Path] = None,
    sandbox_config: Optional[Any] = None,
    run_in_background: bool | str | int = False,
) -> ToolChunk | AsyncGenerator[ToolChunk, None]:
    """Execute a shell command and return its output.

    Each call runs in a fresh subprocess — `cd`, `export`, `source`,
    etc. do NOT persist. Pass `cwd=` or chain in one call
    (`cd /repo && pytest`).

    Set `run_in_background=true` for long work: the call returns a job
    id immediately (no timeout). Collect with `job_output`, stop with
    `job_kill`. Do not busy-poll; you are notified when it finishes.

    IMPORTANT: Check the 'Default Shell' field to
    determine which shell is active, and generate commands using the
    appropriate syntax (e.g. bash vs PowerShell vs cmd.exe).

    Args:
        command (`str`):
            The shell command to execute.
        timeout (`float`, defaults to `60.0`):
            The maximum time (in seconds) allowed for the command to run.
            Default is 60.0 seconds. Ignored when `run_in_background`.
        cwd (`Optional[Path]`, defaults to `None`):
            The working directory for the command execution.
            If None, defaults to the agent workspace.
        sandbox_config (`Optional[Any]`, defaults to `None`):
            Sandbox execution configuration compiled from governance policy.
            When provided, the command executes within a sandboxed environment
            with the specified mount permissions and network restrictions.
        run_in_background:
            If true, detach the process and return a job id. Tool-call
            cancel does not kill it.

    Returns:
        `ToolChunk | AsyncGenerator[ToolChunk, None]`:
            A single terminal chunk for blocked / sandboxed / Windows
            runs, or a stream of incremental chunks (no qp) followed by
            one terminal chunk (with qp) on POSIX. Timeout sets exit -1.
    """

    shell_executable = (
        get_current_shell_command_executable()
        or os.environ.get("SHELL")
        or None
    )
    cmd = _collapse_embedded_newlines(
        (command or "").strip(),
        shell_executable=shell_executable,
    )

    if _is_dangerous_self_kill(cmd):
        return ToolChunk(
            is_last=True,
            state=ToolResultState.ERROR,
            content=[
                TextBlock(
                    type="text",
                    text=(
                        "Blocked: this command would terminate the "
                        "Potato process or its parent. "
                        "Refusing to execute."
                    ),
                ),
            ],
            metadata=_shell_metadata(
                sandboxed=sandbox_config is not None,
                ok=False,
            ),
        )

    if isinstance(timeout, str):
        try:
            timeout = float(timeout)
        except (ValueError, TypeError):
            timeout = 60.0

    # Apply agent-configured default when the caller used the hardcoded
    # default (60.0).  An explicit LLM-provided value != 60.0 is kept.
    if timeout == 60.0:
        configured = get_current_shell_command_timeout()
        if configured is not None:
            timeout = configured

    # Use current workspace_dir from context, fallback to WORKING_DIR
    if cwd is not None:
        working_dir = cwd
    else:
        working_dir = get_current_workspace_dir() or WORKING_DIR

    # Ensure the venv Python is on PATH for subprocesses
    env = os.environ.copy()
    python_bin_dir = str(Path(sys.executable).parent)
    existing_path = env.get("PATH", "")
    if existing_path:
        env["PATH"] = python_bin_dir + os.pathsep + existing_path
    else:
        env["PATH"] = python_bin_dir

    from ...runtime.jobs import coerce_bool

    try:
        background = coerce_bool(
            run_in_background,
            default=False,
            field_name="run_in_background",
        )
    except ValueError as exc:
        return ToolChunk(
            is_last=True,
            state=ToolResultState.ERROR,
            content=[TextBlock(type="text", text=f"Error: {exc}")],
            metadata=_shell_metadata(sandboxed=False, ok=False),
        )

    if background:
        return await _start_background_job(
            cmd,
            str(working_dir),
            env,
            shell_executable,
            sandbox_config,
        )

    if sandbox_config is not None:
        # Create a copy with resolved shell and timeout to avoid mutating
        # the shared config object (it may be reused across tool calls).
        sandbox_config = replace(
            sandbox_config,
            shell_executable=shell_executable,
            timeout_seconds=int(timeout),
        )
        result = await _execute_in_sandbox(
            cmd,
            sandbox_config,
            timeout,
            str(working_dir),
            env,
        )
        # Sandbox violation: command tried to access something not permitted
        if result.sandbox_violation:
            return ToolChunk(
                is_last=True,
                state=ToolResultState.DENIED,
                content=[
                    TextBlock(
                        type="text",
                        text=f"Sandbox violation: {result.sandbox_violation}\n"
                        f"Command was blocked by sandbox security policy.",
                    ),
                ],
                metadata={
                    "sandbox_violation": result.sandbox_violation,
                    **_shell_metadata(
                        sandboxed=True,
                        ok=False,
                        violation=result.sandbox_violation,
                    ),
                },
            )
        return _final_shell_chunk(
            _format_shell_output(
                result.exit_code,
                result.stdout,
                result.stderr,
            ),
            sandboxed=True,
            ok=result.exit_code == 0,
            exit_code=result.exit_code,
        )

    import logging as _logging

    _logging.getLogger(__name__).debug(
        "[sandbox] SKIP: sandbox_config is None, executing directly",
    )

    try:
        if sys.platform == "win32":
            # Windows: use thread pool to avoid asyncio subprocess limitations
            from ...tool_calls import cancellable_wait

            cancel_event = threading.Event()
            worker_task = asyncio.create_task(
                asyncio.to_thread(
                    _execute_subprocess_sync,
                    cmd,
                    str(working_dir),
                    timeout,
                    env,
                    shell_executable,
                    cancel_event,
                ),
            )
            try:
                # Shield the worker so cancellable_wait only cancels this
                # awaiter. Python cannot stop a running thread; the explicit
                # event lets that thread kill and reap its process tree.
                returncode, stdout_str, stderr_str = await cancellable_wait(
                    asyncio.shield(worker_task),
                )
            except asyncio.CancelledError:
                cancel_event.set()
                returncode, stdout_str, stderr_str = await asyncio.shield(
                    worker_task,
                )
        else:
            return _stream_posix_shell(
                cmd,
                str(working_dir),
                timeout,
                env,
                shell_executable,
            )

        return _final_shell_chunk(
            _format_shell_output(returncode, stdout_str, stderr_str),
            sandboxed=False,
            ok=returncode == 0,
            exit_code=returncode,
        )

    except Exception as e:
        return ToolChunk(
            is_last=True,
            state=ToolResultState.SUCCESS,
            content=[
                TextBlock(
                    type="text",
                    text=f"Error: Shell command execution failed due to \n{e}",
                ),
            ],
            metadata=_shell_metadata(sandboxed=False, ok=False),
        )


async def _start_background_job(
    cmd: str,
    working_dir: str,
    env: dict,
    shell_executable: str | None,
    sandbox_config: Any,
) -> ToolChunk:
    """Detach the process and return a structured job handle."""
    if sandbox_config is not None:
        return ToolChunk(
            is_last=True,
            state=ToolResultState.ERROR,
            content=[
                TextBlock(
                    type="text",
                    text=(
                        "Error: run_in_background is not available inside a "
                        "sandbox. Run the command in the foreground, or "
                        "without a sandbox."
                    ),
                ),
            ],
            metadata=_shell_metadata(sandboxed=True, ok=False),
        )
    from ...runtime.jobs import (
        JobAccessError,
        JobLimitError,
        JobStart,
        get_job_registry,
    )
    from ...tool_calls import get_call_context
    from .shell_background import start_background_shell

    ctx = get_call_context()
    owner_session = ctx.root_session_id or ctx.session_id if ctx else ""
    owner_agent = ctx.agent_id if ctx else ""
    try:
        from ...app.agent_context import (
            get_current_channel,
            get_current_user_id,
        )

        owner_user = get_current_user_id() or ""
        owner_channel = get_current_channel() or "console"
    except Exception:  # pylint: disable=broad-except
        owner_user = ""
        owner_channel = "console"

    async def _run():
        proc = await start_background_shell(
            cmd,
            working_dir,
            env,
            shell_executable,
        )
        return proc.hooks()

    try:
        job_id = await get_job_registry().start(
            JobStart(
                kind="bash",
                label=cmd,
                run=_run,
                owner_session=owner_session,
                owner_agent=owner_agent,
                owner_user=owner_user,
                owner_channel=owner_channel,
            ),
        )
    except JobLimitError as exc:
        return ToolChunk(
            is_last=True,
            state=ToolResultState.ERROR,
            content=[TextBlock(type="text", text=f"Error: {exc}")],
            metadata=_shell_metadata(sandboxed=False, ok=False),
        )
    except JobAccessError as exc:
        return ToolChunk(
            is_last=True,
            state=ToolResultState.ERROR,
            content=[TextBlock(type="text", text=f"Error: {exc}")],
            metadata=_shell_metadata(sandboxed=False, ok=False),
        )
    return ToolChunk(
        is_last=True,
        state=ToolResultState.SUCCESS,
        content=[
            TextBlock(
                type="text",
                text=(
                    f"started background job {job_id}\n"
                    "Collect output with job_output; stop with job_kill. "
                    "You will be notified when it finishes."
                ),
            ),
        ],
        metadata={
            **_shell_metadata(sandboxed=False, ok=True, exit_code=0),
            "background": True,
            "job_id": job_id,
        },
    )


def smart_decode(data: bytes) -> str:
    try:
        decoded_str = data.decode("utf-8")
    except UnicodeDecodeError:
        encoding = locale.getpreferredencoding(False) or "utf-8"
        decoded_str = data.decode(encoding, errors="replace")

    return decoded_str.strip("\n")
