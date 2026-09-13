# 对抗审查：iOS 端架构复盘结论

你是审查者。下面是 Claude 对 Potato 仓库 iOS 端（`native/potato-ios`）、Cloudflare Worker（`native/potato-worker`）和 Mac Rust 运行时（`native/potato-core`）做的架构对比结论。请逐条核实，指出错误、夸大、遗漏，以及建议里不成立或代价被低估的部分。

规则：
- 只读审查，不要修改任何文件。
- 每条结论给出「成立 / 部分成立 / 不成立」并附 file:line 证据。
- 不要补充泛泛的"最佳实践"建议；只关注这份结论本身对不对、收敛顺序是否合理。
- 结尾给一个简短的总评：哪几条最值得先做，哪几条你会反对。
- 用中文回答，控制在 1500 字以内。

## 结论 A：iOS 是三个互不相通的后端

同一个 iPhone 客户端按设置把消息送到三处：本地体验（固定文案）、Worker（`search-chat.ts` 自带 agent 循环，工具硬编码为 web_search / run_python / 5 个 recall 工具）、经中继 DO 驱动 Mac 上的 potato-core。Worker 那条是独立于 Rust core 的第二套 agent runtime，功能是子集、协议另起炉灶。

## 结论 B：手机侧没有统一的工具数据模型

- `Sources/ChatService.swift` 的 `SSEDecoder.decode` 只认三个专用事件 `potato_search` / `potato_execution` / `potato_recall`，每个事件对应一套 struct、一个记录函数（`recordSearch` / `recordCodeExecution` / `recordRecall`）、一个视图。新增一种工具要改 Worker、Swift 解码器、`Models.swift`、视图四处。
- 远程路径用 `RemoteMessage{role, kind, text}` 扁平行（`RemoteModels.swift`），本机路径用 `ChatMessage` 加 `searches` / `codeRuns` / `recalls` 三个独立数组。同一个"工具调用"在手机上有两种表示，两套渲染，两套状态机。
- Rust core 虽然也没有 trait（`tool_registry.rs` 宏表 + `tools.rs` 长 match），但调用与结果统一为 `function_call` / `function_call_output` 帧，且有 MCP 客户端。

## 结论 C（bug 级）：工具结果不进下一轮上下文

`ChatService.request(settings:token:messages:storage:draft:choice:)` 组装 wire 消息时只序列化每条消息的 `displayText` 和用户附件（图片 base64 / 提取文本），`searches`、`codeRuns`、`recalls` 全部不进请求体。模型下一轮只知道自己最终说了什么，不知道自己查到了什么、跑出了什么。多轮数据分析任务第二轮会失忆。

请特别核实：Worker 侧是否有别的机制把上一轮工具结果带回（例如 recall 同步的对话记录），使得这个问题实际上被绕过了。

## 结论 D：沙箱与会话没有文件系统级连续性

E2B 每次全新（`sandbox.ts`），手机侧 `CodeExecution.swift` 的 `automaticInput` 只把"最近一条带附件的消息"的至多 4 个文件重新上传。上一轮产物能回流是因为 `recordCodeExecution` 把产物追加进了回复消息的 `attachments`，隐式且只覆盖最近一条。对手机可接受，但边界要明确。

## 结论 E：记忆是两座孤岛，且检索方式可疑

- Mac 记忆在本机 `workspace/memory` Markdown 文件（`memory.rs`），iOS 本机聊天的记忆在 R2 `manifest.json`（`recall.ts`）。远程模式看 Mac 记忆，本机模式看 R2 记忆，两者不互通。
- Worker 的跨对话检索方式：把至多 10 个对话（≤1.5 MB）写进 E2B 沙箱，跑固定 Python 关键词脚本 `RECALL_SEARCH_CODE`（`recall.ts:149-191`）。既然已经全量加载进 Worker 内存，再起沙箱来 grep 是多余的成本和延迟。

请核实：用沙箱做检索是否有我没看到的理由（比如 Worker CPU 时间限制、隔离不可信内容的安全考虑）。

## 结论 F：Worker 循环没有审批

`search-chat.ts` 所有工具自动执行，`remember` / `forget_memory` 有副作用也自动执行（受 `auto_memory` 开关）。没有 steering，中途只能整体断连。

## 建议的收敛顺序

1. 手机侧建通用 `ToolCall {id, name, arguments, status, result, artifacts}` 模型，合并三个数组，渲染按 name 查表，未知工具走通用卡片；远程路径也用它。只改 Swift。
2. `request()` 把 ToolCall 序列化成 `tool` 角色消息送回上下文，并加最简单的压缩（超预算先丢最旧工具结果）。只改 Swift，依赖 1。
3. Worker 的 SSE 事件对齐 Rust core 帧格式（`function_call` / `function_call_output`），替代三个 `potato_*` 事件。Worker 已有 `/v1/desktop/chat/completions` 接受客户端自带 tools。依赖 1。
4. 记忆二选一：Mac 端加云端记忆同步，或远程在线时手机本机聊天读 Mac 记忆；同时把 E2B 关键词搜索换成 Worker 内存匹配或 D1 FTS5。
5. 不把 Rust core 搬上云：它依赖 Seatbelt、cap-std、子进程、本地 SQLite，上云需容器而非 Workers。

## 我认为应保留、不要顺手重写的部分

瘦客户端定位；Keychain 凭据隔离与换主机清空；全部网络层禁重定向与尺寸上限；远程 RPC 幂等回执与 `expected_run_id` 绑定；E2B 无网每次全新；R2 记忆 etag CAS。

## 需要你重点挑战的问题

1. 结论 C 是否真的成立？有没有被 recall 同步或其他机制绕过？
2. 建议 3 的代价：把 `potato_*` 事件换成 `function_call` 帧，会不会破坏现有 Worker 单测（`native/potato-worker/test`）和 iOS 单测（`native/potato-ios/Tests`）里对这三个事件的大量依赖？是否值得，还是应该只在手机侧做适配层（建议 1）就停？
3. 建议 1 中"远程路径也用 ToolCall"：`remote.rs` 的 `display_messages` 输出的是已扁平化的 rows，手机侧能否从中重建结构化工具调用？如果不能，这条建议需要改 Rust core，代价评估是否遗漏？
4. 建议 4 的 D1 FTS5 是否与当前 wrangler 绑定和账号规模匹配？
5. 有没有我完全漏掉的维度或更严重的问题？
