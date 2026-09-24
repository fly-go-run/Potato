# iPhone 0.2.2（2026091701）发布记录

**已发布至 TestFlight「个人内测」组，可在 TestFlight → Potato Remote → 更新。** 2026-09-17T08:28:36+08:00 读回确认 `VALID`、`IN_BETA_TESTING`、`testing: true`，中文更新说明一致。Apple 构建 ID：`9c705c96-33cf-4436-9e3d-2e2f3c50f879`。见 [分发回读](distribution.json)。

本次发布当前 iPhone 工作树，包含界面语言（跟随系统、英文、简体中文）、对话内排队消息、远程列表缓存、远程语音统一和精简加载提示。桌面 GPUI、Rust 核心及 Worker 未随本次部署。远程队列须由兼容电脑声明 `outbox_protocol=1`，并由中继放行 outbox 操作；旧电脑保持原有补充消息行为。

- 原生验证：185 项单元测试、1 条语言完整 UI 流程、2 条队列 UI 流程，全部通过，0 跳过、0 失败。见 [测试摘要](tests.json)。
- 语言 UI 验证预览/保存/取消/重启、中文和英文系统跟随，以及未发送草稿保留。
- 队列 UI 验证连续发送、消息菜单编辑/删除/打断、派发后去重，以及中文长消息完整显示。
- [104 个源码输入摘要](source-sha256.json)在归档及上传前核对一致；与上一版 iOS 输入相比 56 项新增或变化，主要为本地化及上述远程交互，见 [差异](changes-from-2026091608.json)。
- [归档检查](archive-check.json)确认 iPhone-only、iOS 17 最低版本、语言资源、隐私清单和加密声明。
- 归档：`native/potato-ios/build/distribution/PotatoMobile-0.2.2-2026091701.xcarchive`。
- 测试结果包：`/tmp/PotatoRelease2026091701Tests.xcresult`。
- 日志：`/tmp/potato-release-2026091701-{tests,archive,upload}.log`。完整上传日志在归档旁 `.xcarchive.export/upload.log`。
- [上传回执](upload.json) · [中文更新说明](notes.txt)。

仅分发到既有「个人内测」组，不创建测试组、邀请测试者或提交正式/外部审核。
