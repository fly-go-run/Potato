# iPhone 分发状态

2026-09-14 **0.2.2 (2026091401)** 已通过命令行/API 上传并加入既有「个人内测」组，状态 **Testing**，中文说明读回一致。TestFlight → Potato Remote → 更新。优化模型目录缓存与后台刷新，移除模型面板首页关闭按钮，包含当前工作树远程对话布局、任务状态和审批改进。全量 iOS 单元测试 129 项与模型 UI 测试 9 项通过。见 [发布与验证](../../docs/design/iphone/model-picker-cache-20260914/README.md)。后续使用本机 `potato-ios-release` skill；Developer API 密钥、本机分发证书和描述文件已配置，普通发布无需浏览器。

2026-09-13 **0.2.2 (2026091306)** 已于 20:41 上传 Apple，处理完成并加入既有「个人内测」组（1 位测试者），状态 **Testing**、有效期 90 天，中文说明已保存。TestFlight → Potato Remote → 更新。Python 已作为与联网搜索、历史检索平级的模型工具，自动执行并返回日志、图表和文件；历史检索仍遵守记忆开关。线上真实模型自主执行两次 Python、生成 PNG/PDF 验证通过。见 [代码工具发布与验证](../../docs/design/iphone/automatic-code-tool-20260913/README.md)。

2026-09-13 **0.2.2 (2026091305)** 已于 20:03 通过 Xcode Organizer 上传 Apple，Apple 处理完成，已加入既有「个人内测」组（1 位测试者），状态 **Testing**、有效期 90 天，中文更新说明已保存。可在 TestFlight → Potato Remote → 更新。新增跨对话检索与长期记忆；对应 Worker 已上线，真实 R2/E2B/模型合成数据检索及记忆增改/忘记验证通过。首次安装仍显示本地「周末计划」示例，升级恢复原有数据。进度见 [记忆功能发布记录](../../docs/design/iphone/cross-chat-recall/release-2026091305/release.md)。

2026-09-13 最新 **0.2.2 (2026091304)** 已于 17:57 通过 Xcode Organizer 上传 Apple，处理完成后已加入既有「个人内测」组，状态 **Testing**、有效期 90 天、1 位测试者；中文更新说明已保存。本版降低点击语音后等待连接才能说话的延迟，连接期间暂存开头音频，并改善慢连接、提前结束和取消处理。iPhone 可通过 TestFlight → Potato Remote → 更新。发布与验证见 [语音启动优化发布记录](../../docs/design/iphone/voice-startup-20260913/release.md)。

2026-09-13 新版 **0.2.2 (2026091303)** 已于 16:28 通过 Xcode Organizer 的 TestFlight Internal Only 成功上传 Apple，Apple 处理已完成，已加入既有「个人内测」组（1 位测试者），状态 **Testing**、有效期 90 天，中文更新说明已保存。Apple 构建 ID `4d14a686-b2a6-452e-bda4-e3051df7fbaa`。iPhone 可通过 TestFlight → Potato Remote → 更新。包含回复操作精简、换模型重试、精简云端目录及准确思考档位。归档为 `build/distribution/PotatoMobile-0.2.2-2026091303.xcarchive`，检查见 [release-check.json](../../docs/design/iphone/model-capabilities-20260913/release-check.json)。初次命令行上传因无法使用账号而在上传前失败；随后经 Xcode 成功上传，无需重复上传本构建。

2026-09-13 云端模型构建：**0.2.2 (2026091302)** 已完成原生 Release 归档与签名检查，Cloudflare 服务已上线，真实 iOS 模拟器登录、DeepSeek/sub2api 回复、重启恢复及退出验证通过。已于 15:12 通过 Xcode Organizer 的 TestFlight Internal Only 成功上传 Apple；15:17 确认已加入「个人内测」组，状态 **Testing**、有效期 90 天、1 位现有测试者。中文更新说明已保存，iPhone 可通过 TestFlight → Potato Remote → 更新后进入「设置 → 云端模型」登录。见 [云端模型记录](../../docs/design/iphone/cloud-models/README.md)。

2026-09-13 已发布：**0.2.2 (2026091301)** 于 13:16 成功上传 Apple，并于 15:04 加入「个人内测」组，状态 **Testing**、有效期 90 天。中文测试说明已保存，可通过 iPhone TestFlight → Potato Remote → 更新。见 [本轮发布记录](../../docs/design/iphone/simple-home-models/release.md)。

2026-09-12 最新：Potato Remote **0.2.1 (2026091203)** 已完成 Apple 处理并加入个人内测组，构建状态为 **Testing**。App Store Connect 测试者页显示 iPhone 已安装 0.2.1。本版修复 Remote 消息布局、过程折叠与回复操作，见 [修复及验证记录](../../docs/design/iphone/remote-control/reply-fix-20260912/README.md)。真机远程任务操作与网络恢复仍待验收。

## 0.2.1 更新记录

- 2026-09-12 23:29:47（北京时间）上传成功，日志 `/tmp/potato-reply-upload-2026091203.log`。
- 归档：`build/distribution/PotatoMobile-0.2.1-2026091203.xcarchive`，已核对版本、仅 iPhone 设备类型及隐私清单。
- 个人内测组现有 1 位测试者、2 个构建；0.2.1 显示 Testing、90 天有效期，测试说明已保存。未来构建自动分发设置保持关闭。
- 3 项单元测试和 1 项原生 UI 测试通过，六张截图逐张检查。现有 Mac 与 Worker 兼容，无需重新安装 Mac。
- 更新入口：iPhone 的 TestFlight → Potato Remote → 更新；已安装本版则直接打开。

## 安装

在 iPhone 安装 Apple TestFlight，打开 Apple 发来的测试邀请邮件，点击 View in TestFlight，接受邀请并安装 **Potato Remote**。内部测试使用邮件邀请；未创建公开邀请链接，App Store Connect 管理页不是安装链接。

安装后打开侧栏 → 远程，登录与电脑相同的 Cloudflare 账号。电脑 Potato 需保持运行、联网。2026-09-12 用户安装并登录 iPhone 后，已按用户要求更新本机 Mac 客户端、完成同账号登录并开启远程访问，Mac 显示“已连接，等待手机操作”。

## 0.2.0 首次分发记录

- 应用：Potato Remote（Potato 名称已被占用），Apple ID `6811367042`。
- 版本：`0.2.0`，构建：`2026091202`，仅 iPhone，最低 iOS 17。
- Bundle ID：`com.potato.iphone.prototype`，付费开发团队：`ZMLCWDNFSH`。
- 内部组：个人内测（`fbbe4f4e-56a2-4ba2-8298-5d80bc4ffe4d`），1 位账号所有者、1 个构建；不自动分发未来构建。
- 上传：2026-09-12 22:16（北京时间）成功，日志 `/tmp/potato-testflight-upload-after-app.log`。Apple 已完成处理；无需重复上传同构建。
- [TestFlight 管理页](https://appstoreconnect.apple.com/teams/2b4712b4-b6bc-437e-a154-eb5d72362d9a/apps/6811367042/testflight/groups/fbbe4f4e-56a2-4ba2-8298-5d80bc4ffe4d/builds)。

## 0.2.0 本地产物与验证

- 分发签名 IPA：`build/distribution/app-store-2026091202/PotatoMobile.ipa`。
- SHA-256：`fc6f22cfcbd1d8a245aa6370b4e100cedbd331221fb7fb151df138f546195530`。
- 归档：`build/distribution/PotatoMobile-0.2.0-2026091202.xcarchive`。
- 上传配置：`build/distribution/ExportOptions-TestFlight.plist`。

这是 App Store Connect 分发包，不能通过 Safari 下载后直接安装。不要将模拟器产物、未签名归档或 `2026091201` 中间包用于安装分发。

归档、导出和上传通过，已核对 IPA 中版本、iPhone-only 设备类型、嵌入描述文件和隐私清单。补充 UserDefaults 本应用偏好设置用途 `CA92.1`；加密依赖系统 HTTPS/WSS、Keychain 与系统 SHA-256，声明不使用非豁免加密。本次完成内部测试分发，未提交 App Store 正式发布或外部 Beta 审核，未声称完成真机、蜂窝网络或完整 VoiceOver 验收。

参考：[Apple TestFlight 内部测试](https://developer.apple.com/help/app-store-connect/test-a-beta-version/add-internal-testers)、[必需原因 API](https://developer.apple.com/documentation/bundleresources/describing-use-of-required-reason-api)、[系统加密声明](https://developer.apple.com/documentation/security/complying-with-encryption-export-regulations)。
