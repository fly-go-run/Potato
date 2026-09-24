# 输入框高度 · 0.2.2（2026092001）

状态：2026-09-20 已发布至既有 TestFlight「个人内测」组，确认 `IN_BETA_TESTING`、`testing: true`，中文更新说明读回一致。Apple 构建 ID `755388c7-93be-4d03-bab6-afe3eb737d43`。可在 TestFlight 更新。

相比已发布的 2026091901，只有 ComposerTextInput.swift、WorkspaceView.swift、RemoteView.swift 三处输入框最小高度变更，以及项目构建号更新。本机与远程聊天最小编辑区高度设为 40 pt。

## 验证

- 196 项 iOS 全量单元测试通过，0 失败。
- 4 项输入与键盘 UI 回归通过，0 失败；首页、键盘和远程键盘截图已检查。
- Release 真机签名归档成功；版本、Bundle ID、iPhone-only、加密声明与隐私清单已核对。
- 归档前后源码摘要一致；真机安装验收未执行。

归档：`native/potato-ios/build/distribution/PotatoMobile-0.2.2-2026092001.xcarchive`。

目标：Apple App `6811367042`（`com.potato.iphone.prototype`），TestFlight 既有「个人内测」组 `fbbe4f4e-56a2-4ba2-8298-5d80bc4ffe4d`。不部署 Worker 或桌面，不提交外部或正式审核。

日志：`/tmp/potato-release-2026092001-tests.log`、`/tmp/potato-release-2026092001-archive.log`。UI 结果：`/tmp/potato-composer-height-20260920.xcresult`。

详见 [源码摘要](source-sha256.json)、[相对上一版变化](changes-from-2026091901.json)、[测试摘要](tests.json)、[归档检查](archive-check.json)、[更新说明](notes.txt)。

[上传结果](upload.json) · [内测分发与说明回读](distribution.json)
