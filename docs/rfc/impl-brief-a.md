# 任务：实施 Runtime Kernel P0-A（RegistrationHandle + PluginScope）

你是实施工程师。架构方案是 `docs/rfc/runtime-kernel.md`（r2，已吸收你的
审查意见 `runtime-kernel-review-r1.md`），本次只做 **§2 P0-A**。
P0-B（ServiceManager 依赖图）不在本次范围，一行都不要动
`service_manager.py` 的启动算法。

## 交付物（按 commit 顺序）

1. **commit 1 — 基础设施**：`src/qwenpaw/runtime/registration.py`
   （`RegistrationHandle` / `Scope`，按 RFC §2.2 的 API 签名），
   纯新增 + 该模块的单测（幂等、逆序、child scope、async/sync 混排、
   sync 关闭含 async handle 抛错、disposer 异常聚合不中断）。
2. **commit 2 — PluginRegistry 各 register_* 返回 handle**：
   disposer 按对象身份精确撤销；HTTP route disposer 按捕获的 route
   集合删除 + 清 prefix 所有权 + `app.openapi_schema = None`；
   HookRegistry disposer 清 `_sorted_cache`。此 commit 里
   `unregister_plugin()` 保持原逐表过滤实现不动（还没人消费 handle）。
3. **commit 3 — PluginApi 持有 PluginScope**：构造时创建，所有 API
   方法内部 `scope.add()`；startup / workspace-created 回调内经 api
   的注册自动入同一 scope；新增 `api.add_disposer(fn, *, tag)`。
4. **commit 4 — 二级注册 handle 化**：`_post_load_setup` 的
   provider→ProviderManager、control command→两个全局 registry、
   tool→治理表/模块属性/workspace ToolRegistry/bootstrap 每步产生
   handle 入 scope；`_cleanup_plugin_tools` 逻辑拆为对应 disposer
   （行为以现函数为基准逐项对齐）后退役。
5. **commit 5 — WorkspacePlugins 入口补齐**：`register_stop_handler()`、
   可撤销 fallback、事务式 `register_mode()`（child scope 聚合子注册，
   失败逆序回滚）。built-in bootstrap 的 handle 归 workspace，不入
   插件 scope。
6. **commit 6 — 卸载编排切换**：loader 卸载走 `await scope.aclose()`；
   `unregister_plugin()` 变兼容薄壳；删除逐表过滤与
   `_cleanup_plugin_tools` 旧路径。
7. **commit 7 — 运行时级等价测试**：RFC §2.3 主断言（测试插件覆盖
   全部注册面，load→unload 后与从未加载逐项相等，含访问过
   `/openapi.json` 的场景）+ 双插件隔离 + mode setup 中途失败无残留。

## 硬性约束

- 每个 commit 后 `uv run pytest tests/unit -q` 必须全绿再进下一步。
- 不改任何用户可见行为；不动 service_manager 启动算法；不引新依赖。
- RFC §2.3 的 Backlog 项（PawApp 卸载路径、force reinstall、live
  channel 关停、仓内插件 monkey-patch 迁移）**不做**，遇到相关代码
  绕开并在最终报告里记录。
- 卸载语义以现有 `unregister_plugin` + `_cleanup_plugin_tools` 行为
  为基准：先读懂再拆解，不确定处以"与现状一致"为准，不自行增强。
- 禁止过度验证循环：每个 commit 验证一轮测试即可，不要反复跑同一
  套测试或引入额外验证脚本。
- **抓本质，拒绝完美主义**：本次的本质 = 注册所有权可逆（load→unload
  干净）。途中发现的小毛病（命名、日志措辞、无关的潜在 bug、理论上的
  边角 case）一律不就地修，记录到报告延后处理。如果某个 commit 的完整
  实现被一个边角问题卡住超过一次尝试，采取"与现状一致"的保守实现并
  记录，继续推进。

## 完成报告

写入 `docs/rfc/impl-a-report.md`：每 commit 一段（做了什么、测试
结果数字）、行为对齐存疑点清单、Backlog 触碰记录。
