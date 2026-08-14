# RFC: Runtime Kernel — 注册生命周期与服务依赖图

状态：Draft r1（架构师：Claude；实现与审查：codex gpt-5.6-sol high）
分支：`refactor/runtime-kernel`
来源：DeepSeek Harness（commit `47f9438`）架构考察结论 —— 借其约束，不搬其框架。

## 0. 背景与判断

DeepSeek Harness 的四条核心约束值得 Potato 吸收：

1. 每个注册都是可逆 effect —— 注册返回 disposer，插件卸载时自动反向清理。
2. 服务按声明的依赖激活，而不是靠启动顺序的隐性约定。
3. 工具执行是一条统一管线（validate → pre → execute → post → canonical result），
   模型可见结果与 UI 展示元数据分离。
4. 会话有一条 append-only 事件日志，"model-visible means logged"。

本 RFC 只覆盖 **第 1、2 条（P0）**。第 3 条（Tool Runtime）与第 4 条
（观察者式审计日志——注意：**不做**"日志作为真相源"，因为 AgentScope
拥有消息历史的所有权，强行夺权会导致永久双写）留待后续 RFC。

明确的非目标（已定案，review 时不要挑战这些）：

- 不迁移 TypeScript / Cordis。
- 不在本阶段收敛 `PluginRegistry` 单例与 `WorkspacePlugins` 的两层结构
  （三 Scope 收敛是独立的后续手术）。
- 不引入前端插件系统。
- 不改变任何现有运行时行为——P0 是纯结构性重构，行为等价。

## 1. 四个概念

| 概念 | 定义 | P0 落点 |
|---|---|---|
| **RegistrationHandle** | 一次注册的所有权凭证，`dispose()` 精确撤销该次注册 | 本 RFC 核心交付 |
| **Scope** | 一组 RegistrationHandle 的容器，`close()` 逆序释放全部 | `PluginScope`（每插件一个） |
| **Service** | ServiceManager 管理的生命周期对象 | 依赖图真正生效 |
| **Event** | 类型化事件（Runtime Hook 8 阶段已是雏形） | P0 不动，后续 RFC |

## 2. P0-A：RegistrationHandle 与 PluginScope

### 现状（已核实）

- 全局 [PluginRegistry](../../src/qwenpaw/plugins/registry.py) 是 Singleton，
  约 10 张注册表（providers/hooks×4/control_commands/channels/middleware/
  http_routers/prompt_sections）。卸载走 `unregister_plugin()`
  （registry.py:934），对每张表做 `plugin_id` 过滤重建 —— 每加一种注册类型
  都要记得改这里，已是隐性 bug 源。
- Per-workspace [WorkspacePlugins](../../src/qwenpaw/app/workspace/workspace_plugins.py)
  持有 slash/hook/tool/prompt/modes/stop_handlers 六个注册面，各自的
  `register()` 均返回 `None`，无统一撤销路径。

### 目标设计

新模块 `src/qwenpaw/runtime/registration.py`：

```python
class RegistrationHandle:
    """One registration's ownership token. dispose() is idempotent."""
    def __init__(self, dispose_fn: Callable[[], None], *, tag: str = ""): ...
    def dispose(self) -> None: ...
    @property
    def disposed(self) -> bool: ...

class Scope:
    """Ordered container of handles; close() disposes in reverse order."""
    def add(self, handle: RegistrationHandle) -> RegistrationHandle: ...
    def close(self) -> None:          # 同步注册的逆序清理
    async def aclose(self) -> None:   # 允许异步 disposer（如 http 卸载）
    @property
    def closed(self) -> bool: ...
```

要求：

1. **所有** `register_*` 方法改为返回 `RegistrationHandle`（当前返回
   `None`，改返回值向后兼容——现有调用方不接返回值照常工作）。
2. disposer 必须精确撤销"这一次"注册（按身份，不按 plugin_id 过滤），
   幂等，重复 dispose 是 no-op。
3. `PluginRegistry.unregister_plugin()` 保留为兼容入口，内部改为
   `plugin_scope.close()`；`loader.py` 为每个插件建 `PluginScope`，
   加载路径上的每次注册都挂进该 scope。
4. Scope 关闭时单个 disposer 抛异常不得中断其余清理：捕获、记日志、
   继续，最后聚合报告（参考 ExitStack 语义，但不要直接用 ExitStack，
   我们需要 tag 与诊断信息）。
5. `WorkspacePlugins` 的六个注册面同样返回 handle。Workspace 关停时
   整个对象被丢弃，所以这里 handle 的近期价值是**动态 Mode/技能开关**
   的精确撤销；不要为此改变 Workspace 生命周期。

### 验收标准（P0-A）

- 现有全部单测通过，插件 load → unload → reload 循环后各注册表状态
  与从未加载时逐表相等（新增测试断言这一点）。
- 新增测试：双插件交错注册后卸载其一，另一插件的注册完好。
- 新增测试：disposer 抛异常不阻断 scope 内其余清理。
- `unregister_plugin()` 中不再存在按 plugin_id 的逐表过滤逻辑。

## 3. P0-B：ServiceManager 依赖图

### 现状（已核实）

[service_manager.py](../../src/qwenpaw/app/workspace/service_manager.py)
的 `ServiceDescriptor.dependencies` 字段已声明（:73），docstring 宣称
"dependency handling"，但 `start_all()`（:176）实际只按 `priority` 分组
并发启动，dependencies 完全未参与排序。依赖满足目前靠 priority 数值的
人工约定，是隐性耦合。

### 目标设计

`start_all()` 改为分层拓扑排序（Kahn），规则：

1. **拓扑层**：一个服务的所有 dependencies 启动完成后才可启动。
2. **层内排序**：同一拓扑层内按现有 `priority` 排序分组——priority 降级
   为 tiebreaker，保留现有 `concurrent_init` 并发语义与组间
   `await asyncio.sleep(0)` 让出事件循环的行为（启动期间要能响应 HTTP）。
3. **缺失依赖 = 启动时显式报错**（`ServiceDependencyError`，列出缺什么、
   谁要的）。不学 Cordis 的静默 PENDING —— 桌面应用里"服务悄悄没起来"
   是最坏的失败模式。例外：依赖一个 `optional=True` 且启动失败的服务时,
   依赖方本身若也是 optional 则跳过并记日志，若是必选则报错。
4. **循环依赖 = 注册后首次 start_all 时显式报错**，错误信息给出环路径。
5. `stop_all()` 改为启动序的精确逆序。
6. 保留 `reused_services` 跳过逻辑：被跳过的服务视为"已满足"参与拓扑。

### 迁移与守护

- 先写一个只告警不改序的影子模式？**不要**——直接切换，但要求：
  切换 commit 里必须附带一个测试，用当前生产注册的全部
  ServiceDescriptor 快照（从 `workspace.py` 的实际注册代码提取）验证
  新旧算法产出的启动序在依赖约束下等价（旧序本身满足全部声明依赖时，
  新序的分层结果不得晚于旧序所在层）。
- 若发现现有 descriptor 有"实际依赖了但没声明"的情况（新算法把它排早
  导致启动失败），**补声明**，不要调 priority 掩盖。每处补声明单独列在
  PR 描述里。

### 验收标准（P0-B）

- 单测：缺失依赖、循环依赖、optional 依赖失败、reused 服务参与拓扑、
  priority tiebreak、stop 逆序，各一个用例。
- 真实启动冒烟：`QWENPAW_WORKING_DIR=<临时目录> uv run qwenpaw app`
  正常起服务，日志无新增 ERROR/WARNING。
- 现有全部单测通过。

## 4. 后续阶段（本 RFC 不实施，仅锚定方向）

- **P1 Tool Runtime**：ToolRegistry/GuardedFunctionTool/Governance/审计
  收敛为一条管线；`tool/result` 学 Harness 分离 model-facing canonical
  JSON 与 UI presentation meta。
- **P1.5 持久化下移**：仿 Harness checkpoint-policy——热路径不 await
  落盘，耐久性由独立策略组件负责。
- **P2 观察者式审计事件日志**：append-only、只写不读，先兑现
  replay/调试收益；"日志即真相源"永不作为默认目标。
- **P2 三 Scope 收敛**：App/Workspace/Request；届时 PluginRegistry
  单例退役。
- **P3 Mode → AgentPreset**：声明式组合，配置糖。

## 5. 工程纪律

- 每个 P0 子项独立 commit，行为等价重构与新增测试分开 commit。
- 不引入新依赖。
- 不顺手重构无关代码（发现问题记录到 PR 描述，不动手）。
- Python 3.10+ 语法基线以仓库现有代码为准。
- 测试命令：`uv run pytest tests/unit -q`。
