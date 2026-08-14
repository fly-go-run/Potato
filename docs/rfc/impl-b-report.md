# Runtime Kernel P0-B 实施报告

## Commit 1 — 依赖边语义与图算法（`ca362654`）

`ServiceDescriptor` 新增 order-only `after`；`ServiceManager` 在创建任何
实例前校验 required 缺边与 required/after 合并环，并用带环路径的
`ServiceDependencyError` 报告。启动改为确定性 Kahn 分层，层内按 priority
分组并保留 `concurrent_init` 与组间让出；停止改为逆拓扑层、层间等待、
层内并发。启动结果以 `ServiceStartResult` 显式记录 `started`、`reused`、
`skipped_optional`、`failed`，required 与 order-only 的失败传播按 RFC
执行。新增缺边、混合边环、两类失败传播、order-only、priority、并发及
逆层停止测试。隔离本机语音凭据后全量单测：5615 passed，8 skipped。

## Commit 2 — reused 与 borrowed（`8186c44a`）

`set_reusable()` 只登记复用实例及 borrowed 所有权；`reload_func` 延迟到
复用节点的拓扑位置，且保持原有 best-effort 异常语义。回滚停止显式排除
borrowed，正常成功后的 final stop 仍可停止已接管实例。新增
`D(rebuild) → R(reuse) → C` 传递顺序与 borrowed 回滚测试。全量单测：
5617 passed，8 skipped。

## Commit 3 — 启动失败回滚（`9bfe9b31`）

`ServiceManager` 跟踪本次实际成功 `started` 且归当前 workspace 所有的
节点；workspace 启动失败时不再被 `_started == False` 提前挡住，而是以
`final=True` 按逆拓扑回滚该集合。这样新建且标记 reusable 的 owned 服务
仍会停止，borrowed 服务不会被误停；回滚后清空集合，避免二次停止。
新增 workspace 中途失败回滚测试。全量单测：5618 passed，8 skipped。

## Commit 4 — 生产依赖声明与图快照（本提交）

按 RFC 补齐四处生产声明：channel 的 local workspace/session/chat required
边，cron 的 channel/chat required 边，agent watcher 的 channel/cron
order-only 边，driver watcher 的 driver order-only 边。快照测试锁定 9 个
生产 descriptor 的注册全集、8 条边全集及 4 层 Kahn 结果；同时将同层
候选固定为注册序，避免邻接遍历顺序影响诊断与测试。

## Commit 5 — CronManager 关停安全（待实施）

待补充派发 task 的跟踪、取消与 await 收尾测试，以及最终全量测试和一次
CLI 冒烟结果。

## 新发现的未声明依赖

无。复核了 service factory 与 CronManager 执行路径：memory/driver 对
channel、memory 对 cron 属运行时可缺失能力，构造与启动不读取这些实例；
因此没有把它们伪装成 required 或 order-only 启动边，也没有调整 priority。

## 存疑点

- `post_init` 返回 `None` 的条件 watcher 仍记为 `started`，与旧实现的
  “安静跳过但启动成功”行为一致；若将来需要区分 absent，可另扩状态，
  本阶段不改变用户可见语义。
- 节点的 start hook 自身抛错时，该节点记 `failed`，回滚集合只含此前明确
  `started` 的节点。若某个 hook 在抛错前自行产生未托管副作用，仍应由
  hook 内部保证失败原子性；本阶段按 RFC 的实际 started 集合保守实现。
- 根目录 `.env` 的真实语音凭据会覆盖三个既有 ASR 单测中的 legacy
  `apikey`；各 commit 全量验收均按 P0-A 相同方式隔离该本机变量。

## Backlog 触碰记录

- 插件注册体系与 P0-A backlog：未修改。
- Tool Runtime、事件日志、三 Scope 收敛：未修改。
- 条件服务的 absent 独立状态、失败 hook 的事务协议：仅记录，未扩展。
- Cron 派发 task 泄漏：留在独立 commit 5 修复。
