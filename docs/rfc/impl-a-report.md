# Runtime Kernel P0-A 实施报告

## Commit 1 — 基础设施（`a00be9a8`）

新增 `RegistrationHandle` / `Scope`，支持幂等释放、逆序关闭、child
scope、同步/异步 disposer 混排、同步关闭预检，以及不中断后续清理的
`ExceptionGroup` 聚合。新增 6 个基础设施测试。隔离本地语音凭据变量后，
`uv run pytest tests/unit -q`：5595 passed，8 skipped。

## Commit 2 — PluginRegistry handle（`cae249bd`）

所有 `PluginRegistry.register_*` 返回按捕获对象身份撤销的 handle；HTTP
路由按捕获 route 集合移除、释放 prefix ownership 并失效 OpenAPI cache；
`HookRegistry` 撤销同步清理排序缓存。此阶段保留原
`unregister_plugin()`。新增 3 个 registry 测试。测试：5598 passed，
8 skipped。

## Commit 3 — PluginApi scope（`19d92e84`）

每个 `PluginApi` 构造唯一 plugin scope，一级注册及延迟 workspace
注册自动入 scope；新增 `add_disposer()`。`PluginRecord` 保存 API；slash、
tool、prompt 等 runtime registry 开始返回身份 handle。新增 1 个延迟注册
所有权测试。测试：5599 passed，8 skipped。

## Commit 4 — 二级注册（`c34d11b8`）

ProviderManager、全局 control handler、command priority registry 均返回
身份 handle；tool ownership、governance、`agents.tools` 属性、workspace
ToolRegistry 和 bootstrap 列表逐步登记到 tool child scope，失败仍逆序
回滚。新增 2 个二级 registry 隔离测试。测试：5601 passed，8 skipped。

## Commit 5 — WorkspacePlugins（`3f94cd61`）

新增可撤销 stop handler/fallback 入口；`register_mode()` 以独立事务 scope
聚合 slash/tool/hook/prompt/stop 注册，setup 失败立即逆序回滚。built-in
bootstrap handle 归 workspace scope；保留 duck-typed mode 容器的原兼容
路径。新增 2 个 workspace 测试。测试：5603 passed，8 skipped。

## Commit 6 — 卸载编排（`63bdd6ee`）

loader 卸载与失败加载统一等待 plugin scope；`unregister_plugin()` 改为
仅释放直接 registry 消费者 handle 的同步兼容薄壳。删除逐表过滤、
`_cleanup_plugin_tools` 及正常卸载后的 provider/command 二次清理。新增
1 个 async scope 卸载测试。测试：5604 passed，8 skipped。

## Commit 7 — 运行时等价验收

新增完整注册面 load→unload 等价测试（包含已生成 OpenAPI schema、延迟
startup/workspace-created 注册）、双插件隔离测试，以及 mode setup 中途
失败无残留测试。`uv run pytest tests/unit -q`：5607 passed，8 skipped。

## 行为对齐存疑点

- 工具配置文件写入在旧 `_cleanup_plugin_tools` 中不会撤销，本实现继续
  保留该持久化行为，只撤销旧路径实际清理的内存/runtime effect。
- loader 对 disposer 聚合异常记录完整错误后继续完成 module/record/file
  卸载，与旧清理失败不阻断卸载的行为一致。
- 直接构造 `PluginRecord(api=None)` 的旧测试/调用由 handle-tracked legacy
  tool bridge 兼容；正常插件加载不走该分支。
- mode 对非 `WorkspacePlugins` 的 duck-typed 容器保留原直接注册方式；
  事务保证适用于正式 workspace 入口。
- force-reinstall 专用的旧 provider/command 回调仍保留，未扩展或验证其
  全矩阵；正常卸载已完全由 scope 负责。

## Backlog 触碰记录

- PawApp 独立卸载路径：未修改。
- force reinstall 全矩阵：只识别并绕开其专用编排，未新增行为或测试。
- live channel 实例关停：未修改；仍依赖既有 workspace reload 行为。
- 仓内插件 monkey-patch / synthetic module 迁移：未修改；插件可后续使用
  `api.add_disposer()` 显式接入。
- `service_manager.py` 启动算法：未修改。
