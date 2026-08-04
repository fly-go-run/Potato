# 桌面侧车瘦身验证报告

日期：2026-08-04

## 结论

桌面 PyInstaller 打包链路已切换到 `.[local]`，并完成一次全量重建。新侧车已自动同步到 Tauri 资源目录，体积从 `1,151,448 KiB` 降至 `534,320 KiB`，减少 `617,128 KiB`（约 `602.7 MiB`，`53.6%`）。

PyInstaller 硬校验通过；`_internal/torch`、`_internal/whisper`、`_internal/numba`、`_internal/llvmlite`、`_internal/triton`、`_internal/tiktoken` 均不存在。

端口冒烟和完整单测在本机受限沙箱中受到同一个系统能力限制：沙箱禁止本地 TCP `bind`，因此无法取得 READY/healthz 计时，单测也有 8 个 socket 相关失败。代码和打包产物本身已完成验证，需在允许 localhost 监听的 CI/本机环境补跑这两项。

## 改动摘要

本工作包涉及的代码 diff 仅限以下三个桌面打包文件（工作区原有的 `src/qwenpaw/**`、前端和测试改动未触碰）：

```text
scripts/pack-tauri/build_pyinstaller.sh  | 22 ++++++++++++++++++----
scripts/pack-tauri/build_pyinstaller.ps1 | 11 +++++++++--
scripts/pack-tauri/qwenpaw.spec          |  5 +----
```

- `build_pyinstaller.sh`：依赖安装从 `.[full]` 改为 `.[local]`；依赖指纹改为 `extras=local`；在 PyInstaller 前卸载 `openai-whisper`、`torch`、`numba`、`llvmlite`、`triton`、`tiktoken`；构建后硬拒绝顶层 `torch`/`whisper` 目录。
- `qwenpaw.spec`：移除 Whisper 数据、`openai-whisper` metadata 和 `collect_submodules("whisper")` 收集；加入 `torch`、`whisper`、`numba`、`llvmlite`、`triton` excludes。
- `build_pyinstaller.ps1`：同步改用 `.[local]`、更新文案，并加入同等的残留包卸载逻辑。
- `pyproject.toml`、`uv.lock`、`src/qwenpaw/**`、`scripts/pack/**` 均未作为本工作包修改。

## 体积与目录校验

重建前基线：

```text
du -sh dist/pyinstaller/qwenpaw-backend
1.1G
du -sk dist/pyinstaller/qwenpaw-backend
1151448
```

重建后：

```text
du -sh dist/pyinstaller/qwenpaw-backend
522M
du -sk dist/pyinstaller/qwenpaw-backend
534320

du -sh console/src-tauri/binaries/qwenpaw-backend
522M
du -sk console/src-tauri/binaries/qwenpaw-backend
534320
```

`dist/pyinstaller/qwenpaw-backend` 与 `console/src-tauri/binaries/qwenpaw-backend` 的侧车可执行文件均为 `89,386,816` bytes，确认脚本已自动同步新产物。

旧产物的顶层 `_internal` 中存在：`torch`、`whisper`、`numba`、`llvmlite`、`tiktoken`；新产物的同名目录检查结果为 `none`。

新产物中可见的 `transformers/models/whisper` 是基础依赖 Transformers 自带的纯 Python 模型支持，不是 `openai-whisper` 的顶层 `whisper` 包，也不包含 torch runtime；本报告的硬校验针对打包链路要求的顶层目录。

## PyInstaller 构建

要求的入口命令已执行：

```bash
QWENPAW_FORCE_PYINSTALLER=1 bash scripts/pack-tauri/build_pyinstaller.sh
```

本机沙箱无法访问原 uv cache、网络也不可用，因此先遇到 uv cache `EPERM` 和 macOS `system-configuration` 初始化崩溃；随后使用本机已有的 PyInstaller/hook 缓存、venv 现有依赖和临时 PyInstaller cache 完成了同一脚本的构建。最终输出：

```text
PyInstaller: 6.21.0
Build complete
Bundle size: 522M
Copied to: console/src-tauri/binaries/qwenpaw-backend
```

PyInstaller 分析日志同时显示 `PyTorch was not found`，构建后的 shell 硬校验通过。

## 侧车冒烟

按要求启动新产物并轮询 READY/healthz。侧车完成了工作区初始化和内置 skill 同步，但在分配端口时退出：

```text
Traceback ...
  File "qwenpaw/utils/port.py", line 99, in find_free_port
    sock.bind((host, 0))
PermissionError: [Errno 1] Operation not permitted
```

同一限制可用最小复现确认与侧车内容无关：

```bash
.venv/bin/python -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0))'
# PermissionError: [Errno 1] Operation not permitted
```

结果：`QWENPAW_BACKEND_READY` 和 `/api/healthz` 200 计时在当前沙箱中不可取得；不是侧车初始化或 Whisper 缺包错误。需在允许 localhost TCP bind 的 runner 上补测，以获得与基线约 `3.0s / 4.5s` 的对比数据。

## 转写兜底

```text
audio_transcription import: OK
availability = {'available': False, 'ffmpeg_installed': True, 'whisper_installed': False}
local_whisper_result = None
warning = Local Whisper unavailable (missing: openai-whisper). Install the missing dependencies to use local transcription.
```

`audio_transcription.py` 中 `import whisper` 仍在函数体内懒加载；`local_whisper` 选择路径保留，缺包时给出上述可读告警并返回 `None`。`whisper_api` provider 路径仍保留，云端转写不受本次打包改动影响。

## 单元测试

要求的命令首次执行结果：

```text
uv run --extra test pytest tests/unit -q
error: failed to open file /Users/liuxu/.cache/uv/sdists-v9/.git: Operation not permitted
```

使用临时 uv cache 重试时，uv 在 macOS 系统配置初始化处崩溃：`Attempted to create a NULL object`。因此用同一个 `.venv` 直接执行等价测试目标：

```bash
.venv/bin/python -m pytest tests/unit -q
```

结果：

```text
5569 passed, 8 skipped, 8 failed, 5 warnings in 111.85s
```

8 个失败均为沙箱禁止 socket bind 导致：

- `tests/unit/channels/test_onebot_channel.py`：5 个生命周期/看门狗测试；
- `tests/unit/local_models/test_llamacpp_backend.py`：2 个端口选择测试；
- `tests/unit/tauri/test_entry.py::test_socket_port_returns_bound_port`：1 个绑定端口测试。

## 全仓一致性检查

执行 `grep -rn '\[full\]\|extras=full' scripts .github/workflows` 后：

- `scripts/pack-tauri` 中无 `[full]` 或 `extras=full`，只有 `.[local]` 和 `extras=local`；
- `.github/workflows` 中无桌面构建的精确 `[full]` 引用；通用测试/预览工作流中的组合 extras（如 `.[dev,test,full]`）保持不变；
- `scripts/pack/` 仍保留 `qwenpaw[full]` 的 legacy conda 打包引用及文档引用，按边界要求未修改。

最后复核：`bash -n scripts/pack-tauri/build_pyinstaller.sh` 和 `git diff --check` 均通过；本机无 PowerShell 解析器，`build_pyinstaller.ps1` 未在本机执行。
