# 流式闪动修复 · 0.2.2（2026092101）

状态：2026-09-21 已发布至既有 TestFlight「个人内测」组，确认 `IN_BETA_TESTING`、`testing: true`，中文更新说明读回一致。Apple 构建 ID `741e5c49-6075-472a-97b0-bdfb3e442b1d`。可在 TestFlight 更新。

相比 2026092001，仅变更本机聊天的流式显示、Markdown 解析、布局与自动追底，及相关测试和构建号。原工作树的其他 iOS 文件与上一版发布摘要一致。本次不部署桌面或 Worker。

## 验证

- 最终发布源码的 201 项 iOS 全量单元测试通过，0 失败。
- 同一应用源码的 3 项 UI 回归通过：长回复与逐字代码围栏、回看/恢复追底、停止/重试/版本/历史。发布阶段仅调整构建号，复用这些 UI 证据。
- Release 真机签名归档通过，已核对 Bundle ID、版本、iPhone-only、隐私清单及加密声明。
- 归档前后源码 SHA-256 一致。用户真机闪动与超长历史性能尚未实机验收。

归档：`native/potato-ios/build/distribution/PotatoMobile-0.2.2-2026092101.xcarchive`。

目标：Apple App `6811367042`，Bundle ID `com.potato.iphone.prototype`，既有内部组「个人内测」`fbbe4f4e-56a2-4ba2-8298-5d80bc4ffe4d`。仅内部 TestFlight 分发。

日志：`/tmp/potato-release-2026092101-tests.log`、`/tmp/potato-release-2026092101-archive.log`，上传日志路径见 [上传结果](upload.json)。

[源码摘要](source-sha256.json) · [相对上一版变化](changes-from-2026092001.json) · [测试摘要](tests.json) · [归档检查](archive-check.json) · [中文说明](notes.txt) · [Apple 状态](status.json)

[内测分发与更新说明回读](distribution.json)
