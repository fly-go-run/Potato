# 工作包 1：iOS 本机聊天三处 bug 修复（Worker + Swift）

背景：`docs/design/iphone/ios-architecture-review-20260913/` 下的架构复盘与你的审查结论。本包只修三处 bug，不做任何结构收敛（通用 ToolCall 模型、事件改名、记忆合并都不在范围内）。

约束：
- 仓库根 `/Users/liuxu/lifeProjects/Potato`，分支 `feat/process-track-codex`，工作区已有大量未提交改动，**不要 commit、不要 stash、不要 revert 任何你没改的文件**。
- 只改 `native/potato-worker/src`、`native/potato-worker/test`、`native/potato-worker/README.md`、`native/potato-ios/Sources`、`native/potato-ios/Tests`、`native/potato-ios/README.md`。不碰 `native/potato-core`。
- 不改 SSE 事件名（`potato_search` / `potato_execution` / `potato_recall` 保持原样）。
- 不追求完美：按下面的规格做，规格没写的边界按最简单的处理，不要顺手重构无关代码，不要补规格之外的测试。
- 完成后把验证命令的真实输出摘要写进 `docs/design/iphone/ios-architecture-review-20260913/fix-batch-1-report.md`（失败就如实写失败）。

---

## 修复 1：`forget_memory` 不受自动记忆开关约束

现状：`native/potato-worker/src/recall.ts` 的 `RecallSession.call` 里 `remember` 检查 `this.autoMemory`，`forget_memory` 不检查，立即调用 `store.memory(... forget: true)`。

规格：
1. `forget_memory` 分支开头加与 `remember` 相同的门：`if (!this.autoMemory) fail('Memory updates are disabled in settings.');`
2. `search-chat.ts` 组装 `availableTools` 时，若 `recall && !recall.autoMemory`，从 `recallTools` 中过滤掉 `remember` 和 `forget_memory` 两个定义，不向模型宣告。`recall.autoMemory` 已是 `readonly` 公开字段。
3. 测试：`test/recall.test.ts` 加两个用例：autoMemory=false 时 (a) 工具列表不含这两个名字，(b) 直接调用 `forget_memory` 抛 "disabled"。autoMemory=true 时行为不变（现有用例覆盖）。

不做：不把 forget 改成延迟提交。

---

## 修复 2：上一轮工具结果不进下一轮上下文

现状：
- Swift `native/potato-ios/Sources/ChatService.swift` 的 `ChatService.request(settings:token:messages:storage:draft:choice:)` 组装 wire 消息时只用 `displayText` 与用户附件；`ChatMessage` 上的 `displaySearches` / `displayCodeRuns` 全部丢弃。
- Worker `native/potato-worker/src/index.ts` 的 `validate()` 只放行 `system` / `user` / `assistant` 三种 role，assistant 只允许字符串 content。

### 2a. Wire 格式（Worker 与 Swift 共同契约）

一条曾经调用过工具的 assistant 回复，在下一轮请求里展开为**三段**（OpenAI Chat Completions 标准格式）：

```json
{"role":"assistant","content":"","tool_calls":[
  {"id":"<run.id>","type":"function","function":{"name":"web_search","arguments":"{\"query\":\"...\"}"}},
  {"id":"<run.id>","type":"function","function":{"name":"run_python","arguments":"{\"code\":\"...\"}"}}
]}
{"role":"tool","tool_call_id":"<run.id>","content":"<JSON 字符串>"}
{"role":"tool","tool_call_id":"<run.id>","content":"<JSON 字符串>"}
{"role":"assistant","content":"<该回复的 displayText>"}
```

- 同一条回复的所有工具调用合并进**一个** `tool_calls` 数组（先全部 searches，再全部 codeRuns，各自按数组顺序），随后每个 id 对应恰好一条 `tool` 消息，顺序与 `tool_calls` 一致，然后才是最终文字。
- `recalls` 不参与（手机没有记录工具名与参数）。
- 只回放 `state == "complete"` 或 `"failed"` 的 run；`running` / `stopped` 跳过。若一条回复过滤后没有可回放的 run，就退化为原来的单条 assistant 消息。
- `web_search` 的 tool content：`{"query":..., "state":..., "results":[{"title","url","content"(截到 600 字符),"publishedDate"}]}`，`failed` 时 `{"error":"search failed"}`。
- `run_python` 的 tool content：`{"status","stdout","stderr","error","text","artifacts":[{"name","mime"}]}`，`failed` 且无 result 时 `{"error": run.message ?? "execution failed"}`。**不含 base64。**
- 每条 `tool` content 的 UTF-8 长度上限 8000 字节，超过按字符截断并追加 `…[truncated]`。
- 整个请求里 `tool_calls` + `tool` 消息的 UTF-8 总长上限 200 000 字节。超过时**从最早的回复开始**整段丢弃其 `tool_calls` 与全部 `tool` 消息（该回复退化为单条 assistant 文字），直到不超。绝不单独删某一条 `tool` 消息。
- 现有的 4 MiB 请求体上限保持不变。

### 2b. Worker：`validate()` 放行并校验配对

`native/potato-worker/src/index.ts` `validate()`：
- `assistant` 消息允许可选 `tool_calls`：数组，1–16 项，每项 `{id: string 1–200, type: "function", function: {name: /^[a-z_]{1,64}$/, arguments: string ≤ 32 000}}`。此时 `content` 必须是字符串（可空）。只转发这些字段。
- 新增 `tool` role：`{tool_call_id: string 1–200, content: string 1–16 000}`。
- 配对校验（校验失败一律 `APIError(400, 'Invalid tool history.')`）：
  - 每条 `tool` 消息必须紧跟在一条带 `tool_calls` 的 assistant 之后（中间只允许其他 `tool` 消息）；
  - 该 assistant 的每个 `tool_calls[].id` 必须有且仅有一条对应的 `tool` 消息，多余或缺失都拒绝；
  - id 在整个请求内唯一。
- `validateDesktop`（`desktop-chat.ts`）本来就接受 tool 消息，不要动。
- `search-chat.ts` 与直通路径均原样把这些消息转发上游，不需要改（确认一下 `chatWithSearch` 的 `messages` 复制不会破坏它们即可）。
- 测试：`test/api.test.ts` 加用例：(a) 合法三段历史通过校验并被转发；(b) 缺配对、多余 `tool`、id 重复、`tool` 不紧跟 assistant 四种非法各返回 400。
- README「已实现」段补一句请求格式说明。

### 2c. Swift：`request()` 生成工具历史

`native/potato-ios/Sources/ChatService.swift`：
- 在现有 for 循环里，对 `role == "assistant"` 且有可回放 run 的消息按 2a 展开；其余逻辑不变。用 `displaySearches` / `displayCodeRuns`（尊重回复版本选择）。
- 预算裁剪按 2a 实现为一个纯函数（例如 `static func toolHistory(...)`），便于测试。
- `native/potato-ios/Sources/CodeExecution.swift` 的 `recordCodeExecution` 目前 `saved.result?.artifacts = []`；改为保留 `name` 与 `mime`、把 `base64` 置空字符串，这样回放时能报产物名。检查 `CodeExecutionRun.validate` 与现有 `CodeExecutionTests` 不受影响（`base64` 为空满足总量上限）。
- 测试放 `native/potato-ios/Tests/StreamServiceTests.swift` 或新建 `ToolHistoryTests.swift`：
  1. 一条 assistant 含 1 个 complete search + 1 个 complete codeRun → 请求体中出现 `tool_calls`（2 项）、2 条 `tool`、最终 assistant 文字，顺序正确，`tool` content 不含 base64；
  2. 选中的回复版本不同 → 回放的是该版本的 run；
  3. 超 200 000 字节 → 最早回复的整段被丢弃，后面保留；
  4. `running` / `stopped` run 不回放。

---

## 修复 3：同一回答内后续 Python 调用拿不到前次产物

现状：`native/potato-worker/src/code-tool.ts` 的 `codeTool()` 在构造时固定 `files`，每次 `run` 都用同一份输入。

规格：
- 闭包内维护 `current: SandboxInput['files']`，初值为用户输入文件。
- 每次 `run` 成功（`execution.status === 'complete'`）后，把 `execution.artifacts` 中名字能通过 `validateSandbox` 文件名规则的产物以 `{name, base64}` 并入 `current`：同名后者覆盖；若超过 4 个文件或超过 2 MB 总量，**先丢最早并入的产物**，绝不丢用户原始输入文件；仍放不下的产物直接不并入。
- 下一次 `run` 用更新后的 `current`。产物在下一次沙箱中位于 `/home/user/<name>`（与输入文件同一目录，由 `sandbox.ts` 现有写入逻辑决定，不改 sandbox.ts）。
- 给模型的 tool 结果（`search-chat.ts` 里 `result = { ...execution, artifacts: ... }`）追加字段 `available_files: string[]`，列出下一次调用可用的全部文件路径。`CodeTool` 类型上暴露一个只读 getter（如 `get currentFiles()`）供 search-chat 读取。
- 更新 `pythonTool.description` 与 `search-chat.ts` 中 run_python 的系统提示：把 "Each call starts fresh" 改成说明"沙箱每次重建，但本回答中前几次成功调用生成的文件会作为输入文件放回 `/home/user/`，见工具结果的 `available_files`"。
- 测试：`test/code-tool.test.ts` 加用例：模拟两次 `run`，第一次产出 `a.png`，第二次执行时 `execute` 收到的 files 含 `a.png`；再加一个超 4 文件时丢最早产物、保留用户文件的用例。

---

## 验证命令

```sh
cd native/potato-worker && npm run check && npm test
cd native/potato-ios && xcodegen generate && xcodebuild -project PotatoMobile.xcodeproj -scheme PotatoMobile -destination 'platform=iOS Simulator,name=iPhone 17' -derivedDataPath build -only-testing:PotatoMobileTests test
```

UI 测试不需要跑。若 iPhone 17 模拟器不存在，用 `xcrun simctl list devices available` 里任意一台 iPhone 替换。

报告里写清：改了哪些文件、每条规格对应的实现位置、验证输出摘要、你有意跳过或与规格不同的地方及理由。
