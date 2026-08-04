# Potato 后端启动/运行性能优化报告

日期：2026-08-04
环境：本机 Apple Silicon，`src` 工作树，Python 3.11，固定回环地址，临时
`QWENPAW_WORKING_DIR`，每次使用全新临时目录。

## 结论

本工作包完成了三类优化：

1. 把后台 agent/service bootstrap 中的同步 import、文件扫描、构造和迁移移出
   asyncio 事件循环；
2. default 与内置 QA agent 保持并发启动，但 `healthz=200` 的 ready 边界改为
   只等待 default，QA 仍在后台继续完成；
3. provider SDK、local-model manager 和几个重路由依赖改为按实际使用懒加载。

在同一套本机启动脚本下，进程启动到第一次 `/api/healthz=200` 的三次中位数从
`2.111s` 降到 `1.404s`，下降约 `33.5%`。debug 分段计时的一次最终运行显示：
`Server ready 0.505s`，全量后台启动完成 `0.708s`。

没有修改 `scripts/pack-tauri/**` 或 `.github/workflows/**`，没有引入依赖或改动
配置格式、API 响应结构和功能开关默认值。

## 测量口径

- 启动命令：`python -m qwenpaw app --host 127.0.0.1 --port <fixed> --log-level warning`。
- 每 100ms 请求一次 `/api/healthz`；`ready_s` 从子进程创建开始计时，到首次
  HTTP 200 为止。
- `health_response_p95_ms` 只统计已经收到 HTTP 响应的请求，使用 nearest-rank
  p95；连接尚未建立的拒绝/超时单独计为 `connection_errors`，不把它们伪装成
  HTTP 响应延迟。
- 每一组跑 3 次取中位数。由于启动很快，首次 bind 前的轮询通常会被拒绝，
  因此每次启动只收到 2～3 个 503/200 响应；p95 应与响应样本数一起解读。
- `-X importtime` 仅作为 import 图代理指标，不作为绝对启动耗时；它会明显
  放大大量小模块的导入成本。

## 1. 消除后台 bootstrap 对事件循环的 GIL 饥饿

### 改了什么

- `MultiAgentManager.get_agent()` 用 `asyncio.to_thread` 创建 Workspace；
- `Workspace.start()` 将 skill-pool 初始化（含首次 import）、agent config 读取、
  legacy weixin 数据迁移移到 worker thread；
- `ServiceManager` 将 service class/参数解析、同步构造和同步 start 继续保持在
  worker thread，并在 post-init/start 前显式让出事件循环；
- Driver、Chat、Channel、AgentConfigWatcher 工厂的同步 import/构造和文件/配置
  扫描移到 worker thread；保留原有优先级、依赖顺序、async start/stop 顺序；
- plugin loader 构造、manifest discovery 和启动阶段 config 读取移到 worker
  thread。

### 为什么

原来的 `_background_startup()` 虽然是 asyncio task，但 task 内的同步 import、
Pydantic/config 解析、Workspace/service 构造会独占 GIL。只要这些段落没有遇到
`await`，Uvicorn 就无法及时处理 healthz、静态资源和其它 API。

### before/after 实测

同一脚本的原始基线（完成本工作包前）：

| run | healthz 200 | 已收到响应的 p95 |
|---|---:|---:|
| 1 | 2.208s | 1.400ms |
| 2 | 2.084s | 0.280ms |
| 3 | 2.111s | 1.360ms |
| **中位数** | **2.111s** | **1.360ms** |

最终代码（`final3`）：

| run | healthz 200 | 已收到响应的 p95 | 已收到响应 | connection errors |
|---|---:|---:|---:|---:|
| 1 | 2.104s | 7.342ms | 503, 200 | 20 |
| 2 | 1.404s | 2.393ms | 503, 200 | 13 |
| 3 | 1.402s | 13.985ms | 503, 200 | 13 |
| **中位数** | **1.404s** | **7.342ms** | **2** | — |

响应 p95 的样本很少，不能用来宣称吞吐提升；但最终三次实际收到的响应均为
个位数到低两位毫秒，未出现后台 CPU 段把已建立请求拖到秒级的情况。相比之下，
启动时间中位数减少了 `0.707s`。

### 风险

线程化只覆盖同步构造/import/文件工作；真正的 async service 生命周期仍在事件
循环中执行。对象构造可能创建 asyncio primitive，因此只把构造移到了不持有事件
循环的阶段，后续第一次 await 仍发生在主 loop；相关 workspace/service 回归测试
已覆盖这一点。线程池异常仍会沿原有启动错误路径传播，optional service 仍保持
原来的降级行为。

## 2. 缩短 core readiness：default ready 后不再等待 QA

### 改了什么

- `start_all_configured_agents()` 增加可选的 `ready_agent_ids`；未传该参数时，
  保留原有“等待全部 core agent”的行为；
- `_app.py` 传入 `ready_agent_ids=("default",)`；default 与 QA task 仍同时创建，
  QA 不会被取消，也不会改变最终 `start_all_configured_agents()` 返回值；
- `startup_ready`、`startup_state` 和 `/api/healthz` 的 JSON 结构保持不变；
  QA 完成前，healthz 200 的 `agents_loaded` 可能暂时只包含 default，这是新的
  ready 边界所定义的正常状态；需要 QA 的调用仍通过既有 lazy get-agent 路径等待。

### 为什么

QA agent 不属于 healthz 翻绿所必需的核心服务。之前 callback 在 default 和 QA
都完成后才触发，导致 default 已可服务时仍被 QA 的全量 bootstrap 拖住。

### before/after 实测

- 提供的原始 debug 基线：框架日志报告
  `Background startup completed in 2.525 seconds`。
- 最终 debug 运行：`Server ready in 0.505s (agents loading in background)`，
  `Background startup completed in 0.708 seconds`；同一运行中日志显示 default
  与 `QwenPaw_QA_Agent_0.2` 同时开始，且两者仍都成功完成。
- 受控 unit 测试保留了旧语义：不传 `ready_agent_ids` 时，
  `test_core_ready_waits_for_enabled_qa` 仍要求 callback 等待 QA；新增测试验证
  传入 default 后 callback 会在 QA release 前触发。

### 风险

这项优化有意改变 ready 的时间边界，但没有改变最终 agent 启动结果或配置语义。
任何把 healthz 200 当作“QA 已经加载”的外部调用方都应改用 agent 状态或 lazy
agent 获取接口；这也是本次明确记录的 ready contract。

## 3. provider SDK 与重路由依赖懒加载

### 改了什么

- `qwenpaw.providers` 只在访问 `ProviderManager` 时加载 registry；模型/schema
  import 不再触发所有 concrete provider；
- OpenAI、Gemini、Anthropic、Ollama、OpenRouter 等 provider 的 SDK import 移入
  实际 client/probe/model 创建路径；仅用于类型标注的 `ChatModelBase` 改为
  `TYPE_CHECKING`；
- 保留了可 patch 的惰性代理/异常兼容入口，既不恢复启动时 SDK import，也不破坏
  现有 provider unit test 和调用方的模块级替换方式；
- `LocalModelConfig` 拆到轻量 `local_models/schemas.py`，路由只在实际依赖注入时
  等待 runtime manager；manager 初始化期间相关 provider/local-model API 会等待
  readiness event，初始化失败则返回 503，而不是解引用 `None`；
- `skills_stream` 的 model factory、workspace 路由的 MD/memory manager 改为首次
  请求时导入。

### importtime before/after 代理

| 指标 | before | after |
|---|---:|---:|
| `import qwenpaw.app._app` 累计 | 1,429,390us | 627,537us |
| import module 数 | 3,840 | 1,619 |
| import self time 合计 | 1,435.13ms | 632.39ms |
| `google.genai.types` self time | 196.487ms | 不在启动 import 链 |
| `qwenpaw.providers.provider_manager` 累计 | 678.526ms | 不在启动 import 链 |

真实启动结果使用第 1 节的同口径 healthz 表：2.111s 中位数降至 1.404s。provider
首次真正被使用时仍会导入对应 SDK，功能路径的首次调用成本因此被保留在正确的
按需位置。

### 风险

首次使用某个 provider/model 的请求会承担一次性 import 成本；这是把成本从所有
启动迁移到真正需要该 provider 的请求。依赖注入等待逻辑只在 lifespan 创建的
readiness event 存在时生效，兼容不带完整 app state 的 router unit test。

## 4. 自发现并已修复的额外阻塞点

profile 与 debug 分段日志显示，原先几个“async factory”在函数第一次 await
之前仍执行大量同步 import/构造。它们不在最初清单中，但会在两个 agent 并发时
争用 GIL。已按第 1 节的 service factory 线程化一起修复；最终 debug 日志中两个
workspace 的 bootstrap/start 成功，且无 API/功能回归。

## 尚未修复、留给后续工作包的问题

### 路由总装仍有一段不可忽略的导入成本

`qwenpaw.app.routers` 最终 importtime 仍约 `392.8ms`；剩余热点包括
`fastapi.openapi.models`、`mcp.types`、部分 workspace/MCP 路由。它们涉及 FastAPI
路由注册顺序、OpenAPI schema 或 agent-scoped 路由，继续粗暴推迟到 lifespan 之后
可能导致启动期间 404 或改变 OpenAPI/SPA 路由优先级，因此本次只处理了明确安全的
provider、model factory 和 workspace-manager 依赖。

### 两个 agent 的命令注册日志仍重复

最终 debug 日志中 default 与 QA 各自注册一遍 `/stop`、`/daemon ...`、`/status` 等
命令；同一组 `CommandRegistry` 日志成对出现。这是 per-workspace registry 与
全局控制命令边界的问题，简单去重日志可能掩盖实际重复注册，暂未修改。后续应先
确认命令 dispatch 是否允许共享 registry，再决定复用实例或仅降噪。

### Uvicorn access log 仍默认开启

`src/qwenpaw/cli/app_cmd.py` 和桌面入口仍保留 access log，频繁 healthz/static
请求会产生访问日志 I/O。本次没有改变日志可见性契约；可作为低风险后续项，优先
考虑默认关闭、debug/trace 开启，并为显式 access-log 需求保留开关。

### bind 前的连接拒绝不是 event-loop 响应延迟

最终三次压测仍分别记录 13～20 次 connection error，原因是 CLI 进程需要先完成
app import/Uvicorn bind；这些请求没有 HTTP 响应，未计入 p95。若还要压低感知到的
“端口可连接”时间，需要把 socket bind/sidecar reveal 与 app import 解耦，属于
桌面入口/启动编排方向，未触碰本工作包禁止修改的打包文件。

## 验证结果

最终执行：

```text
uv run --extra test pytest tests/unit -q
```

结果：`5578 passed, 8 skipped, 5 warnings in 116.96s`。期间受影响的
provider/router/workspace/agent 集合单独验证为 `426 passed`，首次全量运行发现的
11 个模块级 monkeypatch 兼容问题已通过惰性代理修复并重跑通过。
