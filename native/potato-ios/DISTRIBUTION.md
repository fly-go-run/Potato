# iPhone 分发状态

2026-09-25 **0.2.2（2026092502）** 已发布至既有 TestFlight「个人内测」组，确认 **Testing**，中文说明读回一致。合入回复过程顺滑化（`d51b0bb3`、`e75d6000`，来自 `feat/process-track-codex`）：末尾标记从发送到结束固定一处、过程摘要去掉次数与秒数、流结束时文字平滑补完、回复操作图标减轻、本机对话去掉顶部标题、远程任务标题栏不透明；包含上一版新图标。源码为合并提交 `7ef24d91`（分支 `feat/ios-brand-warmth`，不含主工作区未提交的远程改动），222 项 iOS 单元测试、代码执行与跨对话检索 4 项 UI 回归及 Release 签名归档通过。Apple 构建 ID `c32dd80d-c614-41ad-b6fd-66a0e0e06500`。说明见 `testflight-notes-2026092502.zh-Hans.txt`。

2026-09-25 **0.2.2（2026092501）** 已发布至既有 TestFlight「个人内测」组，确认 **Testing**，中文说明读回一致。新图标「探头·夜」（土豆像月亮从夜空探出头，屏幕式眼睛与嫩芽；深色同图、着色为灰阶），首页角色与嫩芽叶形随之统一；浅色下设置类表单与列表改暖色背景，深色保持系统原样；登录过期等警告改为红色。源码 `58c02b6c`（分支 `feat/ios-brand-warmth`），221 项 iOS 单元测试与 Release 签名归档通过。Apple 构建 ID `e5a4451b-41ba-4dad-b842-4ae933fd0f51`。说明见 `testflight-notes-2026092501.zh-Hans.txt`。

2026-09-24 **0.2.2（2026092402）** 已发布至既有 TestFlight「个人内测」组，确认 **Testing**，中文说明读回一致。新极简图标（奶油底发芽土豆，含深色与着色版本）；主操作改用焦糖品牌色；回复进行中显示摇摆嫩芽，首页土豆可点按晃动；资料库、对话列表、远程新对话与搜索空状态换上扁平插画；中文输入提示改为“问问土豆”。源码基于 `83350be0`（分支 `feat/ios-brand-warmth`，不含主工作区未提交的远程改动），221 项 iOS 单元测试与 Release 签名归档通过。Apple 构建 ID `f78d8a72-fcc2-4b0b-a115-9db4fc556f4d`。说明见 `testflight-notes-2026092402.zh-Hans.txt`。

2026-09-24 **0.2.2（2026092401）** 已发布至既有 TestFlight「个人内测」组，确认 **Testing**，中文说明读回一致。设置页合并为“通用”，记忆移入设置，新增新对话默认模型与删除所有对话；回复过程按时间顺序穿插显示，进行中文字扫光、发送后呼吸圆点替代转圈；图标统一单色；管理员可管理云端模型。源码 `c4e355d2`，221 项 iOS 单元测试及相关 UI 回归、Release 签名归档通过。Apple 构建 ID `75f8351d-00bf-4f84-9977-28f72ce32341`。说明见 `testflight-notes-2026092401.zh-Hans.txt`。

2026-09-21 **0.2.2（2026092101）** 已发布至既有 TestFlight「个人内测」组，确认 **Testing**，中文说明读回一致。修复手机直接聊天的流式正文重建、长回复追底与分段 Markdown 标记造成的布局跳变。201 项全量单元测试、3 项相关 UI 回归及 Release 签名归档通过。Apple 构建 ID `741e5c49-6075-472a-97b0-bdfb3e442b1d`。见 [发布记录](../../docs/design/iphone/streaming-flicker-20260921/release-2026092101/README.md)。

2026-09-20 **0.2.2（2026092001）** 已发布至既有 TestFlight「个人内测」组，确认 **Testing**，中文说明读回一致。本机与远程聊天最小编辑区增至 40 pt，增加文字与底部按钮之间的留白。196 项 iOS 单元测试、4 项输入与键盘 UI 回归及 Release 签名归档通过。Apple 构建 ID `755388c7-93be-4d03-bab6-afe3eb737d43`。见 [发布记录](../../docs/design/iphone/composer-height-20260920/release-2026092001/README.md)。

2026-09-19 **0.2.2（2026091901）** 已发布至既有 TestFlight「个人内测」组，确认 **Testing**，中文说明读回一致。新增最近照片快捷附件面板，轻点直接添加，每条消息最多 20 个附件，并移除底部操作说明。配套 Worker 已部署 100% 流量；196 项 iOS 单元测试、4 项附件 UI 场景、92 项隔离 Worker 回归及 Release 签名归档通过。Apple 构建 ID `ac8eea2a-1380-4114-8f61-82161ee4e1e1`。见 [发布记录](../../docs/design/iphone/attachments-20260919/release-2026091901/README.md)。

2026-09-17 **0.2.2（2026091703）** 已发布至既有 TestFlight「个人内测」组，确认 **Testing**，中文说明读回一致。本机及远程聊天输入框四周留白、按钮间隙可唤起键盘，按钮（含禁用发送）不误触。190 项原生单元测试、2 项输入触控 UI 回归及 Release 真机签名归档通过。Apple 构建 ID `f7c5fbdc-189d-4725-b27d-09ad56a51f9e`。见 [发布记录](../../docs/design/iphone/composer-focus-20260917/release-2026091703/README.md)。

2026-09-17 **0.2.2（2026091702）** 已发布至既有 TestFlight「个人内测」组，确认 **Testing**，中文说明读回一致。新增多语言代码高亮及深浅色适配，移除手动运行按钮与中间页，保留通过对话调用沙箱；改善 Markdown 嵌套围栏展示。全量 190 项原生单元测试、5 项相关 UI 回归与 Release 真机签名归档通过。Apple 构建 ID `f79fbb0f-faeb-456b-809e-2a40b9172678`。见 [发布记录](../../docs/design/iphone/code-blocks-20260917/release-2026091702/README.md)。

2026-09-17 **0.2.2（2026091701）** 已发布至既有 TestFlight「个人内测」组，确认 **Testing**，中文说明读回一致。新增“跟随系统 / English / 简体中文”，支持保存恢复；包含对话内排队消息、远程目录缓存、语音统一及精简加载提示。远程排队需配套电脑端与中继支持，本次仅发布 iPhone。185 项原生单元测试和 3 条 UI 流程全部通过，Release 真机签名归档成功。Apple 构建 ID `9c705c96-33cf-4436-9e3d-2e2f3c50f879`。见 [发布记录](../../docs/design/iphone/language-20260917/release-2026091701/README.md)。

2026-09-16 **0.2.2 (2026091608)** 已上传并分发至既有「个人内测」组，确认 **Testing**，中文说明读回一致。TestFlight → Potato Remote → 更新。本版将远程页面统一为对话体验：按轮合并回复与过程入口、精简操作、统一顶部和输入组件，移除多余完成提示，改善键盘及大字号断线状态。全量 167 项 iOS 单元测试、相关亮暗色 UI 回归和 Release 真机签名归档通过。Apple 构建 ID `0dec6f88-dec8-47a7-b723-10ba80a525b8`。见 [发布记录](../../docs/design/iphone/remote-conversation-20260916/release-2026091608/README.md)。

2026-09-16 **0.2.2 (2026091607)** 已上传并分发至既有「个人内测」组，确认 **Testing**，中文说明读回一致。TestFlight → Potato Remote → 更新。本版优化流式输出：云端持续推送、手机平滑显示、减少重复保存与 Markdown 解析，保留断线续接与停止恢复。配套 Worker 已部署 100% 流量，真实模型订阅和主动断开后续接通过；原生及 Worker 回归、Release 真机签名归档通过。Apple 构建 ID `8f689613-fde0-4435-816f-9f2cd4176e6d`。见 [发布记录](../../docs/design/iphone/streaming-20260916/release-2026091607/README.md)。

2026-09-16 **0.2.2 (2026091606)** 已上传并分发至既有「个人内测」组，确认 **Testing**，中文说明读回一致。TestFlight → Potato Remote → 更新。新增设置 → 外观 → 亮色 / 暗色 / 自动，保存选择并支持系统跟随，适配主要页面明暗配色。157 项 iOS 单元测试、暗色下 4 项与亮色下 2 项 UI 回归以及 Release 真机签名归档通过。Apple 构建 ID `5b051e6e-bdcc-4ab0-a905-22f4d2000d7c`。见 [发布记录](../../docs/design/iphone/appearance-20260916/release-2026091606/README.md)。

2026-09-16 **0.2.2 (2026091605)** 已上传并分发至既有「个人内测」组，确认 **Testing**，中文说明读回一致。TestFlight → Potato Remote → 更新。设置页支持下拉关闭，顶部显示拖动条；下拉等同取消，修改仍需保存。155 项 iOS 单元测试、3 项设置 UI 回归及 Release 真机签名归档通过。Apple 构建 ID `120829d0-bb13-4127-8399-1d5e54992b6e`。见 [发布记录](../../docs/design/iphone/settings-dismiss-20260916/release-2026091605/README.md)。

2026-09-16 **0.2.2 (2026091604)** 已上传并分发至既有「个人内测」组，确认 **Testing**，中文说明读回一致。TestFlight → Potato Remote → 更新。包含云端异步回复及断网/退出恢复、发送去重和停止恢复，并包含当前资料库改版。配套 Worker 已部署 100% 流量，真实云端断连后同任务完整回复及补读验证通过；155 项 iOS 单元测试、4 条聊天 UI 回归与真机签名归档通过。Apple 构建 ID `1135e5c3-a4ac-40bc-bef0-b71082240886`。见 [发布记录](../../docs/design/iphone/async-replies-20260916/release-2026091604/README.md)。

2026-09-16 **0.2.2 (2026091603)** 已上传并分发至既有「个人内测」组，14:05 确认 **Testing**，中文说明读回一致。TestFlight → Potato Remote → 更新。聊天主线恢复独立思考摘要，轻点查看完整内容；工具步骤采用半屏过程概览与输入/输出详情，图片优先预览、文件卡片交付。全量 136 项 iOS 单元测试和 9 条 UI 回归通过，真机签名归档成功。Apple 构建 ID `73f5b7a9-944b-4090-8aeb-db4affdb428a`。本次仅发布 iPhone 客户端，配套 Worker 改动未部署。见 [发布记录](../../docs/design/iphone/activity-20260916/release-2026091603/README.md)。

2026-09-16 **0.2.2 (2026091602)** 已上传并分发至既有「个人内测」组，12:26 确认 **Testing**，中文说明读回一致。TestFlight → Potato Remote → 更新。本版支持聊天区空白处轻点及上下拖动收起键盘，保留草稿与原有编辑/侧栏手势。129 项 iOS 单元测试及 6 项 UI 回归通过，真机签名归档成功。Apple 构建 ID `4e49149a-407e-4a4e-9dbd-43aaf0bda0d1`。见 [键盘修复发布记录](../../docs/design/iphone/keyboard-dismiss-20260916/release-2026091602/README.md)。

2026-09-16 **0.2.2 (2026091601)** 已通过命令行/API 上传并加入既有「个人内测」组，11:20 确认状态 **Testing**，中文说明读回一致。TestFlight → Potato Remote → 更新。包含系统默认 Liquid Glass、正文延伸至浮动输入区后方、侧栏连续圆角与关闭后点击恢复、当前会话选中及大字号布局优化。发布前全量 iOS 单元测试 129 项通过，原生玻璃界面回归 5 项通过，真机签名归档成功。Apple 构建 ID `8e51c492-178b-43e8-8e94-cad9fddb6bca`。见 [本次发布记录](../../docs/design/iphone/liquid-glass-20260916/release-2026091601/README.md)。

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
