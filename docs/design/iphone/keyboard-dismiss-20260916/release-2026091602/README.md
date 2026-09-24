# TestFlight 0.2.2（2026091602）

2026-09-16。**已发布至既有「个人内测」组，可在 TestFlight → Potato Remote → 更新。** 北京时间 12:26 确认 Testing，中文更新说明读回一致。

## 本次内容

- 本机聊天首页及消息区轻点空白处收起键盘。
- 首页即使内容不足一屏，也可上下拖动收起键盘；消息区复用现有原生滚动手势收起键盘。
- 保留未发送草稿、再次点击输入框编辑，以及侧栏和横向内容的手势。

与已发布的 2026091601 源码摘要逐项对比，功能源码只改变 WorkspaceView.swift，新增测试位于 SidebarUITests.swift。此前 Liquid Glass 与远程界面改动沿用已发布版本。营销版本保持 0.2.2，构建号更新为 2026091602。

## 验证

- iPhone 17 / iOS 26.3 模拟器 6 项 UI 回归全部通过：空白页点击、上下拖动、消息区点击与拖动、侧栏收键盘保留草稿、输入编辑手势、代码与表格横向滚动。日志 `/tmp/potato-keyboard-tests-2.log`。
- 发布前全量 iOS 单元测试 129 / 129 通过。结果 `/tmp/potato-release-unit-2026091602.xcresult`；日志 `/tmp/potato-release-unit-2026091602.log`。
- Release 真机签名归档成功，日志 `/tmp/potato-archive-2026091602.log`。
- 归档后源码摘要一致；版本、Bundle ID、iPhone-only、隐私清单、非豁免加密声明检查通过。见 [源码摘要](source-manifest.json)和[归档检查](archive-check.json)。
- Xcode 26.3 / iOS 26.2 SDK。键盘交互尚未进行用户 iPhone 真机验收。

## 产物与分发

归档：`native/potato-ios/build/distribution/PotatoMobile-0.2.2-2026091602.xcarchive`。

更新说明：[notes.txt](notes.txt)。上传日志：归档同名 `.export/upload.log`。

沿用 App ID 6811367042、Bundle ID com.potato.iphone.prototype、既有个人内测组；未部署桌面或 Worker。

Apple 构建 ID：`4e49149a-407e-4a4e-9dbd-43aaf0bda0d1`。最终 processingState 为 VALID、internalBuildState 为 IN_BETA_TESTING，inGroup / testing / notesVerified 均为 true。见 [上传结果](upload.json)、[最终状态](status.json)和[分发读回](distribution.json)。
