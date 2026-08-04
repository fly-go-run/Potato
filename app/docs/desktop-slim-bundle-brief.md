# 工作包：桌面侧车去掉 torch / whisper（瘦身 ~500MB + 冷启动提速）

## 背景与目标

桌面 PyInstaller 侧车目前 1.1GB，其中 `_internal/torch` 408MB + whisper 及其依赖链（numba/llvmlite/tiktoken 等），全部来自打包时安装 `.[full]`（`full = qwenpaw[local,whisper]`，whisper → openai-whisper → torch）。用户已拍板：**桌面版不需要本地 Whisper 转写，torch 与 whisper 整链从桌面分发中移除**。云端转写（`whisper_api` / provider 路径）必须保持可用。

运行期已确认安全：`import whisper` 只出现在 `src/qwenpaw/agents/utils/audio_transcription.py` 的函数体内（懒加载）；当前 dev `.venv` 本来就没有 torch/whisper，整套后端与 649 个单测运行正常。**不要改任何 src/qwenpaw 运行时代码**（含 config.py 的 `local_whisper` 枚举——保留选项，运行期缺包时的报错路径已存在）。

## 改动范围（只动打包链路，不动 pyproject extras）

`pyproject.toml` 的 `full`/`whisper` extras 保留不动（PyPI/服务器用户还要用）。桌面打包从 `.[full]` 降级为 `.[local]`：

### 1. `scripts/pack-tauri/build_pyinstaller.sh`

- L151 附近：`install_python_packages -e ".[full]"` → `install_python_packages -e ".[local]"`，同步修改紧随的 echo 文案。
- L100 附近 `calculate_fingerprint()` 里的 `printf 'extras=full\n'` → `extras=local`（否则签名缓存不失效，旧 1.1GB 侧车会被直接复用）。同理检查 `calculate_dependency_fingerprint` 是否也编码了 extras 字符串。
- 安装依赖之后、运行 PyInstaller 之前，**防御性卸载**残留包（打包用的是仓库 `.venv`，历史上装过 full）：用现成的 `uninstall_python_package` 逐个卸载 `openai-whisper` `torch` `numba` `llvmlite` `triton` `tiktoken`（tiktoken 若被其他直接依赖需要则跳过——先 `uv pip tree`/`pip show` 确认 requiredby，再决定）。
- PyInstaller 成功后加**硬校验**：若 `dist/pyinstaller/qwenpaw-backend/_internal/torch` 或 `_internal/whisper` 目录存在则 `echo` 错误并 `exit 1`（防止未来某个依赖静默把 torch 拖回来）。

### 2. `scripts/pack-tauri/qwenpaw.spec`

移除三处 whisper 显式收集（L91 `collect_data_files("whisper")`、L125 `"openai-whisper"` 所在列表项、L184 `collect_submodules("whisper")`——以实际 grep 为准，可能行号有漂移）。并在 `Analysis(...)` 增加 `excludes=["torch", "whisper", "numba", "llvmlite", "triton"]`（belt-and-braces；若 Analysis 已有 excludes 参数则合并）。

### 3. `scripts/pack-tauri/build_pyinstaller.ps1`

L133 附近 `Install-PythonPackages -Packages @("-e", ".[full]")` → `".[local]"`；若 ps1 有对应 fingerprint/extras 字符串与卸载逻辑，做与 sh 等价的修改（Windows 本地无法验证，改代码即可，CI 后验）。

### 4. 全仓一致性检查

`grep -rn '\[full\]\|extras=full' scripts .github/workflows` 逐个核对：`scripts/pack/`（legacy conda 打包）**不要动**；`.github/workflows` 里若有桌面构建装 `.[full]` 的步骤（desktop-build/fork-verify-desktop/release 等）改成 `.[local]`，非桌面链路（PyPI publish、服务器测试）不动。

## 验证（必须全部执行并在报告里贴数据）

1. `QWENPAW_FORCE_PYINSTALLER=1 bash scripts/pack-tauri/build_pyinstaller.sh` 全量重建成功。
2. `du -sh dist/pyinstaller/qwenpaw-backend`（或脚本产物目录）报告前后大小对比；确认 `_internal/` 下无 `torch`、`whisper`、`numba`、`llvmlite` 目录。
3. 侧车冒烟：
   ```bash
   WD=$(mktemp -d); QWENPAW_DESKTOP_APP=1 QWENPAW_WORKING_DIR=$WD PYTHONUNBUFFERED=1 \
     <产物目录>/qwenpaw-backend &
   # stdout 等到 "QWENPAW_BACKEND_READY {\"port\":N}" 后：
   curl -sf http://127.0.0.1:<port>/api/healthz   # 轮询直到 200，记录耗时
   ```
   healthz 200 即通过；顺带记录 ready 时间（对比基线：改动前热启动 ready ~3.0s / healthz ~4.5s）。
4. 转写兜底不崩：`.venv/bin/python -c "from qwenpaw.agents.utils import audio_transcription"` import 正常；再确认选了 `local_whisper` 时的调用路径抛出的是可读错误（读代码即可，不必真跑音频）。
5. `uv run --extra test pytest tests/unit -q` 全绿（改动不应影响任何测试）。
6. 若 `console/src-tauri/binaries/qwenpaw-backend` 是脚本自动同步的产物目录，确认新侧车已同步（脚本本身会做就不要手动 cp）。

## 边界与禁区

- 不改 `pyproject.toml`、不改 `uv.lock`、不改 `src/qwenpaw/**` 任何运行时代码。
- 不动 `scripts/pack/`（legacy）。
- 不做"顺手优化"（重构脚本、升级 PyInstaller 等一律不做）。
- PyInstaller 构建耗时数分钟属正常，不要因为慢就中断重试循环；构建失败贴完整报错后停下来，不要自行发散修依赖版本。

## 交付物

改动 diff + 验证报告（大小前后对比、冒烟计时、测试结果、grep 一致性检查结论），写入 `app/docs/desktop-slim-bundle-report.md`。
