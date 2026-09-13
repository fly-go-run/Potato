# iOS 本机思考过程修复 · 2026-09-13

已接通本机 Chat Completions 的公开 `reasoning_content`，并修复搜索 Worker 丢弃该字段的问题。思考内容默认折叠，用户可展开阅读；只有实际收到内容后才出现思考行，正文到来后结束活动指示。改动仅在工作树，尚未发布 iOS 或部署 Worker。首页三个概念仍待选择，模型/思考设置及远程过程状态仍未完成。

## 行为与协议

- 同一 SSE 帧中的思考和正文都保留；遇到 `finish_reason: length`，先保留该帧内容，再显示输出上限错误。中文通过真实 `URLSession.bytes` 的逐字节 fixture 验证。
- 收到首个思考字段才开始计时；搜索开始时结束当前区间，后续再收到思考时累加新区间。时间是客户端观察到的区间，包含该阶段网络等待，不是供应商计算耗时。
- 正文、完成、停止、失败均结束当前思考活动。搜索期间不同时显示“正在思考”。仅有思考而无正文的终止流保留内容并提示失败，不冒充完整答案。
- 展开思考暂停自动追底；用户点击回到最新后恢复。减弱动态效果分支使用静态指示，进入后台停止视图定时更新；没有生成假思考或逐字延迟正文。
- 思考与回复版本一起保存，停止及失败后仍可阅读。重启恢复未完成回复时以最后收到内容的时间冻结计时，不把离线时间计入；旧版没有思考字段的历史可正常解码。
- 普通下一轮聊天请求只回放可见正文，思考内容不会拼进用户消息或历史正文。Worker 内部工具调用保留模型原有的思考回放字段。
- 手机把正文与思考合计限制在 2,000,000 UTF-8 字节。搜索 Worker 保留原每轮字段限制，并在转发前检查跨搜索轮的同一累计预算；工具参数不作为思考发给手机，取消继续传递至上游。

协议参考：[DeepSeek 官方流式思考示例](https://api-docs.deepseek.com/guides/thinking_mode_api_example_streaming/)。这一实现针对既有 OpenAI Chat Completions 兼容接口，不构成 Anthropic Messages 或其他原生 API 的通用适配。Worker 流和内存边界按 [Cloudflare Workers 最佳实践](https://developers.cloudflare.com/workers/best-practices/workers-best-practices/)检查，并读取当前官方 workers-types 5.20260911.1 的流类型。

## 验证

Xcode 26.3，iOS 26.3.1（23D8133）模拟器。测试使用独立沙盒及 URLProtocol 合成流，没有调用真实模型、发送远程任务或改动线上配置。

| 检查 | 结果及覆盖 |
| --- | --- |
| iPhone 17 | 83 项通过：全部 76 项单元测试、4 条思考页面流程、3 条侧栏/代码表格/系统返回回归 |
| iPhone SE 第三代 | 5 条页面流程通过：同样 4 条思考流程，加新建、键盘、流式停止、历史原有流程 |
| 补录 | 在同一 Debug 构建重放 1 条已通过的 SE 流程，确认动态状态与重启恢复；不计入独立用例总数 |
| Worker | 31 项测试通过，TypeScript 检查通过；包括同帧数据、超限不转发、跨搜索轮中文字节预算、取消上游 |
| Release | simulator Release 构建通过，产物未检出测试 fixture 主机或类型；不是 TestFlight 或真机签名验收 |

上述最终运行均 0 失败、0 跳过。SE 用例与 iPhone 17 有重叠，不能相加称为 88 个独立用例。

- [iPhone 17 摘要](../../../../native/potato-ios/qa/ios-audit-20260913/reasoning-fix/iphone17-summary.json)、[日志/命令](../../../../native/potato-ios/qa/ios-audit-20260913/reasoning-fix/iphone17-tests.log)、[截图索引](../../../../native/potato-ios/qa/ios-audit-20260913/reasoning-fix/iphone17-attachments/manifest.json)。原始结果 `/tmp/potato-reasoning-final-4.xcresult`。
- [iPhone SE 摘要](../../../../native/potato-ios/qa/ios-audit-20260913/reasoning-fix/iphonese-summary.json)、[日志/命令](../../../../native/potato-ios/qa/ios-audit-20260913/reasoning-fix/iphonese-tests.log)、[截图索引](../../../../native/potato-ios/qa/ios-audit-20260913/reasoning-fix/iphonese-attachments/manifest.json)。原始结果 `/tmp/potato-reasoning-se-5.xcresult`。
- [Worker 测试](../../../../native/potato-ios/qa/ios-audit-20260913/reasoning-fix/worker-tests.log)、[类型检查](../../../../native/potato-ios/qa/ios-audit-20260913/reasoning-fix/worker-types.log)、[Release 构建](../../../../native/potato-ios/qa/ios-audit-20260913/reasoning-fix/release-build.log)、[产物检查](../../../../native/potato-ios/qa/ios-audit-20260913/reasoning-fix/release-check.json)、[验证源码 SHA-256](../../../../native/potato-ios/qa/ios-audit-20260913/reasoning-fix/source-hashes.json)。

初次 UI 检查错误地按 OtherElement 查找 SwiftUI 状态行；从实际无障碍层级修正为精确可见文本查询后通过，未移除任何状态切换断言。[早期失败日志](../../../../native/potato-ios/qa/ios-audit-20260913/reasoning-fix/first-ui-attempt.log)保留。首次单元测试的版本恢复用例未等待计划保存，已按现有测试模式显式持久化后读取。Node 首次运行受到沙盒 loopback EPERM 限制，获自动授权后同一测试命令全部通过。

## 实际画面

已打开核对两种尺寸的思考、搜索、正文和停止截图；以下是应用测试截图，不是 AI 概念图。测试模型名 `reasoning-fixture` 只用于说明合成数据来源。

![iPhone SE 思考展开](../../../../native/potato-ios/qa/ios-audit-20260913/reasoning-fix/iphonese-attachments/80B753CA-FEF8-4232-B50F-A7530DFBDE47.png)

![iPhone SE 思考结束后输出正文](../../../../native/potato-ios/qa/ios-audit-20260913/reasoning-fix/iphonese-attachments/57DEF08C-DD0C-4DAB-B1D4-A06163D5CF99.png)

[思考→正文→重启恢复录屏](../../../../native/potato-ios/qa/ios-audit-20260913/reasoning-fix/thinking-to-reply-se.mp4)是同一最终 Debug 构建在 `/tmp/potato-reasoning-video-6.xcresult` 运行时的真实模拟器画面，仅裁去启动前等待，无速度修改或合成动画。[补录测试日志](../../../../native/potato-ios/qa/ios-audit-20260913/reasoning-fix/video-tests.log)。

![录屏连续状态取样](../../../../native/potato-ios/qa/ios-audit-20260913/reasoning-fix/flow-frames.png)

## 尚未覆盖

远程手机页面通过 core 快照展示执行过程，未使用这条本机 SSE 实现；其断连、陈旧状态和恢复动画仍需单独修复与验收。模型目录、能力声明、思考档位及远程不可变发送覆盖也仍待实现。搜索思考转发只有本地 Worker 测试证据，线上需要后续部署及联调。完整 VoiceOver、系统级 Reduce Motion 手工操作、动态大字号、最低 iOS 17 和真机运行未由本轮测试证明。
