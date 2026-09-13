只读核查完成，未改文件、未运行测试。下文 I/W/R 分别指三个目录的 Sources/src/src。

- **A：部分成立。** Worker 确是独立循环，但演示不是后端，手机还可直连兼容接口，并非固定三处；工具按配置启用。“互不相通”也过强，桌面可共用 Worker 模型代理。[I/ChatService.swift:85](/Users/liuxu/lifeProjects/Potato/native/potato-ios/Sources/ChatService.swift:85)、[W/index.ts:147](/Users/liuxu/lifeProjects/Potato/native/potato-worker/src/index.ts:147)。

- **B：部分成立。** 三类工具事件、分立数组成立，但新增 recall 工具未必新增 struct/视图。“四处”不是固定成本。RemoteMessage 实在 RemoteService.swift；Rust 统一调用帧并支持 MCP 成立。[I/ChatService.swift:25](/Users/liuxu/lifeProjects/Potato/native/potato-ios/Sources/ChatService.swift:25)、[I/RemoteService.swift:21](/Users/liuxu/lifeProjects/Potato/native/potato-ios/Sources/RemoteService.swift:21)、[R/tool_execution.rs:42](/Users/liuxu/lifeProjects/Potato/native/potato-core/src/tool_execution.rs:42)、[R/tools.rs:163](/Users/liuxu/lifeProjects/Potato/native/potato-core/src/tools.rs:163)。

- **C：成立，但“第二轮必失忆”夸大。** 请求不回传工具记录；recall 同样只同步 displayText，无法绕过。最终回答、用户附件及回流产物仍能保留部分信息；同一请求内部工具结果会进入模型下一轮。[I/ChatService.swift:91](/Users/liuxu/lifeProjects/Potato/native/potato-ios/Sources/ChatService.swift:91)、[I/Recall.swift:43](/Users/liuxu/lifeProjects/Potato/native/potato-ios/Sources/Recall.swift:43)、[W/search-chat.ts:117](/Users/liuxu/lifeProjects/Potato/native/potato-worker/src/search-chat.ts:117)。

- **D：部分成立。** 附件回流描述准确；更严重的是同一回答内后续 Python 调用也只拿初始文件，拿不到前次产物。例外：recall 沙箱在请求内复用。[I/CodeExecution.swift:31](/Users/liuxu/lifeProjects/Potato/native/potato-ios/Sources/CodeExecution.swift:31)、[同文件:63](/Users/liuxu/lifeProjects/Potato/native/potato-ios/Sources/CodeExecution.swift:63)、[W/code-tool.ts:24](/Users/liuxu/lifeProjects/Potato/native/potato-worker/src/code-tool.ts:24)、[W/recall.ts:172](/Users/liuxu/lifeProjects/Potato/native/potato-worker/src/recall.ts:172)。

- **E：部分成立。** 两套记忆属实，遗漏 Mac 项目记忆；检索只加载一页候选，并非全部历史。固定脚本处理数据，未见必须隔离的依据，但“多余”需实测 CPU：免费 Worker 仅有较小预算。[R/memory.rs:150](/Users/liuxu/lifeProjects/Potato/native/potato-core/src/memory.rs:150)、[W/recall.ts:228](/Users/liuxu/lifeProjects/Potato/native/potato-worker/src/recall.ts:228)、[官方限制](https://developers.cloudflare.com/workers/platform/limits/)。

- **F：部分成立。** 无审批/steering 成立；关键纠错：forget_memory **不检查 auto_memory，立即删除**；只有 remember 检查开关并延迟提交。断连不能撤销删除。[W/recall.ts:201](/Users/liuxu/lifeProjects/Potato/native/potato-worker/src/recall.ts:201)、[W/search-chat.ts:156](/Users/liuxu/lifeProjects/Potato/native/potato-worker/src/search-chat.ts:156)。

建议逐条判断：

1. **部分成立。** Swift 适配可做；远程已丢失 call_id、结构和长文本，完整统一必须改 Rust。recall 事件也没发送工具名、参数和完整结果。[R/remote.rs:27](/Users/liuxu/lifeProjects/Potato/native/potato-core/src/remote.rs:27)、[W/search-chat.ts:123](/Users/liuxu/lifeProjects/Potato/native/potato-worker/src/search-chat.ts:123)。
2. **不成立。** Worker 拒绝 tool 角色并剥掉 tool_calls；必须联改、补配对调用，压缩不能孤立删除结果。不必先合并三个数组。[W/index.ts:46](/Users/liuxu/lifeProjects/Potato/native/potato-worker/src/index.ts:46)。
3. **部分成立。** 替换会破坏测试，但直接引用仅 Worker 两处、iOS 三处，“大量”夸大。desktop 接口是模型透传，不是统一 runtime；宜先适配并补字段。[W/desktop-chat.ts:8](/Users/liuxu/lifeProjects/Potato/native/potato-worker/src/desktop-chat.ts:8)、[Worker 测试:84](/Users/liuxu/lifeProjects/Potato/native/potato-worker/test/recall.test.ts:84)、[iOS 测试:24](/Users/liuxu/lifeProjects/Potato/native/potato-ios/Tests/RecallTests.swift:24)。
4. **部分成立。** 同步涉及项目范围、来源及删除语义；在线读取也非同步。[D1 支持 FTS5](https://developers.cloudflare.com/d1/sql-api/sql-statements/)，但当前无 D1 绑定，账号规模不足以证明需要新增索引系统。[wrangler:119](/Users/liuxu/lifeProjects/Potato/native/potato-worker/wrangler.jsonc:119)。
5. **部分成立。** 现状不适合直接上 Workers；“必须容器”不成立，且 Linux 沙箱尚缺，容器不是直接可部署方案。[R/sandbox/mod.rs:10](/Users/liuxu/lifeProjects/Potato/native/potato-core/src/sandbox/mod.rs:10)。

保留项**部分成立**：凭据、回执、CAS 应保留；“全部尺寸上限”不能理解为全程内存受限，recall 是完整下载后检查。[I/Recall.swift:74](/Users/liuxu/lifeProjects/Potato/native/potato-ios/Sources/Recall.swift:74)。

总评：先处理 F 的删除授权、C 的上下文及 D 的文件连续性；反对把全面模型合并、协议改名、D1 或记忆合并作为这些修复的前置条件。