# -*- coding: utf-8 -*-
"""Tests for potato.agents.tools.shell.

Covers:
- _collapse_newlines_outside_quotes
- _collapse_embedded_newlines
- _sanitize_win_cmd
- _read_temp_file
- _shell_basename
- _is_powershell / _is_cmd
- _extract_powershell_command
- smart_decode
- execute_shell_command (mocked subprocess)
"""
# pylint: disable=protected-access,unused-argument

import asyncio
import os
import shlex
import subprocess
import sys
import threading
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from potato.agents.tools.shell import (
    _collapse_embedded_newlines,
    _collapse_newlines_outside_quotes,
    _execute_in_sandbox,
    _extract_powershell_command,
    _is_cmd,
    _is_dangerous_self_kill,
    _is_powershell,
    _read_temp_file,
    _sanitize_win_cmd,
    _shell_basename,
    _windows_taskkill_args,
    execute_shell_command,
    smart_decode,
)
from potato.sandbox import (
    ExecutionResult,
    MountSpec,
    SandboxConfig,
    SandboxMode,
)


def _async_stream(data: bytes):
    state = {"done": False}

    async def read(_n=-1):
        if state["done"]:
            return b""
        state["done"] = True
        return data

    stream = MagicMock()
    stream.read = read
    return stream


def _mock_proc(stdout: bytes, stderr: bytes, returncode: int = 0):
    proc = MagicMock()
    proc.stdout = _async_stream(stdout)
    proc.stderr = _async_stream(stderr)
    proc.returncode = returncode
    proc.pid = 12345

    async def wait():
        return returncode

    proc.wait = wait
    return proc


async def _run_shell(*args, **kwargs):
    result = await execute_shell_command(*args, **kwargs)
    if not hasattr(result, "__aiter__"):
        return result
    chunks = []
    async for chunk in result:
        chunks.append(chunk)
    last = chunks[-1]
    text = "".join(
        block.text
        for chunk in chunks
        for block in chunk.content
        if getattr(block, "type", None) == "text"
    )
    if last.content:
        last.content[0].text = text
    return last


# ---------------------------------------------------------------------------
# _shell_basename
# ---------------------------------------------------------------------------


class TestShellBasename:
    """Tests for _shell_basename."""

    def test_unix_path(self):
        assert _shell_basename("/usr/bin/bash") == "bash"

    def test_windows_path(self):
        assert _shell_basename("C:\\Windows\\cmd.exe") == "cmd.exe"

    def test_powershell_path(self):
        assert (
            _shell_basename(
                "/usr/local/bin/pwsh",
            )
            == "pwsh"
        )

    def test_lowercase(self):
        assert _shell_basename("/bin/BASH") == "bash"


# ---------------------------------------------------------------------------
# _is_powershell / _is_cmd
# ---------------------------------------------------------------------------


class TestIsPowershell:
    """Tests for _is_powershell."""

    @pytest.mark.parametrize(
        "exe",
        ["powershell", "powershell.exe", "pwsh", "pwsh.exe"],
    )
    def test_powershell_variants(self, exe):
        assert _is_powershell(exe) is True

    def test_non_powershell(self):
        assert _is_powershell("/bin/bash") is False

    def test_cmd_is_not_powershell(self):
        assert _is_powershell("cmd") is False


class TestIsCmd:
    """Tests for _is_cmd."""

    @pytest.mark.parametrize("exe", ["cmd", "cmd.exe"])
    def test_cmd_variants(self, exe):
        assert _is_cmd(exe) is True

    def test_non_cmd(self):
        assert _is_cmd("/bin/bash") is False


def test_windows_taskkill_args_targets_entire_process_tree():
    assert _windows_taskkill_args(4321) == [
        "taskkill",
        "/F",
        "/T",
        "/PID",
        "4321",
    ]


def test_windows_sync_cancel_kills_process_tree(monkeypatch, tmp_path):
    from potato.agents.tools import shell

    process = MagicMock(pid=4321, returncode=None)
    cancel_event = threading.Event()
    cancel_event.set()
    kill_tree = MagicMock()
    monkeypatch.setattr(
        shell.subprocess,
        "Popen",
        MagicMock(return_value=process),
    )
    monkeypatch.setattr(shell, "_kill_process_tree_win32", kill_tree)

    returncode, stdout, stderr = shell._execute_subprocess_sync(
        "python -c pass",
        str(tmp_path),
        30,
        cancel_event=cancel_event,
    )

    kill_tree.assert_called_once_with(4321)
    process.wait.assert_called_once_with(timeout=5)
    assert returncode == -1
    assert stdout == ""
    assert "cancelled" in stderr.lower()


# ---------------------------------------------------------------------------
# _collapse_newlines_outside_quotes
# ---------------------------------------------------------------------------


class TestCollapseNewlinesOutsideQuotes:
    """Tests for _collapse_newlines_outside_quotes."""

    def test_no_newlines(self):
        assert _collapse_newlines_outside_quotes("echo hello") == "echo hello"

    def test_unquoted_newline_to_space(self):
        assert _collapse_newlines_outside_quotes("echo\nhello") == "echo hello"

    def test_crlf_to_space(self):
        assert (
            _collapse_newlines_outside_quotes("echo\r\nhello") == "echo hello"
        )

    def test_single_quoted_newline_preserved(self):
        result = _collapse_newlines_outside_quotes("echo 'hello\nworld'")
        assert "\n" in result

    def test_double_quoted_newline_preserved(self):
        result = _collapse_newlines_outside_quotes('echo "hello\nworld"')
        assert "\n" in result

    def test_backslash_newline_continuation(self):
        result = _collapse_newlines_outside_quotes("echo \\\nhello")
        assert result == "echo hello"

    def test_backslash_before_normal_char_kept(self):
        result = _collapse_newlines_outside_quotes(r"echo \nhello")
        assert result == r"echo \nhello"

    def test_mixed_quoted_and_unquoted(self):
        cmd = 'echo "line1\nline2" && \necho second'
        result = _collapse_newlines_outside_quotes(cmd)
        # First \n inside double quotes preserved
        assert "line1\nline2" in result
        # Second \n outside quotes collapsed to space
        assert "echo second" in result


# ---------------------------------------------------------------------------
# _collapse_embedded_newlines
# ---------------------------------------------------------------------------


class TestCollapseEmbeddedNewlines:
    """Tests for _collapse_embedded_newlines."""

    def test_no_newlines_unchanged(self):
        command = "echo hello"
        assert (
            _collapse_embedded_newlines(command, "powershell.exe") == command
        )

    @patch("potato.agents.tools.shell.sys")
    def test_windows_cmd_collapses_all(self, mock_sys):
        mock_sys.platform = "win32"
        result = _collapse_embedded_newlines(
            'echo "hello\r\nworld"',
            r"C:\Windows\System32\cmd.exe",
        )
        assert result == 'echo "hello world"'

    @patch("potato.agents.tools.shell.sys")
    def test_windows_default_shell_collapses_all(self, mock_sys):
        mock_sys.platform = "win32"
        result = _collapse_embedded_newlines('echo "hello\nworld"')
        assert result == 'echo "hello world"'

    @pytest.mark.parametrize("shell", ["powershell.exe", "pwsh.exe"])
    @pytest.mark.parametrize("newline", ["\n", "\r\n"])
    @patch("potato.agents.tools.shell.sys")
    def test_windows_powershell_preserves_here_string(
        self,
        mock_sys,
        newline,
        shell,
    ):
        mock_sys.platform = "win32"
        command = (
            f'$content = @"{newline}hello{newline}'
            f'world{newline}"@{newline}$content'
        )
        assert _collapse_embedded_newlines(command, shell) == command

    @patch("potato.agents.tools.shell.sys")
    def test_unix_preserves_quoted_newlines(self, mock_sys):
        mock_sys.platform = "linux"
        command = 'echo "hello\nworld"'
        assert _collapse_embedded_newlines(command, "/bin/bash") == command


# ---------------------------------------------------------------------------
# _sanitize_win_cmd
# ---------------------------------------------------------------------------


class TestSanitizeWinCmd:
    """Tests for _sanitize_win_cmd."""

    def test_no_escaped_quotes(self):
        assert _sanitize_win_cmd("echo hello") == "echo hello"

    def test_all_escaped_quotes_stripped(self):
        # Every " is preceded by \ — double-escape artefact
        result = _sanitize_win_cmd('echo \\"hello\\"')
        assert result == 'echo "hello"'

    def test_mixed_quotes_not_stripped(self):
        # Mix of escaped and unescaped — don't strip
        cmd = 'echo \\"hello" world'
        assert _sanitize_win_cmd(cmd) == cmd


# ---------------------------------------------------------------------------
# _read_temp_file
# ---------------------------------------------------------------------------


class TestReadTempFile:
    """Tests for _read_temp_file."""

    def test_read_existing_file(self, tmp_path):
        f = tmp_path / "out.txt"
        f.write_text("hello world", encoding="utf-8")
        result = _read_temp_file(str(f))
        assert result == "hello world"

    def test_read_nonexistent_file(self):
        result = _read_temp_file("/nonexistent/file.txt")
        assert result == ""

    def test_read_utf8_bytes(self, tmp_path):
        f = tmp_path / "out.txt"
        f.write_bytes("你好".encode("utf-8"))
        result = _read_temp_file(str(f))
        assert "你好" in result


# ---------------------------------------------------------------------------
# _extract_powershell_command
# ---------------------------------------------------------------------------


class TestExtractPowershellCommand:
    """Tests for _extract_powershell_command."""

    def test_powershell_command(self):
        ps_exe, inner = _extract_powershell_command(
            'powershell -Command "Get-Process"',
        )
        assert ps_exe == "powershell"
        assert inner == "Get-Process"

    def test_pwsh_command(self):
        ps_exe, _ = _extract_powershell_command(
            'pwsh -Command "Get-Process"',
        )
        assert ps_exe == "pwsh"

    def test_powershell_with_flags(self):
        ps_exe, inner = _extract_powershell_command(
            "powershell -NoProfile -NonInteractive -Command Get-Process",
        )
        assert ps_exe == "powershell"
        assert inner == "Get-Process"

    def test_non_powershell(self):
        ps_exe, inner = _extract_powershell_command("echo hello")
        assert ps_exe is None
        assert inner == "echo hello"

    def test_pwsh_exe(self):
        ps_exe, _ = _extract_powershell_command(
            "pwsh.exe -Command test",
        )
        assert ps_exe == "pwsh.exe"

    def test_execution_policy_flag(self):
        ps_exe, inner = _extract_powershell_command(
            "powershell -ExecutionPolicy Bypass -Command echo hi",
        )
        assert ps_exe == "powershell"
        assert inner == "echo hi"


# ---------------------------------------------------------------------------
# smart_decode
# ---------------------------------------------------------------------------


class TestSmartDecode:
    """Tests for smart_decode."""

    def test_utf8_bytes(self):
        result = smart_decode("hello".encode("utf-8"))
        assert result == "hello"

    def test_strips_trailing_newlines(self):
        result = smart_decode("hello\n\n".encode("utf-8"))
        assert result == "hello"

    def test_non_utf8_fallback(self):
        # Bytes that are invalid UTF-8 should fall back to
        # locale encoding with error replacement
        data = b"\xff\xfe"  # BOM for UTF-16-LE, invalid UTF-8
        result = smart_decode(data)
        assert isinstance(result, str)


# ---------------------------------------------------------------------------
# _is_dangerous_self_kill
# ---------------------------------------------------------------------------


class TestIsDangerousSelfKill:
    """Tests for _is_dangerous_self_kill."""

    def test_taskkill_by_image_name_python(self):
        assert _is_dangerous_self_kill("taskkill /F /IM python.exe")

    def test_taskkill_by_image_name_pythonw(self):
        assert _is_dangerous_self_kill("taskkill /F /IM pythonw.exe")

    def test_taskkill_by_image_name_cmd(self):
        assert _is_dangerous_self_kill("taskkill /F /IM cmd.exe")

    def test_taskkill_by_image_name_powershell(self):
        assert _is_dangerous_self_kill("taskkill /F /IM powershell.exe")

    def test_taskkill_by_image_name_pwsh(self):
        assert _is_dangerous_self_kill("taskkill /F /IM pwsh.exe")

    def test_taskkill_by_image_name_conhost(self):
        assert _is_dangerous_self_kill("taskkill /F /IM conhost.exe")

    def test_taskkill_by_image_name_without_exe(self):
        assert _is_dangerous_self_kill("taskkill /F /IM python")

    def test_taskkill_by_pid_self(self):
        assert _is_dangerous_self_kill(f"taskkill /F /PID {os.getpid()}")

    def test_taskkill_by_pid_parent(self):
        if hasattr(os, "getppid"):
            assert _is_dangerous_self_kill(
                f"taskkill /F /PID {os.getppid()}",
            )

    def test_taskkill_by_pid_other_is_safe(self):
        assert not _is_dangerous_self_kill("taskkill /F /PID 99999")

    def test_kill_unix_pid_self(self):
        assert _is_dangerous_self_kill(f"kill -9 {os.getpid()}")

    def test_kill_unix_pid_other_is_safe(self):
        assert not _is_dangerous_self_kill("kill -9 99999")

    def test_kill_shell_var_dollar_dollar(self):
        assert _is_dangerous_self_kill("kill -9 $$")

    def test_kill_shell_var_ppid(self):
        assert _is_dangerous_self_kill("kill $PPID")

    def test_kill_shell_var_pid(self):
        assert _is_dangerous_self_kill("kill $PID")

    def test_false_positive_command_contains_cmd(self):
        """'command' contains 'cmd' but should not be blocked."""
        assert not _is_dangerous_self_kill("echo 'run a command'")

    def test_false_positive_echo_kill_python(self):
        """echo with 'kill python' in text should not be blocked."""
        assert not _is_dangerous_self_kill(
            'echo "do not kill python"',
        )

    def test_false_positive_cat_file(self):
        """Reading a file named kill_list_python.txt should not be blocked."""
        assert not _is_dangerous_self_kill("cat kill_list_python.txt")

    def test_safe_command(self):
        assert not _is_dangerous_self_kill("echo hello")

    def test_stop_process_by_name(self):
        assert _is_dangerous_self_kill("Stop-Process -Name python")


# ---------------------------------------------------------------------------
# execute_shell_command (mocked)
# ---------------------------------------------------------------------------


class TestExecuteShellCommand:
    """Tests for execute_shell_command with mocked subprocess."""

    @pytest.mark.asyncio
    async def test_windows_cancel_signals_sync_worker(self, monkeypatch):
        from potato.agents.tools import shell
        from potato.tool_calls import (
            ToolCallContext,
            reset_call_context,
            set_call_context,
        )

        worker_started = threading.Event()
        worker_stopped = threading.Event()

        def fake_execute_sync(
            cmd,
            cwd,
            timeout,
            env,
            shell_executable,
            cancel_event,
        ):
            worker_started.set()
            assert cancel_event.wait(timeout=2)
            worker_stopped.set()
            return -1, "", "Command execution was cancelled."

        monkeypatch.setattr(shell.sys, "platform", "win32")
        monkeypatch.setattr(
            shell,
            "_execute_subprocess_sync",
            fake_execute_sync,
        )

        loop = asyncio.get_running_loop()
        ctx = ToolCallContext(
            tool_call_id="call-win-cancel",
            tool_name="execute_shell_command",
            session_id="session-win-cancel",
            agent_id="agent-win-cancel",
            root_session_id="session-win-cancel",
            started_at=loop.time(),
            deadline=None,
            cancel_event=asyncio.Event(),
        )
        token = set_call_context(ctx)
        try:
            task = asyncio.create_task(shell.execute_shell_command("sleep"))
            while not worker_started.is_set():
                await asyncio.sleep(0.01)
            ctx.cancel_event.set()
            result = await asyncio.wait_for(task, timeout=2)
        finally:
            reset_call_context(token)

        assert worker_stopped.is_set()
        assert "cancelled" in result.content[0].text.lower()

    @pytest.mark.asyncio
    @patch("potato.agents.tools.shell.get_current_shell_command_timeout")
    @patch("potato.agents.tools.shell.get_current_workspace_dir")
    @patch("potato.agents.tools.shell.get_current_shell_command_executable")
    async def test_simple_command_success(
        self,
        mock_shell_exe,
        mock_workspace,
        mock_timeout,
    ):
        mock_shell_exe.return_value = None
        mock_workspace.return_value = None
        mock_timeout.return_value = None

        async def fake_wait_for(coro, timeout=None):
            return await coro

        mock_proc = _mock_proc(b"hello\n", b"", 0)

        with (
            patch(
                "potato.agents.tools.shell.asyncio.create_subprocess_shell",
                AsyncMock(return_value=mock_proc),
            ),
            patch(
                "potato.agents.tools.shell.asyncio.wait_for",
                side_effect=fake_wait_for,
            ),
        ):
            result = await _run_shell("echo hello")
            assert result.content is not None
            text = result.content[0].text
            assert "hello" in text
            assert result.metadata["qp"] == {
                "v": 1,
                "kind": "shell",
                "ok": True,
                "data": {"sandboxed": False, "exit_code": 0},
            }

    @pytest.mark.asyncio
    @patch("potato.agents.tools.shell.get_current_shell_command_timeout")
    @patch("potato.agents.tools.shell.get_current_workspace_dir")
    @patch("potato.agents.tools.shell.get_current_shell_command_executable")
    async def test_command_failure(
        self,
        mock_shell_exe,
        mock_workspace,
        mock_timeout,
    ):
        mock_shell_exe.return_value = None
        mock_workspace.return_value = None
        mock_timeout.return_value = None

        async def fake_wait_for(coro, timeout=None):
            return await coro

        mock_proc = _mock_proc(b"", b"error msg\n", 1)

        with (
            patch(
                "potato.agents.tools.shell.asyncio.create_subprocess_shell",
                AsyncMock(return_value=mock_proc),
            ),
            patch(
                "potato.agents.tools.shell.asyncio.wait_for",
                side_effect=fake_wait_for,
            ),
        ):
            result = await _run_shell("false")
            text = result.content[0].text
            assert "failed" in text.lower() or "error" in text.lower()
            assert result.metadata["qp"]["ok"] is False
            assert result.metadata["qp"]["data"] == {
                "sandboxed": False,
                "exit_code": 1,
            }

    @pytest.mark.asyncio
    async def test_blocked_command_omits_unavailable_exit_code(self):
        from potato.agents.tools.shell import execute_shell_command

        result = await execute_shell_command(f"kill {os.getpid()}")

        assert result.metadata["qp"] == {
            "v": 1,
            "kind": "shell",
            "ok": False,
            "data": {"sandboxed": False},
        }

    @pytest.mark.asyncio
    @patch("potato.agents.tools.shell.get_current_shell_command_timeout")
    @patch("potato.agents.tools.shell.get_current_workspace_dir")
    @patch("potato.agents.tools.shell.get_current_shell_command_executable")
    async def test_empty_command(
        self,
        mock_shell_exe,
        mock_workspace,
        mock_timeout,
    ):
        mock_shell_exe.return_value = None
        mock_workspace.return_value = None
        mock_timeout.return_value = None

        async def fake_wait_for(coro, timeout=None):
            return await coro

        mock_proc = _mock_proc(b"", b"", 0)

        with (
            patch(
                "potato.agents.tools.shell.asyncio.create_subprocess_shell",
                AsyncMock(return_value=mock_proc),
            ),
            patch(
                "potato.agents.tools.shell.asyncio.wait_for",
                side_effect=fake_wait_for,
            ),
        ):
            result = await _run_shell("")
            text = result.content[0].text
            assert "successfully" in text.lower()

    @pytest.mark.asyncio
    @patch("potato.agents.tools.shell.get_current_shell_command_timeout")
    @patch("potato.agents.tools.shell.get_current_workspace_dir")
    @patch("potato.agents.tools.shell.get_current_shell_command_executable")
    async def test_timeout_string_converted(
        self,
        mock_shell_exe,
        mock_workspace,
        mock_timeout,
    ):
        mock_shell_exe.return_value = None
        mock_workspace.return_value = None
        mock_timeout.return_value = None

        async def fake_wait_for(coro, timeout=None):
            return await coro

        mock_proc = _mock_proc(b"ok", b"", 0)

        with (
            patch(
                "potato.agents.tools.shell.asyncio.create_subprocess_shell",
                AsyncMock(return_value=mock_proc),
            ),
            patch(
                "potato.agents.tools.shell.asyncio.wait_for",
                side_effect=fake_wait_for,
            ),
        ):
            # timeout as string "30" should be converted to float
            result = await _run_shell("echo ok", timeout="30")
            assert result.content is not None

    @pytest.mark.asyncio
    @patch("potato.agents.tools.shell.get_current_shell_command_timeout")
    @patch("potato.agents.tools.shell.get_current_workspace_dir")
    @patch("potato.agents.tools.shell.get_current_shell_command_executable")
    async def test_invalid_timeout_defaults(
        self,
        mock_shell_exe,
        mock_workspace,
        mock_timeout,
    ):
        mock_shell_exe.return_value = None
        mock_workspace.return_value = None
        mock_timeout.return_value = None

        async def fake_wait_for(coro, timeout=None):
            return await coro

        mock_proc = _mock_proc(b"ok", b"", 0)

        with (
            patch(
                "potato.agents.tools.shell.asyncio.create_subprocess_shell",
                AsyncMock(return_value=mock_proc),
            ),
            patch(
                "potato.agents.tools.shell.asyncio.wait_for",
                side_effect=fake_wait_for,
            ),
        ):
            # Invalid timeout string falls back to 60.0 default
            result = await _run_shell(
                "echo ok",
                timeout="invalid",
            )
            assert result.content is not None

    @pytest.mark.asyncio
    @pytest.mark.skipif(
        sys.platform == "win32",
        reason="NoneSandbox currently requires a POSIX shell",
    )
    async def test_sandbox_path_starts_with_running_python_bin(
        self,
        monkeypatch,
        tmp_path,
    ):
        from potato.agents.tools.shell import execute_shell_command

        system_bin = tmp_path / "system-bin"
        system_bin.mkdir()
        monkeypatch.setenv("PATH", str(system_bin))
        if sys.platform != "win32":
            monkeypatch.setenv("SHELL", "/bin/sh")

        script = "import os; print(os.environ.get('PATH', ''))"
        args = [sys.executable, "-c", script]
        command = (
            subprocess.list2cmdline(args)
            if sys.platform == "win32"
            else shlex.join(args)
        )
        config = SandboxConfig(
            mode=SandboxMode.NONE,
            workspace_dir=str(tmp_path),
            mounts=[MountSpec(path=str(tmp_path), writable=True)],
        )

        result = await execute_shell_command(
            command,
            cwd=tmp_path,
            sandbox_config=config,
        )

        path_entries = result.content[0].text.strip().split(os.pathsep)
        assert Path(path_entries[0]) == Path(sys.executable).parent
        assert config.env_vars == {}
        assert config.timeout_seconds == 30

    @pytest.mark.asyncio
    async def test_sandbox_uses_explicit_path_without_mutating_config(
        self,
        tmp_path,
    ):
        configured_path = os.pathsep.join(["custom", "bin"])
        config = SandboxConfig(
            mode=SandboxMode.NONE,
            workspace_dir=str(tmp_path),
            env_vars={"PATH": configured_path, "MASKED_SECRET": ""},
        )
        sandbox = AsyncMock()
        sandbox.execute.return_value = ExecutionResult(0, "ok", "")
        context_manager = MagicMock()
        context_manager.__aenter__ = AsyncMock(return_value=sandbox)
        context_manager.__aexit__ = AsyncMock(return_value=None)

        with patch(
            "potato.sandbox.create_sandbox",
            return_value=context_manager,
        ) as create_sandbox:
            await _execute_in_sandbox(
                "echo ok",
                config,
                12.9,
                str(tmp_path),
                {"PATH": os.pathsep.join(["venv", "system"])},
            )

        effective_config = create_sandbox.call_args.args[0]
        assert effective_config.env_vars == {
            "PATH": configured_path,
            "MASKED_SECRET": "",
        }
        assert effective_config.timeout_seconds == 12
        assert config.env_vars == {
            "PATH": configured_path,
            "MASKED_SECRET": "",
        }
        assert config.timeout_seconds == 30

    @pytest.mark.asyncio
    @pytest.mark.skipif(
        sys.platform == "win32",
        reason="POSIX live shell streaming",
    )
    async def test_slow_command_streams_chunks_qp_only_on_last(
        self,
        tmp_path,
    ):
        script = tmp_path / "slow.py"
        script.write_text(
            "import time\n"
            "for i in range(5):\n"
            "    print(f'line{i}', flush=True)\n"
            "    time.sleep(0.22)\n",
            encoding="utf-8",
        )
        cmd = f"{sys.executable} {script}"
        result = await execute_shell_command(cmd, cwd=tmp_path, timeout=10)
        assert hasattr(result, "__aiter__")
        chunks = []
        async for chunk in result:
            chunks.append(chunk)

        non_last = [chunk for chunk in chunks if not chunk.is_last]
        assert len(non_last) >= 2
        for chunk in non_last:
            assert "qp" not in (chunk.metadata or {})

        last = chunks[-1]
        assert last.is_last is True
        assert "qp" in last.metadata
        joined = "".join(chunk.content[0].text for chunk in chunks)
        assert "line0" in joined
        assert "line4" in joined
