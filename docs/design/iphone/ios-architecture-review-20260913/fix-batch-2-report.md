# 工作包 2：远程工具调用结构保留

完成日期：2026-09-13。分支：`feat/process-track-codex`。

本包在 Rust 远程消息 row 上附加工具结构字段，Swift 解码后优先使用结构化工具名，并在现有折叠详情中分别展示参数、输出和失败状态。原有文本协议继续保留。

## 文件与规格对应位置

| 修改文件与位置 | 对应规格 |
| --- | --- |
| `native/potato-core/src/remote.rs:33` | 原 row 的 `id`、`role`、`kind`、`text`、`status` 构造不变，只将 row 改为可追加字段。原有按 ID 更新、文本增量处理逻辑不变。 |
| `native/potato-core/src/remote.rs:34` | 仅处理 `function_call` / `function_call_output`；从 content 数组选择第一个 data 含 `call_id` 的 block。调用帧附加 `call_id/name/arguments`，输出帧附加 `call_id/name/output/state`。缺失字段不写入、不补 null。 |
| `native/potato-core/src/remote.rs:39` | `arguments/output` 字符串原样取，其他 JSON 值用 `to_string()`；按 Rust `chars()` 截取 4000 个 Unicode 字符，超出追加 `\n…`。 |
| `native/potato-core/src/remote.rs:57` | 原有 120 行保留规则、历史提示行和 8000 字符 text 截断代码不变。 |
| `native/potato-core/src/remote_tests.rs:7` | 新增同步测试 `display_messages_keeps_tool_call_structure`：用 protocol 构造两条工具帧；验证 c1、read_file、对象参数序列化、旧 text 前缀、4001 个中文字符输出截为 4000 字符加换行与省略号、success、原 text 未受 4000 限制影响；另构造普通 assistant 帧并验证没有 call_id。 |
| `native/potato-ios/Sources/RemoteService.swift:21` | RemoteMessage 增加五个可选 String 字段；CodingKeys 将 `callID` 映射为 `call_id`，其他新旧字段同名。 |
| `native/potato-ios/Sources/RemoteMessageView.swift:7` | processTitle 优先采用非空 name，复用 titles 和原有 ASCII/长度校验；nil/空字符串回退原 text 首行逻辑。reasoning、tool role 和 output kind 的既有优先规则保留。 |
| `native/potato-ios/Sources/RemoteMessageView.swift:109` | 原“查看详情”中，非空 arguments 显示“参数”与等宽正文；非空 output 显示“输出”与等宽正文，state 存在且非 success 时在小标题旁显示红色“失败”；两者均空时沿用原 text/“暂无文字记录”。reasoning 展示、折叠分组、accessibilityIdentifier 与已有配色常量不变。 |
| `native/potato-ios/Tests/RemotePresentationTests.swift:5` | 辅助函数新增字段参数默认 nil，继续使用 memberwise 构造器，旧测试调用无需调整。 |
| `native/potato-ios/Tests/RemotePresentationTests.swift:8` | 新增 read_file 与 mcp_abc 两个结构化标题用例。原三条 processTitle 断言保留在第 38 行开始的原测试中。 |
| `native/potato-ios/Tests/RemotePresentationTests.swift:14` | 补充实际 JSON 解码测试，验证五个字段及旧 JSON 缺少这些字段时全部为 nil，防止 call_id 映射或兼容性回归。 |

以上五个代码/测试文件和本报告是本包全部修改。没有修改其他 Tests 文件。没有 commit、stash、revert；启动时已有的 GPUI main.rs、view.rs、theme_mode.rs 及 brief 文档改动保持原状。xcodegen 生成的是本地工程产物，没有额外受版本控制文件变更。

## Rust 验证

在 `native/potato-core` 执行用户指定测试；添加 `set -o pipefail` 和 `tee` 保留真实退出码、完整日志，筛选仍为 tail -20：

```sh
set -o pipefail
cargo test --lib remote_tests -- --nocapture 2>&1 | tee /tmp/potato-batch-2-rust.log | tail -20
```

首次沙箱内运行退出 101，真实摘要：

```text
test remote_tests::display_messages_keeps_tool_call_structure ... ok
test result: FAILED. 9 passed; 2 failed; 0 ignored; 0 measured; 177 filtered out; finished in 0.19s
```

失败项为 `expired_remote_login_can_be_cleared_for_signing_in_again` 和 `native_login_persists_sealed_credentials_and_requires_explicit_remote_enable`。均在 `remote_tests.rs:203` 的 `TcpListener::bind("127.0.0.1:0")` 失败，错误为 `PermissionDenied / Operation not permitted`。

获准在沙箱外运行同一测试后通过，退出 0；日志 `/tmp/potato-batch-2-rust-retry.log`：

```text
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.59s
running 11 tests
test remote_tests::display_messages_keeps_tool_call_structure ... ok
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 177 filtered out; finished in 0.36s
```

## Swift / iOS 验证

在 `native/potato-ios` 执行：

```sh
set -o pipefail
xcodegen generate && xcodebuild -project PotatoMobile.xcodeproj -scheme PotatoMobile -destination 'platform=iOS Simulator,name=iPhone 17' -derivedDataPath build -only-testing:PotatoMobileTests test 2>&1 | tee /tmp/potato-batch-2-ios.log | grep -E 'Test Suite|Executed|error:|TEST'
```

首次 xcodegen 成功；沙箱内 xcodebuild 退出 70，未执行测试。日志含 `CoreSimulatorService connection became invalid`、`Connection refused` 和 `Unable to find a device matching the provided destination specifier`。

获准访问模拟器后原命令重跑，xcodegen 成功、xcodebuild 退出 0；日志 `/tmp/potato-batch-2-ios-retry.log`：

```text
Test Suite 'RemotePresentationTests' passed at 2026-09-13 21:03:18.647.
Executed 7 tests, with 0 failures (0 unexpected) in 0.009 (0.011) seconds
Test Suite 'All tests' passed at 2026-09-13 21:03:19.091.
Executed 125 tests, with 0 failures (0 unexpected) in 2.250 (2.282) seconds
** TEST SUCCEEDED **
```

两个新增标题用例、额外 JSON 解码用例和包含原三条 processTitle 断言的 `testProcessIdentitySurvivesStreamingAppendAndDoesNotUseLogAsTitle` 均通过。

测试结果包：`native/potato-ios/build/Logs/Test/Test-PotatoMobile-2026.09.13_21-02-47-+0800.xcresult`。

## 范围与验证边界

- 功能规格无有意跳过项。规格外的畸形 `call_id/name/state` 非字符串值直接忽略；没有增加类型纠正或通用工具模型。
- 未改 Worker、SSE 事件名、本机 searches/codeRuns/recalls 数组，也未替换专用工具视图或改动帧来源。
- 除要求的测试外，仅额外补充一条 Swift JSON 解码兼容测试。多 block 选择与 arguments 截断由代码审查确认，未另加边界测试矩阵。
- 验证覆盖 Rust 远程单元测试及 iPhone 17 模拟器 PotatoMobileTests；没有执行 UI 自动化、真机远程 Mac 联调或视觉验收，不将单元测试通过视为这些验收已完成。
- `git diff --check` 通过。
