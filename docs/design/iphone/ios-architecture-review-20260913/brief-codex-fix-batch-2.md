# 工作包 2：远程路径保留工具调用结构（Rust core + Swift）

背景：`docs/design/iphone/ios-architecture-review-20260913/` 的复盘与审查。工作包 1 已完成（Worker + Swift 三处 bug）。本包只修一处结构缺口：手机远程查看 Mac 会话时，`potato-core` 的 `remote.rs::display_messages` 把 `function_call` / `function_call_output` 帧压成「工具名换行参数」的纯文本，call_id、参数、输出、状态全部混在 `text` 里，手机只能靠解析第一行猜工具名。

**不做**：不合并本机路径的 `searches` / `codeRuns` / `recalls` 三个数组，不改 Worker，不改 SSE 事件名，不做通用工具卡片替换现有专用视图。

约束：
- 仓库根 `/Users/liuxu/lifeProjects/Potato`，分支 `feat/process-track-codex`，工作区有大量未提交改动，**不要 commit、stash、revert 任何你没改的文件**。
- 只改 `native/potato-core/src/remote.rs`、`native/potato-core/src/remote_tests.rs`、`native/potato-ios/Sources/RemoteService.swift`、`native/potato-ios/Sources/RemoteMessageView.swift`、`native/potato-ios/Tests/RemotePresentationTests.swift`（其他 Tests 文件若因构造器变化编译失败可最小修改）。
- 不追求完美；规格外的边界按最简单处理，不要顺手重构。
- 完成后写 `docs/design/iphone/ios-architecture-review-20260913/fix-batch-2-report.md`，含真实验证输出摘要（失败就写失败）。

---

## 1. Rust：`display_messages` 为工具帧附加结构字段

文件 `native/potato-core/src/remote.rs`，函数 `display_messages(frames: &[Value]) -> Value`（约 22–47 行）。

帧来源（不要改这些）：`native/potato-core/src/tool_execution.rs` 第 42 行构造 `function_call` 帧，`content` 为一个 block 数组，block 的 `data` 为 `{"call_id","name","arguments"}`；第 136–153 行构造 `function_call_output` 帧，role `tool`，`data` 为 `{"call_id","name","output","state"}`，state 为 `"success"` 或其他。帧的 `type` 字段分别是 `"function_call"` / `"function_call_output"`。`protocol::message` / `protocol::data` 在 `src/protocol.rs:29-45`。

规格：
- 保持现有 row 的 `id` / `role` / `kind` / `text` / `status` 行为**完全不变**（旧客户端继续工作）。
- 当帧 `type == "function_call"` 时，row 额外带：`"call_id": <string>`, `"name": <string>`, `"arguments": <string>`。`arguments` 若在 `data` 中是字符串就原样取，否则 `to_string()`；按 Unicode 字符截到 4000 个字符，超出追加 `\n…`。
- 当帧 `type == "function_call_output"` 时，row 额外带：`"call_id"`, `"name"`, `"output": <string>`（同样 4000 字符截断）, `"state": <string>`。`output` 若不是字符串则 `to_string()`。
- 只有 `data` 里存在对应字段时才加；缺失就不加（不要写 `null`）。
- 多个 block 的情况取第一个含 `call_id` 的 block。
- 现有的 8000 字符 `text` 截断与 120 行截断逻辑不动。
- 测试：`src/remote_tests.rs` 新增一个**同步**单元测试 `display_messages_keeps_tool_call_structure`，直接构造两帧（用 `protocol::message` + `protocol::data`，一帧 function_call 带 call_id `c1`、name `read_file`、arguments `{"path":"a.md"}`；一帧 function_call_output 带 call_id `c1`、output 超过 4000 字符、state `success`），断言：两行的 `call_id` 都是 `c1`；第一行 `name == "read_file"`、`arguments` 含 `a.md`、`text` 仍以 `read_file\n` 开头；第二行 `output` 长度为 4000 字符加 `\n…`，`state == "success"`；普通 assistant 文本帧的 row **没有** `call_id` 键。

验证：`cd native/potato-core && cargo test --lib remote_tests -- --nocapture 2>&1 | tail -20`。整个 crate 编译可能要几分钟，正常。

## 2. Swift：`RemoteMessage` 解码并使用结构字段

文件 `native/potato-ios/Sources/RemoteService.swift` 第 21–23 行 `struct RemoteMessage: Decodable, Identifiable`。

规格：
- 增加可选字段 `callID: String?`（JSON key `call_id`）、`name: String?`、`arguments: String?`、`output: String?`、`state: String?`。用 `CodingKeys` 映射 `call_id`，其余同名。其他现有字段不变。
- `Tests/RemotePresentationTests.swift` 第 5–6 行的 `message(...)` 辅助函数直接用 memberwise 构造器；给新字段默认值 `nil`，保证现有测试无需改动。

文件 `native/potato-ios/Sources/RemoteMessageView.swift`：
- `processTitle`：当 `name` 非空时，用 `name` 查现有 `titles` 表；查不到时沿用现有「调用 \(name)」规则（同样的 ASCII/长度校验）。`name` 为空时保持现有的「解析 text 第一行」逻辑不变。`role == "tool"` 或 kind 含 `output` 仍返回「执行结果」。
- `RemoteProcessView` 展开后的每条非 reasoning 消息：
  - 若 `arguments` 非空：`DisclosureGroup("查看详情")` 里先显示一行小标题「参数」，下面等宽字体显示 `arguments`；
  - 若 `output` 非空：小标题「输出」，下面等宽字体显示 `output`；若 `state` 存在且不是 `"success"`，小标题旁显示红色「失败」；
  - 两者都为空：沿用现有显示 `text` 的逻辑。
  - 不改折叠分组规则、不改 accessibilityIdentifier、不改配色常量。
- 测试：`Tests/RemotePresentationTests.swift` 新增两个用例：(a) `name: "read_file"` 且 `text` 为任意内容时 `processTitle == "读取文件"`；(b) `name: "mcp_abc"` 时 `processTitle == "调用 mcp_abc"`；并确认旧的三条 `processTitle` 断言仍通过。

验证：
```sh
cd native/potato-ios && xcodegen generate && xcodebuild -project PotatoMobile.xcodeproj -scheme PotatoMobile -destination 'platform=iOS Simulator,name=iPhone 17' -derivedDataPath build -only-testing:PotatoMobileTests test 2>&1 | grep -E "Test Suite|Executed|error:|TEST"
```

报告里写清：改了哪些文件、每条规格对应的位置、两侧验证输出摘要、有意跳过或与规格不同之处及理由。
