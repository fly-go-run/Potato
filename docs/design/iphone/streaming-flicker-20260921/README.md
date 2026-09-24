# iOS 直接聊天的流式闪动修复

2026-09-21，已发布 TestFlight **0.2.2（2026092101）** 至既有「个人内测」组，确认 Testing，中文说明读回一致。见 [发布记录](release-2026092101/README.md)。用户明确反馈的是手机直接与模型聊天。本轮只修改 iOS 的这条显示链路及其测试；未修改服务端流协议或远程电脑任务界面。

## 发现与修复

- `StreamingMarkdown` 原来总从空缓冲开始，即使视图创建时已收到长篇回复，也会先显示空内容，再重新逐字展开。切换回生成中的对话或视图重新创建时，消息高度会骤降。现在创建时直接使用已有正文，后续新增文字仍保留平滑显示。
- 本机聊天使用 `LazyVStack`，每次可见字符变化都向父视图发送计数并调用 `scrollTo`。改为实际布局的 `VStack`，只在内容高度变化时追底，避免流式消息高度估算与频繁追底相互影响。保留上滑暂停、点击箭头恢复跟随。
- 高度通知发生在 UIKit 提交新内容尺寸之前。第一轮新增 UI 测试确实检出长回复末尾不可见；修正为下一次主队列执行滚动，并再次检查是否仍允许跟随。最终该回归通过。
- 分段到达的 Markdown 开头标记和代码结束围栏原来会短暂进入正文/代码，再被解析器移除。生成期间暂缓不完整标记；停止或完成后按原始全文正常解析，并使缓存包含生成状态，避免结束时漏掉仍未闭合的内容。

这些是代码中确认的渲染缺陷与风险，并非对用户真机现场闪烁的逐帧复现。

## 验证

环境：Xcode 26.3、iOS 26.3 模拟器 `PotatoAttachmentsVerification`。使用隔离的 `--ui-testing` 沙盒和 URLProtocol 合成 SSE，不发送真实模型请求。

- 201 项单元测试通过，含新增 5 项：UIHostingController 初次布局/重新创建时的全文高度、已有文字只增量动画、分段块标记、分段闭合围栏、结束时解除暂缓并更新缓存。
- 长回复 UI 回归通过：12 段突发文本、逐字代码围栏/代码、中文及组合字符、完成后末尾可见、回看与恢复追底。
- 30 条历史消息的暂停/恢复追底 UI 回归通过。
- 停止、重试、版本切换和历史搜索由 `PrototypeTests/testNewChatKeyboardStreamingStopAndHistory` 覆盖。
- `git diff --check` 通过。实际检查了 [流式代码块截图](streaming-code.png) 与 [完成后截图](streaming-complete.png)。

第一轮日志 [first-tests.log](verification/first-tests.log) 包含 201 项单元测试通过，以及滚动时机修正前的两处 UI 可见性失败；最终两项滚动 UI 测试见 [streaming-ui-tests.log](verification/streaming-ui-tests.log)。停止/重试最终复核见 [stop-tests.log](verification/stop-tests.log)。

本轮已完成 TestFlight 内测分发，尚未进行用户 iPhone 真机验收或 FPS 测量。普通长回复及 30 条历史已经验证；改用实际布局后，极大量历史的首屏布局性能尚未量测。
