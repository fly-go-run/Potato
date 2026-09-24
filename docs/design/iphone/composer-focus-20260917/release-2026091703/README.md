# iPhone 0.2.2（2026091703）发布记录

**已发布至既有 TestFlight「个人内测」组，可在 TestFlight → Potato Remote → 更新。** 2026-09-17T19:04:20+08:00 读回确认 `VALID`、`IN_BETA_TESTING`、`testing: true`，中文说明一致。Apple 构建 ID `f7c5fbdc-189d-4725-b27d-09ad56a51f9e`。见 [分发回读](distribution.json)。

本版修复本机聊天与远程聊天输入框点击范围：四周内边距、编辑器与工具栏间隙、按钮间空白均可唤起键盘；排除按钮区域（含禁用按钮），保留 UITextView 原生编辑手势。录音状态保留原有语音操作。

- 相较上一已发布构建 2026091702，功能源码仅更改 ComposerTextInput、WorkspaceView、RemoteView，并增加 VoiceInteractionTests 回归；构建号同步升至 2026091703。
- 全量 190 项 iOS 单元测试通过，结果 `/tmp/PotatoRelease2026091703Tests.xcresult`。
- 2 项原生 UI 回归通过：本机/远程的四处留白、禁用发送按钮、语音按钮、本机附件按钮、草稿保留。
- Release 真机签名归档成功；版本、iPhone-only、隐私清单及加密声明检查通过。
- 归档前记录源码 SHA-256，上传前复核一致。见 `source-sha256.json`、`archive-check.json`、`tests.json`。
- 归档：`native/potato-ios/build/distribution/PotatoMobile-0.2.2-2026091703.xcarchive`。
- 测试/归档/上传日志：`/tmp/potato-release-2026091703-{tests,archive,upload}.log`；详细上传日志在归档旁 `.xcarchive.export/upload.log`。

本次仅发布 iPhone 到既有个人内测组；不部署桌面或 Worker。模拟器验证不等同真机触控验收。
