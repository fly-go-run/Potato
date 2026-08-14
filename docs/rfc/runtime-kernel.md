# RFC: Runtime Kernel — 注册生命周期与服务依赖图

状态：r2（吸收对抗审查 `runtime-kernel-review-r1.md` 后的定稿）
分支：`refactor/runtime-kernel`
来源：DeepSeek Harness（commit `47f9438`）架构考察结论 —— 借其约束，不搬其框架。

## 0. 背景与判断

DeepSeek Harness 的四条核心约束值得 Potato 吸收：

1. 每个注册都是可逆 effect —— 注册产生 disposer，插件卸载时自动反向清理。
2. 服务按声明的依赖激活，而不是靠启动顺序的隐性约定。
3. 工具执行是一条统一管线，模型可见结果与 UI 展示元数据分离。
4. 会话有一条 append-only 事件日志，"model-visible means logged"。

本 RFC 只覆盖 **第 1、2 条（P0）**。第 3 条（Tool Runtime）与第 4 条
（观察者式审计日志——**不做**"日志作为真相源"：AgentScope 拥有消息
历史所有权，强行夺权会导致永久双写）留待后续 RFC。

明确的非目标（已定案）：

- 不迁移 TypeScript / Cordis。
- 不在本阶段收敛 `PluginRegistry` 单例与 `WorkspacePlugins` 两层结构。
- 不引入前端插件系统。
- 行为等价优先：用户可见行为不变；例外见 §3.4（关停安全属真 bug 修复）。

## 1. 四个概念

| 概念 | 定义 | P0 落点 |
|---|---|---|
| **RegistrationHandle** | 一次注册（或一组原子注册）的所有权凭证，dispose 精确撤销 | 核心交付 |
| **Scope** | handle 的有序容器，关闭时逆序释放全部 | `PluginScope`（每插件一个，由 `PluginApi` 持有） |
| **Service** | ServiceManager 管理的生命周期对象 | 依赖图真正生效 |
| **Event** | 类型化事件（Runtime Hook 8 阶段已是雏形） | P0 不动 |

## 2. P0-A：RegistrationHandle 与 PluginScope

### 2.1 现状（已核实，含审查补充）

- 全局 `PluginRegistry` 是 Singleton；卸载走 `unregister_plugin()`
  （registry.py:934）逐表过滤重建。
- **但 PluginRegistry 只是一级索引**（审查 P0-2）：`_post_load_setup`
  之后还有二级注册——provider 写入 `ProviderManager`
  （_app.py:436-446、routers/plugins.py:166-183）、control command 写入
  全局 handler 与 channel priority registry（_app.py:454-470）、tool 写入
  治理表 / `qwenpaw.agents.tools` 模块属性 / 各 workspace `ToolRegistry` /
  bootstrap 列表（plugins/api.py:821-863），现有卸载对此有专门的
  `_cleanup_plugin_tools`（loader.py:1304-1414）。
- **延迟注册**（审查 P0-1）：startup hook / workspace-created hook 在
  加载完成之后才执行（_app.py:486-501、routers/plugins.py:212-219），
  回调内继续向既有 workspace 注册 slash/mode/hook/stop_handler
  （plugins/api.py:937-951, 1015-1032, 1068-1085）。
- `WorkspacePlugins` 实际是四个 registry + `register_mode()` +
  可直接 append 的 `stop_handlers` 列表；fallback 由
  `SlashCommandRegistry.register_fallback()` 单独写入。

### 2.2 目标设计

新模块 `src/qwenpaw/runtime/registration.py`：

```python
Disposer = Callable[[], None] | Callable[[], Awaitable[None]]

class RegistrationHandle:
    """One (atomic group of) registration's ownership token."""
    def __init__(self, dispose_fn: Disposer, *, tag: str = ""): ...
    def dispose_sync(self) -> None:       # 仅当 disposer 为同步；否则抛
    async def dispose(self) -> None:      # 统一入口，同步 disposer 直接调
    @property
    def is_async(self) -> bool: ...
    @property
    def disposed(self) -> bool: ...       # dispose 幂等

class Scope:
    """Ordered container; aclose() disposes in reverse order."""
    def add(self, handle: RegistrationHandle) -> RegistrationHandle: ...
    def child(self, tag: str) -> "Scope": ...   # 复合注册用（mode）
    async def aclose(self) -> None:
    @property
    def closed(self) -> bool: ...
```

设计裁决（吸收审查 P0-1/P0-2/P0-4/P1-3）：

1. **`await scope.aclose()` 是唯一卸载原语**。`unregister_plugin()`
   保留为兼容 API，但 loader 的卸载编排统一走 async 路径；不存在
   "sync close 静默丢弃 coroutine"的情形——scope 内含 async handle 时
   同步关闭直接抛错。
2. **PluginApi 构造时持有唯一 `PluginScope`**，所有 API 方法内部自动
   `scope.add()`。**不依赖插件调用方接返回值**。startup /
   workspace-created 回调内经 PluginApi 产生的注册同样自动挂入同一
   scope（回调闭包携带 api 引用，天然成立；需测试锁死）。
3. **二级注册 handle 化**：`_post_load_setup` 里 provider → ProviderManager、
   control command → 两个全局 registry、tool → 治理表/模块属性/
   workspace ToolRegistry/bootstrap 的每一步都产生 handle 进同一 scope。
   `_cleanup_plugin_tools` 的逻辑拆解为这些 handle 的 disposer，原函数
   退役；卸载语义与现状逐项对齐（以现函数为行为基准）。
4. **边界声明**（审查 P0-4）："load→unload 等价"只保证 **host API 创建
   的 effect**。插件自造的副作用（sys.path、atexit、monkey-patch、
   synthetic module）不在自动保证内；新增
   `api.add_disposer(fn, *, tag)` 让插件显式登记自定义清理。仓内
   qwenpaw-pet / cloudpaw 迁移到该 API 列为**后续任务**，不阻塞 P0。
5. **WorkspacePlugins 补齐注册入口**（审查 P1-1）：新增
   `register_stop_handler()`（撤销按 registration 对象身份）、
   fallback 注册可撤销；`register_mode()` 改事务式——mode setup 扇出的
   slash/tool/hook/prompt 子注册聚合进一个 child scope，任一步失败
   立即逆序回滚，卸载时整体撤销。built-in bootstrap（workspace.py:154-232）
   返回 handle 但归属 workspace 自身，不入任何插件 scope。
6. **HTTP route disposer**（审查 P1-2）：按注册时捕获的 route 对象集合
   精确删除，清 prefix 所有权，并置 `app.openapi_schema = None` 失效
   缓存。
7. Scope 关闭时单个 disposer 抛异常不得中断其余清理：捕获、记日志、
   继续，最后聚合报告。
8. `HookRegistry` 的 disposer 需同步清 `_sorted_cache`。

### 2.3 验收标准（P0-A）

主断言（审查 P1-8 裁剪版）：

- **运行时级等价测试**：起最小 app + 一个测试插件（覆盖 provider/
  control command/tool/http route/prompt section/startup hook 注册
  workspace slash+mode+hook+stop_handler），load → unload 后断言：
  PluginRegistry 各表、ProviderManager、两个 command registry、
  工具治理表与模块属性、FastAPI routes + 访问过的 `/openapi.json`、
  目标 workspace 四注册面 + stop_handlers，全部与从未加载相等。
- 双插件交错注册后卸载其一，另一插件注册完好。
- 异步 disposer 混在同步 handle 中，aclose 全量清理；含 async handle
  的 scope 被同步关闭时抛错。
- disposer 抛异常不阻断其余清理。
- `register_mode()` setup 中途失败时无半注册残留。
- `unregister_plugin()`/`_cleanup_plugin_tools` 中不再有逐表过滤逻辑。
- 现有全部单测通过。

Backlog（不阻塞 P0，记录在案）：PawApp 直接卸载路径绕过
`_post_unload_cleanup`（routers/pawapps.py:192-202）、force reinstall
全矩阵、live channel 实例关停语义、PawApp TaskManager 按 app_id
cancel、仓内插件自定义副作用迁移 `add_disposer`。

## 3. P0-B：ServiceManager 依赖图

### 3.1 现状（已核实，含审查补充）

- `start_all()`（service_manager.py:176）只按 priority 分组并发启动；
  `stop_all()` 反向 priority、组内并发。
- **8 个生产 descriptor 全部零依赖声明**（workspace.py:302-435）——
  依赖全靠 priority 数值约定。审查盘点出的真实约束：
  - `channel_manager` ← `local_workspace`, `session`, `chat_manager`
    （required）；`memory_manager`, `driver_manager`（可缺失能力）。
  - `cron_manager` ← `channel_manager`, `chat_manager`（required）；
    `memory_manager`（dream 功能条件依赖）。
  - `agent_config_watcher` ← `channel_manager`, `cron_manager`
    （order-only：门控逻辑是 OR，缺一仍可建）。
  - `driver_config_watcher` ← `driver_manager`（order-only：缺失时
    factory 返回 None 安静跳过）。
- reused 服务（memory/chat_manager）的 `reload_func` 在 `set_reusable()`
  时立即执行，早于 start_all。
- workspace 启动失败时 `stop()` 因 `_started == False` 早退，已启动
  服务泄漏（workspace.py:515-521, 568-577）。

### 3.2 依赖边语义（吸收审查 P0-3）

`dependencies` 拆成两种边，均参与拓扑排序：

```python
dependencies: List[str]        # required：dep 必须存在且启动成功
after: List[str]               # order-only：若 dep 已注册则排其后；
                               # dep 缺失/失败/为 None 不阻塞本服务
```

失败传播规则：

- required dep 缺失（未注册）→ 首次 start_all 前的图校验报
  `ServiceDependencyError`（列出缺什么、谁要的）。
- required dep 启动失败 → 依赖方标记 `skipped`；依赖方若 optional
  则记日志继续，若必选则整个启动失败（与现状"必选服务失败即
  workspace 启动失败"一致）。
- order-only dep 任何状态 → 只影响顺序，不传播失败。factory 自行
  处理 None（现状行为，如 driver_config_watcher）。
- 循环依赖（两种边合并成图）→ 图校验报错并给出环路径。

按 §3.1 盘点补声明：channel/cron 的 required 边用 `dependencies`，
两个 watcher 的约束用 `after`。**这是行为等价的编码化，不是新行为。**

### 3.3 启动/停止算法（吸收审查 P1-9/P2-1）

1. **先纯计算后启动**：图校验（缺边、环）在构造任何 service 之前完成。
2. 分层拓扑（Kahn）；层内按 priority 分组，保留 `concurrent_init`
   并发与组间 `await asyncio.sleep(0)` 让出。
3. 每个节点显式记录结果：`started | reused | skipped_optional | failed`；
   `start_all()` 返回该状态表（结构化，供测试与诊断）。
4. **停止 = 逆拓扑层，层间严格 await，层内并发**（不再宣称"精确
   逆序"——并发层内只有偏序）。注册序作日志/测试的确定性 tiebreak。
5. **启动中途失败按实际 started 集合逆拓扑回滚**（修 workspace
   `_started` 早退泄漏）。
6. reused 服务参与拓扑（视为已满足其自身启动），**`reload_func` 延迟
   到其拓扑位置执行**（set_reusable 只登记实例；审查 P1-5）。
7. **borrowed 语义**（审查 P1-7）：reload 注入的 reused 实例标记
   borrowed；新 workspace 启动失败的回滚**永不 stop borrowed 实例**，
   所有权仍归旧 workspace。

### 3.4 关停安全（审查 P1-6；此项是真 bug 修复，非行为等价）

- CronManager 跟踪其派发的执行 task，`stop()` cancel 并 await 收尾，
  之后 channel / core service 才停止。独立 commit。

### 3.5 验收标准（P0-B）

- 单测：缺失 required 边预检报错、环报错（含 after 边参与的环）、
  required dep 失败传播（optional 依赖方跳过 / 必选依赖方致启动失败）、
  order-only 不传播、reused 参与拓扑 + reload_func 延迟执行、
  `D(rebuild)→R(reuse)→C` 传递依赖、启动失败按 started 集合回滚且
  不 stop borrowed、priority tiebreak、逆拓扑停止分层 await。
- **生产图快照测试**（审查 P1-4）：断言 workspace.py 注册全集的
  期望边集合与期望启动分层（防空图空过、防后来漏声明）。
- 冒烟：`QWENPAW_WORKING_DIR=<临时目录> uv run qwenpaw app` 起服务，
  断言 start_all 返回的结构化状态表全部 `started|reused`（不做日志
  文本比对；审查 P2-2）。
- 现有全部单测通过。

## 4. 后续阶段（本 RFC 不实施，仅锚定）

- P1 Tool Runtime 统一管线；model-facing canonical JSON 与 UI meta 分离。
- P1.5 持久化下移（checkpoint-policy 模式，热路径不 await 落盘）。
- P2 观察者式审计事件日志（只写不读；永不作为真相源）。
- P2 三 Scope 收敛（App/Workspace/Request），PluginRegistry 单例退役。
- P3 Mode → AgentPreset。
- Backlog：见 §2.3；另有 PawApp router `dependencies.append` 原地
  mutation 无 disposer（pawapp/app.py:241-256）。

## 5. 工程纪律

- commit 粒度：P0-A 基础设施 / PluginApi 接线 / 二级注册 handle 化 /
  WorkspacePlugins 入口 / P0-B 图算法 / 补声明 + 生产图快照 /
  关停安全修复，各自独立 commit；行为等价重构与新增测试分开。
- 不引入新依赖；不顺手重构无关代码（记录到 PR 描述）。
- 测试命令：`uv run pytest tests/unit -q`。
