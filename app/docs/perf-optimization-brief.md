# 工作包：后端启动/运行性能优化（全链路）

## 使命

把 Potato 后端的启动时延压到底，并顺带清理你自己发现的其他性能瓶颈。下面给出的是已完成的侦查数据和已知瓶颈清单——它们是**起点不是边界**：鼓励你自己 profile、自己发现清单之外的问题。判断标准只有一条：每项优化都要有 before/after 实测数据支撑，且不改变对外行为（API 语义、配置语义、功能可用性）。

## 已实测的基线（2026-08-04，本机 Apple Silicon）

后端启动到 `/api/healthz` 返回 200：
- 热启动（页缓存热）：**~2.8s**（渠道懒加载已落地后）
- 冷启动（当天首次，页缓存冷）：**~23.5s**（dev venv 与 1.1GB PyInstaller 侧车基本一致）
- `--log-level debug` 下框架自报 "Background startup completed in 2.525 seconds"（含 default + QA 两个 agent）

已落地、不要重做的优化：
1. 渠道 SDK 懒加载（`app/channels/registry.py` 的 `get_channel_keys`/`get_channel_class`；启动零渠道 SDK import，feishu 的 lark_oapi 数千个生成模块已移出启动路径）。
2. Tauri 窗口启动即显（splash 先行，感知时延与后端解耦）。
3. torch/whisper 移出桌面打包——**另一个 codex 实例正在做**，因此 `scripts/pack-tauri/**`（含 qwenpaw.spec、build_pyinstaller.sh/ps1）与 `.github/workflows` 的桌面打包 job **本工作包禁止触碰**，避免冲突。

## 已定位但未修的瓶颈（可直接开工）

1. **"后台启动"堵死事件循环（GIL 饥饿）**：`src/qwenpaw/app/_app.py` 的 `_background_startup()` 是 asyncio task，但其中大量同步 CPU/import 工作独占 GIL——实测 23s 冷启动期间 healthz 轮询（100ms 间隔）只有 3 个请求进得来。方向：把重段落放进 `asyncio.to_thread` 并保证等待点让 loop 呼吸、或在同步段落间加显式 `await asyncio.sleep(0)` 切片；目标是启动全程 healthz/静态资源/前端 API 保持低延迟可响应（可用"启动期间每 100ms 打一次 healthz、统计 p95 响应时间"验证）。
2. **两个 agent 串行全量 bootstrap**：default + 内置 QA agent（`multi_agent_manager.start_all_configured_agents`）。方向：并行化，或 QA agent 延迟到首次被使用时再起（确认 `on_core_ready` 只依赖 default 后，healthz 可以更早翻绿）。注意保持 `startup_ready`/`startup_state` 语义（healthz=core agents ready）。
3. **`import qwenpaw.app._app` 本身 1.71s**（-X importtime 实测）。已知大头：`google.genai.types` 246ms、`apscheduler.schedulers.base` 54ms、`fastapi.openapi.models` 46ms、`mcp.types` 39ms、`qwenpaw.config.config` 36ms、`openai.types.chat` 23ms、`anthropic.lib.streaming` 28ms。方向：provider SDK（google.genai/anthropic/openai）改函数级懒 import（provider 只在被配置激活时才需要）；routers 的重模块延迟加载。凡是能从模块顶层挪进函数体的重 import 都值得看。
4. **每日志行双写与访问日志噪声**：uvicorn access log 全开、部分模块日志重复（两个 agent 各注册一遍命令等）。属于顺手项，优先级最低。

## 你自己挖掘时的工具与坑（省得踩一遍）

- 计时口径统一用"进程启动→healthz 200"，脚本模式：fresh `QWENPAW_WORKING_DIR=$(mktemp -d)` + 固定端口 + 100ms 轮询 curl。注意跑完 kill 干净，端口冲突会污染下一次测量。
- 主线程栈采样：`faulthandler.register(signal.SIGUSR1, all_threads=True)` 包一层再启动，然后定时 `kill -USR1`。**必须 all_threads=True**——重活经常在 worker 线程（asyncio.to_thread），主线程栈看起来在 selectors.select 空转。
- `-X importtime` 只能抓到 CLI 早期：`qwenpaw.utils.stdio` 之后 stderr 被应用日志接管，输出会断流；且它会显著拖慢有海量小模块的 import（勿用它做绝对计时）。
- `py-spy` 在 macOS 需要 sudo，不可用；cProfile 只测主线程且干扰大。faulthandler 采样 + 分段日志时间戳是最可靠的组合。
- 冷启动无法本机复现（purge 需 sudo）：用"import 的字节量/模块数下降"作为冷启动改善的代理指标即可，不必强行测真冷。
- `qwenpaw app --log-level debug` 会打印框架自带的启动分段计时（`log_init_timings` + AgentStartupDisplay）。

## 边界（少而硬）

- 禁碰：`scripts/pack-tauri/**`、`.github/workflows/**`（另一实例在改）。
- 不改对外行为：healthz 语义、API 响应结构、配置格式、功能开关默认值。
- 不引入新依赖、不升级依赖版本。
- 不 commit——留给用户手动验收后处理。
- 工作目录里已有未提交改动（渠道懒加载、Tauri reveal、记忆系统修复等），它们是 baseline 的一部分：基于现状改，不要回退它们。
- 优化与验证的比例自己把握，但**不要陷入无限验证循环**：每项优化跑 3 次取中位数即可，测完就走。

## 交付物

1. 实施的优化（代码改动）。
2. `app/docs/perf-optimization-report.md`：每项优化一节——改了什么、为什么、before/after 数据（healthz 时延 + 启动期 healthz 响应性 p95 若适用）、风险点。你自己新发现但没来得及修的瓶颈单独列一节（附证据），作为后续工作包的输入。
3. `uv run --extra test pytest tests/unit -q` 全绿。
