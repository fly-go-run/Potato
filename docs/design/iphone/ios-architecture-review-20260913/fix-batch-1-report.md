# 工作包 1 修复与验证报告

日期：2026-09-13。分支：`feat/process-track-codex`。修复 1、2、3 已完成，Worker 类型检查与 69 项测试通过，iPhone 17 模拟器 122 项单元测试通过。未部署或发布。

已先阅读 `docs/architecture/native-only.md`；本包依据用户明确指定的 iOS + Worker 范围实施。未修改 `native/potato-core`、GPUI、旧客户端或其他现有工作；没有 commit、stash、revert。

## 修改文件与规格映射

本包修改以下 12 个源码、测试、README 文件，另新增本报告。行号对应本次完成后的文件。

| 文件 | 实现位置与规格 |
| --- | --- |
| [Worker src/recall.ts](../../../../native/potato-worker/src/recall.ts) | 201–204：`forget_memory` 分支首先检查 `autoMemory`，关闭时抛出 `Memory updates are disabled in settings.`；开启时仍立即执行原删除逻辑。对应修复 1.1。 |
| [Worker src/search-chat.ts](../../../../native/potato-worker/src/search-chat.ts) | 17：关闭自动记忆时，工具列表同时过滤 `remember` / `forget_memory`。16：Python 系统提示说明每次重建沙箱、成功产物回流到 `/home/user/`，以 `available_files` 为准。110：模型工具结果加入完整可用文件路径列表，产物仍只给模型名称与 MIME。对应修复 1.2、3。 |
| [Worker src/index.ts](../../../../native/potato-worker/src/index.ts) | 45–78：assistant 的可选 `tool_calls` 校验 1–16 项、ID 长度 1–200、function 类型、名称正则、arguments 最大 32,000 字符及字符串 content；tool 校验 ID 长度 1–200、content 长度 1–16,000。仅转发声明字段。`ids` 保证请求内调用 ID 唯一，`pending` 保证连续、完整且恰好一次配对，缺失、多余、重复或打断都返回 400 `Invalid tool history.`。对应修复 2b。 |
| [Worker src/code-tool.ts](../../../../native/potato-worker/src/code-tool.ts) | 5：工具描述更新。11、25–46：只读 `currentFiles` getter；闭包 `current` 初值为用户文件，每次 run 使用当前文件，仅成功执行后合并产物。复用 `validateSandbox` 的名称与编码校验，同名后者覆盖；超过 4 个文件或解码后合计 2,000,000 字节时，按并入先后移除产物，原始输入名称不参与淘汰。仍放不下则不接受该产物。对应修复 3。 |
| [Worker test/recall.test.ts](../../../../native/potato-worker/test/recall.test.ts) | 105、116：新增两项测试，覆盖关闭时不宣告两个记忆修改工具，以及直接调用 forget 抛 disabled。对应修复 1.3。 |
| [Worker test/api.test.ts](../../../../native/potato-worker/test/api.test.ts) | 57–81：新增合法历史转发测试，分别经过直通与搜索路径；另四项测试验证缺配对、多余 tool、重复 ID、不连续 tool 均为 400，错误文字也符合规格。对应修复 2b 测试。 |
| [Worker test/code-tool.test.ts](../../../../native/potato-worker/test/code-tool.test.ts) | 98、111：新增两次 run 的产物回流测试，以及超过 4 文件时淘汰最早产物、保留用户输入的测试。现有 mocks 补 `currentFiles`，现有结果断言补 `available_files` 路径。对应修复 3 测试。 |
| [Worker README.md](../../../../native/potato-worker/README.md) | 11：「已实现」新增 assistant tool_calls → 配对 tool → assistant 文字的请求格式说明。对应修复 2b README 要求。 |
| [iOS Sources/ChatService.swift](../../../../native/potato-ios/Sources/ChatService.swift) | 84–135：纯函数 `toolHistory` 读取所选版本的 `displaySearches` / `displayCodeRuns`，只选 complete/failed；先全部搜索再全部执行，合为一个 tool_calls 数组，再依序附每个 ID 的一条 tool。搜索 content 使用指定字段，来源正文最多 600 个字符，失败为 search failed；Python 使用指定字段及名称/MIME，不含 base64，无 result 的失败采用 message 或 execution failed。88–100：每条 content 按 Swift Character 截断，包含后缀 `…[truncated]` 后仍不超过 8,000 UTF-8 字节。128–134：整段工具历史按序列化 UTF-8 大小统计，超过 200,000 字节从最早回复整段移除。144–163：request 插入保留的工具段，随后沿用最终 displayText；裁掉的回复回到原来的文字消息。对应修复 2a、2c。 |
| [iOS Sources/CodeExecution.swift](../../../../native/potato-ios/Sources/CodeExecution.swift) | 57：持久化产物保留 name、mime，base64 置为空字符串；附件导入逻辑继续沿用。`CodeExecutionRun.validate` 未改。对应修复 2c。 |
| [iOS Tests/StreamServiceTests.swift](../../../../native/potato-ios/Tests/StreamServiceTests.swift) | 94–180：在现有文件新增 `ToolHistoryTests` 四项测试，覆盖搜索+Python 的数组与配对顺序、参数与最终文字、不带 base64；版本选择；超过 200,000 字节后最早回复整段退化且后续配对保留（同时断言单条 8,000 字节及后缀）；running/stopped 不回放。对应修复 2c 四项测试。 |
| [iOS Tests/CodeExecutionTests.swift](../../../../native/potato-ios/Tests/CodeExecutionTests.swift) | 46–48：将原有“产物数组为空”的断言更新为保留名称/MIME、base64 为空，并确认保存后的 run 仍通过 validate。其余已有执行、附件恢复及停止测试继续通过。 |

确认 `search-chat.ts:13` 的数组浅复制完整保留历史消息字段；模型请求继续使用这些消息。直通路径也使用 validate 后的消息，无需改动转发逻辑。`validateDesktop`、`sandbox.ts` 均未改。SSE 的 `potato_search` / `potato_execution` / `potato_recall` 名称保持原样。Worker 与 Swift 的 4 MiB 请求体上限保持原样。

## 验证命令与真实输出

### Worker

在 `native/potato-worker` 执行：

```sh
npm run check && npm test
```

首次在受限沙箱执行：类型检查通过，测试退出码 1。真实汇总为 `tests 71 / pass 49 / fail 22`；失败来自 Miniflare 测试初始化与清理时 `listen EPERM: operation not permitted 127.0.0.1`，含文件级 hook 失败，不能据此称为测试通过。

通过权限流程允许本机监听后重跑同一命令，退出码 **0**：

```text
> check
> tsc --noEmit
> test
> node --test test/*.test.ts
ℹ tests 69
ℹ suites 0
ℹ pass 69
ℹ fail 0
ℹ cancelled 0
ℹ skipped 0
ℹ todo 0
ℹ duration_ms 1267.832
```

日志：`/tmp/potato-fix-batch-1-worker.log`（沙箱失败）、`/tmp/potato-fix-batch-1-worker-retry.log`（通过）。此前另有一次误在仓库根执行 npm 的操作失误，退出码 254，因根目录没有 `package.json` 报 ENOENT；随即修正工作目录，未造成文件修改。

### iOS

在 `native/potato-ios` 执行指定命令：

```sh
xcodegen generate && xcodebuild -project PotatoMobile.xcodeproj -scheme PotatoMobile -destination 'platform=iOS Simulator,name=iPhone 17' -derivedDataPath build -only-testing:PotatoMobileTests test
```

首次在受限沙箱执行：xcodegen 成功，xcodebuild 退出码 **70**。CoreSimulatorService 连接被拒绝，最终报 `Unable to find a device matching the provided destination specifier`。

经权限流程运行 `xcrun simctl list devices available`，退出码 0，确认 iOS 26.3 下存在 `iPhone 17 (BC20CB5A-FAC0-4A46-8E85-C58C35337B22)`，无需替换机型。随后经权限流程重跑上面完整命令，退出码 **0**：

```text
Test Suite 'ToolHistoryTests' passed
Executed 4 tests, with 0 failures (0 unexpected)
Test Suite 'All tests' passed at 2026-09-13 20:55:11.047.
Executed 122 tests, with 0 failures (0 unexpected) in 2.267 (2.300) seconds
** TEST SUCCEEDED **
```

既有 `CodeExecutionTests` 全部通过。日志包含非阻断的 `Metadata extraction skipped. No AppIntents.framework dependency found.` 警告。

日志：`/tmp/potato-fix-batch-1-ios.log`（沙箱失败）、`/tmp/potato-fix-batch-1-ios-retry.log`（通过）。结果包：`native/potato-ios/build/Logs/Test/Test-PotatoMobile-2026.09.13_20-54-41-+0800.xcresult`。

只执行 `PotatoMobileTests`；指定 scheme 会构建 UI 测试 target，但未运行 UI 测试。xcodegen 生成后的 `project.pbxproj` 与执行前备份一致，没有新增工程配置改动。新增 Swift 测试放在现有测试文件中。

## 实施取舍与未做事项

- 不做通用 ToolCall 模型、事件改名、记忆合并、延迟 forget 提交及无关重构。不增加规格外测试。
- 工具历史总预算按完整工具段的序列化 JSON UTF-8 大小计费，包含 assistant 包装字段及 JSON 标点，保证不低估请求开销；不计最终回复文字。单条工具 content 超限后按规格截断，截断后的 content 文本不保证仍是完整 JSON，外层请求仍是有效 JSON。
- 文件容量按解码后 2,000,000 字节判断。原输入同名产物按“同名后者覆盖”替换，但该原输入名称仍受淘汰保护；无法容纳的新产物不提交候选文件集，原文件集不因失败的加入而丢失。
- recalls 不回放；不迁移以前已清空的产物元数据。测试仅验证本地实现与模拟器行为，未做线上 E2B/模型联调、UI 测试或发布。
- 除指定的源码/测试/Worker README 外，只新增用户要求的本报告；构建缓存与 xcresult 是指定验证命令的输出，不作为源码改动提交。
