# iPhone 0.2.2（2026091702）发布记录

**已发布至 TestFlight「个人内测」组，可在 TestFlight → Potato Remote → 更新。** 2026-09-17T09:09:45+08:00 读回确认 `VALID`、`IN_BETA_TESTING`、`testing: true`，中文更新说明一致。Apple 构建 ID：`f79fbb0f-faeb-456b-809e-2a40b9172678`。见 [分发回读](distribution.json)。

本次发布代码多语言语法高亮、移除手动运行按钮及中间页、Markdown 嵌套围栏展示改进。保留模型通过对话调用 Python 沙箱与历史结果。没有部署桌面 GPUI、Rust 核心或 Worker。

- 全量原生单元测试 190 项通过；复用相同功能源码的 5 项原生 UI 回归（Python 明暗色、Markdown、复制及无运行入口；模型工具结果/文件/停止和重启恢复）。见 [测试摘要](tests.json)。
- 相较上次已发布的 2026091701，仅上述功能、测试夹具及构建配置变化，见 [差异](changes-from-2026091701.json)。110 个输入文件的 [源码摘要](source-sha256.json) 在归档前记录、上传前核验一致。
- Release 真机签名归档通过；版本、iPhone-only、最低 iOS 17、语言资源、隐私清单与加密声明均已检查，见 [归档检查](archive-check.json)。
- 归档：`native/potato-ios/build/distribution/PotatoMobile-0.2.2-2026091702.xcarchive`。
- 发布全量单元测试：`/tmp/PotatoRelease2026091702Tests.xcresult`。
- 日志：`/tmp/potato-release-2026091702-tests.log`、`/tmp/potato-release-2026091702-archive.log`、`/tmp/potato-release-2026091702-upload.log`；完整上传日志在归档旁 `.xcarchive.export/upload.log`。
- [上传回执](upload.json) · [中文更新说明](notes.txt) · [界面截图与实现](../README.md)。

本次仅面向既有个人内测组，不创建测试组或新增测试者，不提交正式或外部审核。真实 E2B 执行和完整 VoiceOver 真机验收未在本轮重跑。
