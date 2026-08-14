# 任务：实施 Runtime Kernel P0-B（ServiceManager 依赖图）

你是实施工程师。方案是 `docs/rfc/runtime-kernel.md` r2 的 **§3**（P0-B），
P0-A 已完成并验收（见 `impl-a-report.md`）。本次只动服务启动/停止，
不碰插件注册体系。

## 交付物（按 commit 顺序）

1. **commit 1 — 依赖边语义 + 图算法**（RFC §3.2/§3.3）：
   `ServiceDescriptor` 增加 `after: List[str]`（order-only 边）；
   `start_all()` 改为：启动前纯计算图校验（缺 required 边、环——两种边
   合并成图，报错给出环路径）→ Kahn 分层拓扑 → 层内按 priority 分组、
   保留 `concurrent_init` 并发与组间 `asyncio.sleep(0)`。
   节点结果显式记录 `started | reused | skipped_optional | failed`，
   `start_all()` 返回结构化状态表。失败传播按 RFC §3.2 规则。
   `stop_all()` 改逆拓扑层、层间 await、层内并发。
   本 commit 附全套算法单测（RFC §3.5 列举的用例）。
2. **commit 2 — reused 语义**：`set_reusable()` 只登记实例，
   `reload_func` 延迟到该节点拓扑位置执行；borrowed 标记——启动失败
   回滚永不 stop borrowed 实例。附 `D(rebuild)→R(reuse)→C` 传递依赖
   测试与 borrowed 回滚测试。
3. **commit 3 — 启动失败回滚**：workspace 启动中途失败时按实际
   started 集合逆拓扑回滚（修 workspace.py `_started` 早退泄漏；
   注意 stop() 的 final 语义与 borrowed 排除）。
4. **commit 4 — 生产依赖补声明 + 图快照测试**（RFC §3.1 盘点）：
   - `channel_manager`: dependencies=[local_workspace, session,
     chat_manager]
   - `cron_manager`: dependencies=[channel_manager, chat_manager]
   - `agent_config_watcher`: after=[channel_manager, cron_manager]
   - `driver_config_watcher`: after=[driver_manager]
   快照测试断言 workspace.py 注册全集的期望边集合与期望启动分层
   （防空图空过）。若实施中发现盘点遗漏的真实依赖：补声明并在报告
   里单独列出，不许调 priority 掩盖。
5. **commit 5 — 关停安全（独立真 bug 修复）**：CronManager 跟踪派发的
   执行 task，`stop()` cancel 并 await 收尾；确保其排在 channel/core
   停止之前（拓扑已保证，验证即可）。附测试。

## 硬性约束

- 每 commit 后 `uv run pytest tests/unit -q` 全绿再前进。
- 除 §3.4 关停安全外全部行为等价：现有服务的实际启动顺序在新算法下
  必须仍满足全部约束；用户可见行为不变。
- 不改插件注册体系（P0-A 产物）；不引新依赖。
- 冒烟验收：`QWENPAW_WORKING_DIR=$(mktemp -d) uv run qwenpaw app` 启动，
  断言状态表全为 started|reused 后干净退出（可写成脚本化检查，跑通
  一次即可，不要反复）。
- **抓本质，拒绝完美主义**：本质 = 依赖显式化 + 启动确定性 + 关停不漏。
  小毛病（命名、日志、理论边角）记录到报告延后。被边角卡住超过一次
  尝试就取"与现状一致"的保守实现并记录，继续推进。

## 完成报告

写入 `docs/rfc/impl-b-report.md`：每 commit 一段、新发现的未声明依赖
清单、存疑点、Backlog 触碰记录。
