# Runtime Kernel RFC 对抗审查（R1）

结论：方向正确，但当前稿不宜直接进入实现。P0-A 没有定义“返回的 handle 如何自动归属到插件、以及延迟回调产生的二级 effect 如何继续归属”，P0-B 的 optional 传播规则又无法表达生产 descriptor 的现有降级语义；按现稿实现会得到“主注册表干净、真实运行时仍泄漏”以及可选服务故障升级为 workspace 启动失败两类问题。

现状核对结论：`PluginRegistry` 确为 Singleton，`unregister_plugin()` 也确实按 `plugin_id` 逐表过滤；`ServiceManager.start_all()` 确实完全不读 `dependencies`，`stop_all()` 当前是反向 priority、同组并发停止。RFC 对 `WorkspacePlugins` 的描述略有失真：它是四个 registry、一个 `register_mode()` 和一个可直接 `append` 的 `stop_handlers` 列表，并非六个面都有 `register()`。生产 `workspace.py` 注册了 8 个 descriptor，当前没有任何一个声明 `dependencies`。

专项结论：

- a. 注册点盘点：当前 PluginScope 方案只能覆盖 PluginRegistry 的一级登记，覆盖不到延迟 workspace 注册、provider/control/tool 二级注册、live channel、后台任务、patch、synthetic module、`atexit` 等 effect。
- b. 依赖图风险：8 个生产 descriptor 均未声明依赖；至少存在 channel runtime readiness、cron、agent config watcher、driver config watcher 四组由 priority 承担的实际顺序约束，详见 P1-4。
- c. reused_services：当前仅 reuse `memory_manager`/`chat_manager`，二者都没有 `reload_func` 或对本轮重建服务的反向依赖，故生产现状没有该反例；但通用算法在 `D(rebuild) -> R(reuse) -> C` 上确有反例，详见 P1-5。
- d. stop_all：当前按 reverse priority 分组、组内并发；watcher→manager、cron→channel/runtime、channel→core 的停止顺序依赖现有 priority，且运行中的 cron task 不会被 stop 等待，详见 P1-6。
- e. 异步 disposer：shutdown/uninstall hook、channel stop、A2A close、PawApp task cancel 都是 async；RFC 的 sync `RegistrationHandle.dispose()` 加 `close()/aclose()` 双入口契约不足，详见 P1-3。

## P0（方案性错误，不改会返工）

### P0-1：handle“返回了”不等于已进入 PluginScope，延迟注册尤其无法被 loader 捕获

- 证据：`docs/rfc/runtime-kernel.md:74-80` 只要求 `register_*` 返回 handle，并让 loader 把“加载路径”注册挂入 scope；`src/qwenpaw/plugins/loader.py:559-566` 只把 `PluginApi` 交给 `plugin.register()`；真正执行 startup hook 是更晚的 `src/qwenpaw/app/_app.py:486-501` 和 `src/qwenpaw/app/routers/plugins.py:212-219`；这些 hook 又在 `src/qwenpaw/plugins/api.py:937-951, 1015-1032, 1068-1085` 中向既有 workspace 继续注册。
- 问题：现有调用方按 RFC 的兼容承诺继续忽略返回值时，loader 看不到延迟 callback 内创建的 handle，插件卸载后 slash command、mode、runtime hook 和 stop handler 仍留在已存在的 workspace。
- 建议修正：让 `PluginApi` 构造时显式持有唯一 `PluginScope`，所有 API 方法内部自动 `scope.add()`，并让 startup/workspace-created callback 产生的 workspace handle 也回挂同一 scope，而不是要求插件调用方接返回值。

### P0-2：PluginRegistry 只是一级索引，provider/control/tool/channel 的真实运行时注册在 scope 之外

- 证据：provider 会二次写入 `ProviderManager`（`src/qwenpaw/app/_app.py:436-446`、`src/qwenpaw/app/routers/plugins.py:166-183`）；control command 会二次写入全局 handler 与 channel priority registry（`src/qwenpaw/app/_app.py:454-470`、`src/qwenpaw/app/routers/plugins.py:189-203`）；tool 还会写治理表、`qwenpaw.agents.tools`、各 workspace `ToolRegistry` 和 bootstrap 列表（`src/qwenpaw/plugins/api.py:821-863, 180-209`），当前卸载为此另有专门清理（`src/qwenpaw/plugins/loader.py:1304-1309, 1325-1414`）。
- 问题：把 `PluginRegistry.unregister_plugin()` 简化为 `plugin_scope.close()`，若 scope 只收 registry 的 handle，会出现 registry 已空但 ProviderManager、两个 command registry、工具模块/治理表和 live workspace 仍持有插件对象的假卸载。
- 建议修正：把 `_post_load_setup` 产生的二级注册也做成 handle 并加入同一 PluginScope，明确保留或迁移 `_cleanup_plugin_tools`，同时让所有卸载入口只走一条能等待 workspace reload/连接关闭的编排路径。

### P0-3：optional 依赖传播规则无法保持当前“可选能力失败、workspace 继续启动”的行为

- 证据：`driver_manager` 是 optional（`src/qwenpaw/app/workspace/workspace.py:350-359`），而 `driver_config_watcher` 明确读取它并在缺失时返回 `None`（`src/qwenpaw/app/workspace/service_factories.py:71-96`），但 watcher 自身不是 optional（`src/qwenpaw/app/workspace/workspace.py:425-435`）；RFC 却规定 optional 依赖失败时，必选依赖方必须报错（`docs/rfc/runtime-kernel.md:114-117`）。类似地，`memory_manager` 可选（`src/qwenpaw/app/workspace/workspace.py:328-347`），cron 的 dream callback 条件性使用它（`src/qwenpaw/app/crons/manager.py:684-700`）。
- 问题：如实补上 `driver_config_watcher -> driver_manager` 或 `cron_manager -> memory_manager` 后，现稿规则会把今天可容忍的可选后端故障升级成整个 workspace 启动失败，违反“行为等价”。
- 建议修正：把边区分为 required、optional/order-only（或为依赖项而非依赖方声明 failure policy），并明确 `None` 服务、optional 启动失败、条件性功能依赖三种状态的不同传播语义。

### P0-4：Scope 不是副作用沙箱，RFC 的“每个注册可逆”边界没有覆盖现有插件的非 API effect

- 证据：QwenPaw Pet 在 `register()` 中修改 `sys.path` 并注册 `atexit`（`plugins/bundle/qwenpaw-pet/plugin.py:12-17, 74-100`），startup 后还改类方法和 live channel 回调（`plugins/bundle/qwenpaw-pet/patch_runner.py:121-194, 282-310`）；CloudPaw 注入不位于插件目录命名空间的 synthetic module（`plugins/bundle/cloudpaw/injectors.py:12-59`），并 monkey-patch `PluginLoader.unload_plugin`（`plugins/bundle/cloudpaw/plugin.py:425-468`）及多个运行时方法（`plugins/bundle/cloudpaw/hooks.py:403-487, 494-559`），其 shutdown 只关闭 A2A client（`plugins/bundle/cloudpaw/plugin.py:587-596`）。
- 问题：这些 effect 不经过任何 `register_*`，当前 loader 的 `sys.modules` 路径清扫（`src/qwenpaw/plugins/loader.py:1267-1302`）也清不到 synthetic module、`atexit` 和未恢复的 monkey-patch，所以“load→unload 与从未加载相等”按现方案不可实现。
- 建议修正：明确保证边界为“host API 创建的 effect”，新增插件可主动登记 sync/async disposer 的 API，并把仓内插件的 atexit、synthetic module、patch、后台资源逐项迁入该 API 或显式列为已知兼容清理。

## P1（重要缺口）

### P1-1：WorkspacePlugins 的 stop_handlers、fallback 和 mode 复合注册没有可生成精确 handle 的入口

- 证据：`WorkspacePlugins` 只有四个 registry 字段、`modes`/`stop_handlers` 两个列表和 `register_mode()`（`src/qwenpaw/app/workspace/workspace_plugins.py:32-57`）；fallback 单独由 `SlashCommandRegistry.register_fallback()` 写入（`src/qwenpaw/runtime/slash_command_registry.py:76-81`）；插件 stop handler 直接 append（`src/qwenpaw/plugins/api.py:1289-1294`），mode setup 还会扇出四个 registry（`src/qwenpaw/modes/base.py:40-54`）并可能继续 append stop handler（例如 `src/qwenpaw/modes/goal/goal_mode.py:222-241`）。
- 问题：仅让现有 `register()` 返回 handle 无法覆盖 stop handler/fallback，且 `register_mode()` 先 append mode 再执行多步 setup，任一步失败或卸载都可能留下半注册 mode。
- 建议修正：补 `register_stop_handler()`、可撤销的 fallback 注册和复合/事务式 `register_mode()`，mode handle 应聚合 setup 产生的全部子 handle 并在失败时立即逆序回滚。

### P1-2：HTTP route 的 disposer 还必须维护 FastAPI 的外部派生状态

- 证据：注册时捕获 FastAPI 实际新增的 route 对象并清空 OpenAPI cache（`src/qwenpaw/plugins/registry.py:29-52, 272-287`）；现有卸载按 route 身份 remove，但只更新 prefix/registration 表，没有再次设置 `app.openapi_schema = None`（`src/qwenpaw/plugins/registry.py:298-326`）。
- 问题：即使 registry 表恢复，若 `/openapi.json` 已被访问，卸载后 schema 仍会暴露插件 endpoint，验收中的“逐表相等”检测不到这一残留。
- 建议修正：HTTP handle 应以本次返回的 route 对象集合精确删除、做 prefix 的 ABA 身份保护并同步失效 OpenAPI cache，新增先访问 schema 再卸载的断言。

### P1-3：同步 RegistrationHandle 与异步 Scope 的契约自相矛盾

- 证据：RFC 把 disposer 类型固定为 `Callable[[], None]`，同时又声称 `aclose()` 允许异步 disposer（`docs/rfc/runtime-kernel.md:55-69`），并要求同步 `PluginRegistry.unregister_plugin()` 调 `close()`（`docs/rfc/runtime-kernel.md:78-80`）；实际资源清理包含 async shutdown hook（`src/qwenpaw/plugins/loader.py:1225-1265`）、PawApp 后台任务 cancel+await（`plugins/apps/agent-kanban/backend/main.py:1273-1309`）、A2A `AsyncClient.aclose()`（`plugins/bundle/cloudpaw/modules/a2a/client_manager.py:198-204`）和插件 channel 的 async `stop()`（`plugins/channel/azure_bot/channel.py:330-380`），而 HTTP route 移除本身其实是同步的（`src/qwenpaw/plugins/registry.py:298-326`）。
- 问题：现稿没有说明 handle 如何承载 awaitable、`close()` 遇到 async handle 怎么办、同一 scope 能否先 `close()` 后 `aclose()`，实现者会被迫发明互不兼容的语义。
- 建议修正：定义统一的 async dispose 协议并以 `await scope.aclose()` 作为 loader 唯一卸载原语；同步兼容入口只能处理纯同步登记或明确返回/抛出“需要异步卸载”，不得静默丢弃 coroutine。

### P1-4：生产依赖快照目前没有一条边，RFC 的迁移测试会空过

- 证据：8 个生产 descriptor 位于 `src/qwenpaw/app/workspace/workspace.py:302-435`，没有任何 `dependencies=`；`start_all()` 只按 priority 分组（`src/qwenpaw/app/workspace/service_manager.py:163-210`），从不读取 descriptor 的 `dependencies`。
- 问题：以“旧序满足全部已声明依赖”为判据时，空图天然通过，既不能证明真实隐性依赖已迁移，也不能防止后来漏声明。
- 建议修正：验收测试应断言生产 descriptor 的明确 edge 集合和预期启动批次，而不是只比较算法；本轮至少把下述依赖盘点逐项定性并编码。

生产 descriptor 的实际依赖盘点：

| 服务 | 当前声明 | 实际依赖/排序约束 | 证据与结论 |
|---|---|---|---|
| `local_workspace` | 无 | 无其他 service | `workspace.py:295-311` 只返回预建对象。 |
| `session` | 无 | 无其他 service | `workspace.py:314-325` 只依赖路径。 |
| `memory_manager` | 无 | 无其他 service；自身 optional | `workspace.py:328-347` 只读 config/workdir。 |
| `driver_manager` | 无 | 无其他 service；自身 optional | `service_factories.py:19-67` 自建并启动 manager。 |
| `chat_manager` | 无 | 无其他 service | `service_factories.py:100-126` 自建 repo，reuse 也不重绑别的服务。 |
| `channel_manager` | 无 | 启动前必须至少有 `local_workspace`、`session`、`chat_manager`；`memory_manager`/`driver_manager` 是可缺失的运行时能力 | factory 把 `ws.stream_query` 暴露给 channel（`service_factories.py:144-176`），`start_all()` 又 fire-and-forget 建立外部连接（`channels/manager.py:467-505`）；channel 请求会走 workspace runtime，且 channel 基类直接用 `chat_manager`（`channels/base.py:542-558`）。当前 priority 5/10/20→30 正在承担这组隐性 readiness 约束。 |
| `cron_manager` | 无 | `channel_manager`、`chat_manager` 和 workspace runtime 是实际依赖；`memory_manager` 是 dream 功能的条件依赖 | init_args 直接取 channel（`workspace.py:386-409`）；scheduler 在 start 时即可发布任务（`crons/manager.py:93-178`）；executor 使用 channel、chat 和 `workspace.stream_query`（`crons/executor.py:51-67, 122-180`），dream 使用 memory（`crons/manager.py:684-700`）。 |
| `agent_config_watcher` | 无 | 在 `channel_manager`/`cron_manager` 就绪后判断是否创建（逻辑为 OR，不是普通 AND 依赖） | `service_factories.py:189-226` 同时读取二者并以 `channel_mgr or cron_mgr` 作门控；priority 50 当前隐式保证顺序。 |
| `driver_config_watcher` | 无 | `driver_manager`，但依赖缺失应当安静跳过 | `service_factories.py:71-96` 直接持有 manager/card store；priority 51 当前也保证它先于 driver manager 停止。 |

### P1-5：reused service 在依赖图运行前就执行 reload_func，“视为已满足”会破坏传递依赖

- 证据：`set_reusable()` 立即写入 `services/reused_services` 并调用 `reload_func`（`src/qwenpaw/app/workspace/service_manager.py:111-149`），而真正的 `start_all()` 更晚才运行（`src/qwenpaw/app/workspace/workspace.py:438-510`）；当前两个 reusable descriptor 是 `memory_manager`、`chat_manager`（`workspace.py:328-370`），两者都没有 `reload_func`，所以当前生产图没有“reuse 服务依赖本轮重建服务”的反例。
- 问题：接口层面仍存在确定反例：若 reused `R` 依赖重建 `D`，`reload_func(R)` 会在 `D` 创建前运行，而若 `R` 被预先视为 satisfied，依赖 `R` 的 `C` 也可能绕过 `D` 提前启动。
- 建议修正：`set_reusable()` 只登记候选实例，把 `reload_func` 放到 `R` 所在拓扑节点、等 `R.dependencies` 满足后执行，并新增 `D(rebuild) -> R(reuse) -> C` 的传递依赖测试。

### P1-6：stop_all 不能只“反转计划”，必须按实际成功启动集合回滚，并处理仍在运行的 cron/channel 任务

- 证据：现状按反向 priority、同组并发 stop（`src/qwenpaw/app/workspace/service_manager.py:382-418`）；workspace 启动失败时调用 `stop()`（`src/qwenpaw/app/workspace/workspace.py:515-521`），但 `_started` 仍为 false，`stop()` 直接返回（`workspace.py:568-577`）；CronManager `stop()` 用 `scheduler.shutdown(wait=False)` 且只等待 keepalive（`src/qwenpaw/app/crons/manager.py:180-198`），手工 cron task 也未保存以供 stop cancel（`crons/manager.py:371-397`），执行中仍会访问 channel/chat/workspace（`crons/executor.py:122-180`）。
- 问题：拓扑启动中途失败会泄漏已启动服务，而正常 stop 即使先停 cron，也可能在随后停 channel/core 后仍有 cron 执行继续访问它们，所以“精确逆序”本身不足以保证 shutdown 安全。
- 建议修正：ServiceManager 记录本轮成功启动/接管的节点并在失败时直接逆拓扑回滚；CronManager 必须跟踪并 cancel/await 所有执行任务，之后才允许 channel 和 core service 停止。

### P1-7：reload 失败时 reused 实例的所有权没有进入验收范围

- 证据：reload 先从旧 workspace 取 reusable 实例并注入新 workspace（`src/qwenpaw/app/multi_agent_manager.py:434-452`），新 workspace 启动失败后调用默认 `final=True` 的 `new_instance.stop()`（`multi_agent_manager.py:455-463`）；ServiceManager 的 final stop 会停止 reused 实例（`src/qwenpaw/app/workspace/service_manager.py:433-449`），只是当前又被 `_started == False` 的早退偶然挡住。
- 问题：一旦为 P1-6 修复部分启动回滚，若不同时定义 ownership，失败的新 workspace 可能关闭仍由旧 workspace 服务请求的 memory/chat 实例。
- 建议修正：把 reused 节点标为 borrowed，失败回滚永不停止 borrowed 实例，只有原 owner 的最终 shutdown 或成功原子移交后才转移停止责任，并覆盖“新实例启动失败、旧实例继续服务”的测试。

### P1-8：现有验收只比较主表，无法证明“运行时与从未加载相等”

- 证据：P0-A 只要求逐注册表相等、双插件隔离和 disposer 异常（`docs/rfc/runtime-kernel.md:88-94`）；但卸载还涉及 HTTP app route/OpenAPI、provider/control 二级 registry、工具 module/governance/bootstrap、workspace 六个面、插件后台任务和 live channel（见 P0-2、P1-1 至 P1-3）。
- 问题：按当前标准，即使所有用户可见 endpoint、命令、工具、mode 或后台连接仍在，测试仍可能全绿。
- 建议修正：至少补齐 failed-register 回滚、load 失败、卸载前/后 startup hook、force reinstall、PawApp 直接卸载、route+OpenAPI、provider/control/tool 二级状态、既有/新建 workspace、mode 四面+stop handler、async disposer 混排及 live channel/task 终止用例。

### P1-9：依赖图验收缺少“启动前预检”和 optional/并发失败后的确定状态

- 证据：RFC 说缺失依赖和环在首次 `start_all()` 报错（`docs/rfc/runtime-kernel.md:114-120`），但没有要求错误必须发生在任何 service 构造之前；当前同组通过 `asyncio.gather()` 启动（`src/qwenpaw/app/workspace/service_manager.py:195-207`），optional 失败只 pop service 且不返回结构化状态（`service_manager.py:248-255`）。
- 问题：若边校验边启动或沿用无状态的 `_start_service()`，缺失/环可能在部分副作用发生后才暴露，同层一个 required 节点失败时其他协程的完成/取消和后继 skip 也不可验证。
- 建议修正：先纯计算并完整校验图再启动，节点结果显式记录为 started/reused/skipped-optional/failed，并为并发 sibling 失败、传递 optional skip、stop 仅作用于成功节点分别加测试。

## P2（建议）

### P2-1：“启动序精确逆序”在并发层内没有唯一含义

- 证据：RFC 同时保留层内 `concurrent_init`（`docs/rfc/runtime-kernel.md:110-113`）又要求精确逆序（`runtime-kernel.md:119`），当前同 priority descriptor 是并发 start/stop（`src/qwenpaw/app/workspace/service_manager.py:195-207, 397-410`）。
- 问题：并发节点只有偏序，没有可靠的单一“启动序”；若按实际完成时间反转，stop 顺序还会随 I/O 时序漂移。
- 建议修正：把规范改成“逆拓扑层停止，层间严格 await，层内无依赖节点可并发”，另用稳定注册序仅做日志和测试的确定性 tie-break。

### P2-2：真实冒烟的“无新增 WARNING”没有可复现基线

- 证据：RFC 只写“日志无新增 ERROR/WARNING”（`docs/rfc/runtime-kernel.md:137-138`），而 optional 服务失败本来就按设计记录 WARNING（`src/qwenpaw/app/workspace/service_manager.py:248-255`），插件和未配置可选能力也有大量环境相关 warning 路径。
- 问题：没有基线日志、固定配置和允许列表时，该标准既可能因机器环境误报，也可能被人工解释为通过，无法做稳定门禁。
- 建议修正：提供固定最小 agent 配置与可比较的结构化 service 状态/启动批次断言，日志只对新增的指定 error code 或 exception 做 allowlist 检查。

## 注册点盘点表

“可覆盖”指按 RFC 当前文字，仅靠 `register_*` 返回 handle、loader 建一个 PluginScope 是否足以在卸载时精确恢复；“需补编排”表示底层可以返回 handle，但必须由 `PluginApi`/loader 自动收集并处理二级 effect。

| 调用点 | PluginScope 能否覆盖 | 缺口说明 |
|---|---|---|
| `loader.py:561` → `PluginRegistry.register_plugin_manifest` | 需补编排 | manifest 是第 11 个实际注册项；loader 可直接收 handle，但 RFC 的“约 10 张表”与验收清单未明确它和辅助索引。 |
| `plugins/api.py:380,414,446,489,524,560,580,615,699,1139` → PluginRegistry 各注册面 | 需补编排 | `PluginApi` 必须内部自动 `scope.add()`；仅改变返回值时现有调用方全部忽略 handle。 |
| `registry.py:220-287` → FastAPI `include_router` | 部分 | 已捕获新增 route 身份，但 disposer 还要清 prefix 所有权并失效 OpenAPI cache；这不是 async 操作。 |
| `_app.py:436-470`、`routers/plugins.py:166-203` → ProviderManager + 两个 control command registry | 否 | 它们发生在 plugin.register 结束之后且不在 PluginRegistry 内，必须返回二级 handle 并加入同一 scope；PawApp 的 `routers/pawapps.py:192-202` 直接 unload 目前也绕过 `_post_unload_cleanup`。 |
| `plugins/api.py:811-894` → tool startup hook | 否 | hook handle 只能撤销“将来执行”的 callback，不能撤销已执行后写入的治理 owner、module attr、`__all__`、workspace ToolRegistry、bootstrap funcs 和 agent config。 |
| `plugins/api.py:937-951,1195-1219` → workspace slash registry | 否 | startup/workspace-created callback 内的返回 handle 被丢弃；aliases 必须由一个 handle 原子撤销。 |
| `plugins/api.py:976-993,1221-1245` → `WorkspacePlugins.register_mode` | 否 | mode 又在 `modes/base.py:47-54` 扇出 slash/tool/hook/prompt，并可能 append stop handler；需要复合事务 handle。 |
| `plugins/api.py:1015-1032,1247-1271` → workspace HookRegistry | 否 | 延迟 callback 直接注册且忽略 handle；HookRegistry disposer 还需清 `_sorted_cache`。 |
| `plugins/api.py:1068-1085,1289-1294` → `stop_handlers.append` | 否 | 没有 register 方法，无法返回 handle；应改成按 registration 对象身份撤销。 |
| `plugins/api.py:1368-1385` → skill provider 三类 hook | 部分 | hook 本身可收 handle，但已复制的 skill 目录和 manifest 变更只能靠 uninstall callback；scope.close 与“执行 uninstall effect”必须明确区分。 |
| `pawapp/app.py:237-312` → route/tool/lifecycle 转接 | 部分 | 最终都走 PluginApi，自动收集后可覆盖一级注册；但 `router.dependencies.append()`（`:241-256`）也是原地 mutation，当前无 disposer。 |
| `workspace.py:154-232` → built-in tool/prompt/hook/slash/fallback/mode bootstrap | 不应纳入插件 scope | 这些是 workspace 自有 bootstrap，生命周期随 workspace；底层仍应返回 handle，但不能错误归属某个插件。 |
| `modes/base.py:47-54`、`modes/*` 的 `stop_handlers.append` | 仅插件 mode 需覆盖 | built-in/custom mode 随 workspace；由插件 API 注入的 mode 必须把所有子注册挂到插件的复合 handle。 |
| `plugins/channel/azure_bot/plugin.py:24` → channel class；实例由 `channels/manager.py:120-221,467-505` 创建/连接 | 否 | registry handle 只能移除 class，不能停止已经实例化的 async channel；当前靠 fire-and-forget workspace reload 间接清理，卸载完成语义不精确。 |
| `plugins/apps/agent-kanban/backend/main.py:1241-1309` → 后台 dispatcher/persist task | 否 | 资源由 async lifecycle hook创建；需要 async disposer/terminate hook被 scope 卸载编排 await，且在途 `_RUNNING` task/SSE channel 也应有明确策略。 |
| `pawapp/task.py:114-160,198-207` → 全局 TaskManager 与 untracked task | 否 | `asyncio.create_task()` 返回值未保存到 plugin owner，PawApp 卸载无法按 app_id cancel/await 在途 task。 |
| `qwenpaw-pet/plugin.py:12-17,74-100`、`patch_runner.py:121-310` → sys.path/atexit/monkey-patch/live callbacks | 否 | loader 只清 plugin path/modules；需要插件显式登记复原 disposer，`atexit.unregister` 也要覆盖。 |
| `cloudpaw/injectors.py:12-59`、`cloudpaw/plugin.py:425-468`、`cloudpaw/hooks.py:403-559` → synthetic module/loader 与 runtime patch | 否 | synthetic module 不在插件文件路径 sweep 范围，多个 patch 没有统一 restore；必须显式 handle 化。 |
| `cloudpaw/modules/a2a/client_manager.py:198-204,400-416` → A2A 连接 singleton | 部分 | 已有 async shutdown 函数，但只能由 `aclose`/shutdown 编排 await，sync `unregister_plugin().close()` 不足。 |
