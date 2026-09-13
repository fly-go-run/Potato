# 0.2.2 (2026091305) 记忆与历史发布

2026-09-13 20:03（北京时间），Xcode Organizer 确认 PotatoMobile 0.2.2 (2026091305) uploaded。Apple 处理 Complete，随后已加入既有「个人内测」组（1 位测试者），构建状态 Testing、有效期 90 天，中文更新说明显示 Saved。Apple 构建 ID 为 `94a26e95-f2c7-4cb5-a969-d61ad3cd20eb`。手机可在 TestFlight → Potato Remote 中更新。

- 内容：跨对话检索、原消息来源跳转、长期记忆手动增改/忘记、独立自动记忆开关、对话排除和增量同步。
- 入口：右上角「更多 → 记忆与历史」；跨对话检索默认关闭。先连接云端模型，再按需开启。
- 首次启动：无本地记录时仍创建「周末计划」本地示例并展开工作文稿；覆盖更新恢复会话和设置。这次没有改变首页初始化行为。
- Worker：`1e5b77af-be4e-4fd1-abf1-8b52ce3e4f63`；前一版本 `d54f067f-61bb-4593-9ab8-5c52dbe7fdaf`，保留线上变量。
- 验证：Worker 53 项测试、TypeScript 检查及部署打包通过；iOS 114 项单元测试和 2 项记忆页面测试通过。Release 签名、版本、仅 iPhone、最低 iOS 17、隐私清单与 Debug fixture 排除检查通过。
- 线上：真实 R2 + E2B + DeepSeek 检索合成对话成功，答案和来源匹配；手动记忆新增、编辑、忘记及对话排除通过，已清理合成数据并撤销验证会话。未上传用户真实历史。
- 归档：`native/potato-ios/build/distribution/PotatoMobile-0.2.2-2026091305.xcarchive`。
- 仍需真机升级、iOS 17 与 VoiceOver 人工验收。首版为关键词检索，不承诺语义召回或完整跨设备原文同步。

证据：[构建检查](release-check.json)、[源文件指纹](source-sha256.json)、[线上检查](online-check.json)、[iOS 测试](ios-tests.log)、[Worker 测试](worker-tests.log)。

[App Store Connect 构建详情](https://appstoreconnect.apple.com/teams/2b4712b4-b6bc-437e-a154-eb5d72362d9a/apps/6811367042/testflight/ios/94a26e95-f2c7-4cb5-a969-d61ad3cd20eb)。未更改组成员或未来构建自动分发设置，未提交 App Store 正式发布。
