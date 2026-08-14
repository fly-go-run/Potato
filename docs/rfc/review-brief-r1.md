# 任务：对抗审查 RFC《Runtime Kernel》（只审查，不写代码）

你是本项目的资深审查者。架构师（Claude）写了一份重构 RFC：
`docs/rfc/runtime-kernel.md`。你的任务是**挑战这份方案、查漏补缺**，
产出审查报告。**不要修改任何代码或 RFC 本身。**

## 步骤

1. 通读 RFC。
2. 逐一核对 RFC 引用的现状描述是否属实，重点文件：
   - `src/qwenpaw/plugins/registry.py`（Singleton、unregister_plugin 逐表过滤）
   - `src/qwenpaw/plugins/loader.py`（插件 load/unload 路径，找出 RFC 没提到的注册点和清理点）
   - `src/qwenpaw/app/workspace/service_manager.py`（start_all 只按 priority）
   - `src/qwenpaw/app/workspace/workspace.py`（真实的 ServiceDescriptor 注册全集：
     谁声明了 dependencies、谁靠 priority 隐性排序）
   - `src/qwenpaw/app/workspace/workspace_plugins.py` 及其六个注册面
     （hooks.py / tool_registry.py / slash_command_registry.py / prompt_manager.py）
3. 专项排查（这是重点，逐项给结论）：
   a. **注册点盘点**：grep 全仓库对 PluginRegistry 各 register_* 与
      WorkspacePlugins 各注册面的调用点，列出 RFC 的 PluginScope 方案
      **覆盖不到**的注册/副作用（如：直接 mount 到 FastAPI 的路由如何
      精确卸载、channel 连接、后台任务、信号量、模块级 import 副作用）。
   b. **依赖图风险**：现有 descriptor 里有没有"实际依赖但未声明"的服务
      （读 init_args/post_init 里对 workspace.xxx 的访问即可推断）？列出
      每一个，因为拓扑切换后它们可能提前启动而炸。
   c. **reused_services 与拓扑的交互**：reload 场景下被 reuse 的服务
      不重新 start，RFC 说"视为已满足"——有没有反例（reuse 的服务依赖
      了一个本轮重建的服务）？
   d. **stop_all 现状**：现在是怎么停的？RFC 说改精确逆序，有没有服务
      的 stop 隐性依赖别的服务还活着？
   e. **异步 disposer**：哪些清理是 async 的？Scope.close()/aclose() 的
      切分是否够用？
4. 对 RFC 的验收标准挑刺：缺哪些必要用例？哪些标准无法验证？

## 输出

写入 `docs/rfc/runtime-kernel-review-r1.md`，格式：

- P0（方案性错误，不改会返工）/ P1（重要缺口）/ P2（建议）分级；
- 每条给：文件:行号证据、问题一句话、建议修正一句话；
- 最后一节"注册点盘点表"：调用点 → PluginScope 能否覆盖 → 缺口说明。

## 纪律

- RFC 第 0 节列了明确非目标，不要挑战它们（不迁 TS/Cordis、本阶段
  不收敛两层注册中心、不做真相源事件日志）。
- 不追求完美主义：只报会造成返工或 bug 的问题，风格与理论洁癖不报。
- 只读代码 + 写一个报告文件，不做其他任何修改。
