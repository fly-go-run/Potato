# Potato iPhone 原生客户端

2026-09-24 **产品层打磨（工作树，尚未发布）**：全新安装改为空白首页加登录，未登录发送/语音会先引导登录并保留草稿；登录失效显示横幅和"重新登录"，发送失败给出重试；模型显示友好名；设置即时生效，接口、令牌与本地体验移入隐藏的开发者选项；对话自动生成标题、按日期分组、可重命名；对话"…"菜单改为本对话操作。见 [产品层打磨记录](../../docs/design/iphone/product-polish-20260924/README.md)。

同日复核已修复 DeepSeek 标题请求参数冲突、失败回复无法换模型、旧示例重新生成绕过登录三项回归；29 项针对性测试通过。逐项剩余工作及截图见 [复核记录](../../docs/design/iphone/product-polish-20260924/followup-review.md)。

2026-09-21 **0.2.2（2026092101）** 已发布至既有 TestFlight「个人内测」组，确认 **Testing**，中文说明读回一致。修复手机直接聊天的流式正文重建、长回复追底与分段 Markdown 标记造成的布局跳变。201 项全量单元测试、3 项相关 UI 回归及 Release 签名归档通过。Apple 构建 ID `741e5c49-6075-472a-97b0-bdfb3e442b1d`。见 [发布记录](../../docs/design/iphone/streaming-flicker-20260921/release-2026092101/README.md)。

2026-09-20 **0.2.2（2026092001）** 已发布至既有 TestFlight「个人内测」组，确认 **Testing**，中文说明读回一致。本机与远程聊天最小编辑区增至 40 pt，增加文字与底部按钮之间的留白。196 项 iOS 单元测试、4 项输入与键盘 UI 回归及 Release 签名归档通过。Apple 构建 ID `755388c7-93be-4d03-bab6-afe3eb737d43`。见 [发布记录](../../docs/design/iphone/composer-height-20260920/release-2026092001/README.md)。

2026-09-19 **0.2.2（2026091901）已发布至 TestFlight 个人内测组，确认 Testing**。新增快捷附件面板：点「＋」直接浏览最近照片，轻点即加入消息，支持最多 20 个附件、连续添加、原位状态与再次点击移除；保留所有照片、拍照、文件及资料库入口。包含权限回退、导入名额预留和会话隔离。实现与验证见 [快捷附件面板](../../docs/design/iphone/attachments-20260919/README.md)；配套云端已上线，见 [发布记录](../../docs/design/iphone/attachments-20260919/release-2026091901/README.md)。

2026-09-17 **0.2.2（2026091702）** 已发布至既有 TestFlight「个人内测」组，确认 **Testing**，中文说明读回一致。新增多语言代码高亮及深浅色适配，移除手动运行按钮与中间页，保留通过对话调用沙箱；改善 Markdown 嵌套围栏展示。全量 190 项原生单元测试、5 项相关 UI 回归与 Release 真机签名归档通过。Apple 构建 ID `f79fbb0f-faeb-456b-809e-2a40b9172678`。见 [发布记录](../../docs/design/iphone/code-blocks-20260917/release-2026091702/README.md)。

2026-09-17 **0.2.2（2026091701）** 已发布至既有 TestFlight「个人内测」组，确认 **Testing**，中文说明读回一致。新增“跟随系统 / English / 简体中文”，支持保存恢复；包含对话内排队消息、远程目录缓存、语音统一及精简加载提示。远程排队需配套电脑端与中继支持，本次仅发布 iPhone。185 项原生单元测试和 3 条 UI 流程全部通过，Release 真机签名归档成功。Apple 构建 ID `9c705c96-33cf-4436-9e3d-2e2f3c50f879`。见 [发布记录](../../docs/design/iphone/language-20260917/release-2026091701/README.md)。

2026-09-16 远程对话改版已发布至 TestFlight **0.2.2（2026091608）**，既有个人内测组 Testing：按轮整合正文和过程、统一顶部及输入组件、精简操作并移除冗余完成提示。167 项单元测试、亮暗色及键盘/草稿/状态 UI 回归通过，见 [发布记录](../../docs/design/iphone/remote-conversation-20260916/release-2026091608/README.md)。

2026-09-16 流式输出优化已发布至 TestFlight **0.2.2（2026091607）**，既有个人内测组 Testing，配套云端实时订阅已上线。平滑显示中文与 emoji、积压追赶、Markdown 缓存及断线续接已验证，见 [发布记录](../../docs/design/iphone/streaming-20260916/release-2026091607/README.md)。

2026-09-16 外观选择已发布至 TestFlight **0.2.2（2026091606）**，既有个人内测组 Testing：设置 → 外观 → 亮色 / 暗色 / 自动，默认跟随系统。支持预览、保存和取消恢复，聊天、文稿、资料库、远程和模型面板已适配暗色。157 项单元测试、系统暗色下 4 项及亮色下 2 项 UI 回归通过，见 [截图与验证](../../docs/design/iphone/appearance-20260916/README.md)。

2026-09-16 设置页下拉关闭已发布至 TestFlight **0.2.2（2026091605）**，既有个人内测组 Testing。显示系统顶部拖动条，下拉与取消一致，修改仍需保存。155 项单元测试和 3 项设置 UI 回归通过，见 [发布记录](../../docs/design/iphone/settings-dismiss-20260916/release-2026091605/README.md)。

2026-09-16 **iPhone 云端异步回复已发布至 TestFlight 0.2.2（2026091604）**，既有个人内测组 Testing，配套 Worker 已上线：生成由云端任务持有，手机断网或退出后可恢复补读；发送去重、显式停止与正文/游标一致保存已接入。部署顺序、验证和边界见 [异步回复说明](../../docs/design/iphone/async-replies-20260916/README.md)。

2026-09-16 资料库改版（已随 0.2.2 / 2026091604 发布）：按第三版采用「文档列表 + 图片网格」，支持独立文件保存、分类/正文搜索、导入、预览、分享、用于对话及删除恢复；保留对话历史管理。包含旧附件迁移和独立引用清理，详见 [实现与验证记录](../../docs/design/iphone/library-20260916/implementation.md)。

2026-09-16 工具过程改版已发布至 TestFlight **0.2.2（2026091603）**：聊天主线显示独立思考摘要，点击直接看完整记录；工具采用「步骤入口 → 半屏概览 → 输入/输出详情」，本机图片/文件预览与交付卡片已接入；远程共用思考和文本过程界面。136 项单元测试及 9 条 UI 回归通过，既有个人内测组 Testing。配套 Worker 动作标题与 PPTX MIME 扩展已随后续异步回复服务上线。见 [发布记录](../../docs/design/iphone/activity-20260916/release-2026091603/README.md)。

2026-09-16 键盘交互修复已发布至 TestFlight **0.2.2（2026091602）**：聊天区点击空白处或上下拖动可收起键盘，未发送草稿保留，侧栏与输入编辑手势保持正常。129 项单元测试、6 项 UI 回归及真机签名归档通过；用户 iPhone 真机交互待验收。见 [发布记录](../../docs/design/iphone/keyboard-dismiss-20260916/release-2026091602/README.md)。

2026-09-16 界面改进已发布至 TestFlight **0.2.2（2026091601）**，既有个人内测组 Testing：对话正文延伸到顶部/底部浮层后方，修正整页侧栏圆角、选中状态与开合后的点击恢复，并收紧输入区。按用户要求采用系统默认 Liquid Glass 外观，已去掉自定义玻璃投影遮罩与细轮廓线；保留 44 pt 触控区域。GlassChromeTests 5 项、发布前全量 iOS 单元测试 129 项和真机签名归档通过。环境仍为 Xcode 26.3 / iOS 26.2 SDK；iOS 27 真机尚未验收。见 [截图](../../docs/design/iphone/liquid-glass-20260916/native-default/README.md)与 [发布记录](../../docs/design/iphone/liquid-glass-20260916/release-2026091601/README.md)。

2026-09-13 新增模型自主 `run_python`：与联网搜索、已启用的历史工具平级，由模型决定调用，返回真实结果后继续回答。iOS 展示执行状态、日志和生成文件，支持停止与重启恢复。Worker 已上线，iOS 0.2.2 (2026091306) 验证及分发记录见 [代码工具记录](../../docs/design/iphone/automatic-code-tool-20260913/README.md)。

「更多 → 记忆与历史」提供跨对话关键词检索与原消息跳转、长期记忆增改/忘记及独立自动记忆开关。跨对话检索默认关闭，开启后同步允许的消息文字；附件、示例和远程任务不参与。历史版本见 [1305 发布记录](../../docs/design/iphone/cross-chat-recall/release-2026091305/release.md)。

2026-09-13 新增：设置 → 云端模型 → 登录 Cloudflare，自动获取 DeepSeek / sub2api 模型；豆包语音、Exa 搜索和云端计算沿用同一云端会话。仅限指定邮箱，云端会话与远程电脑权限独立。代码、线上状态、验证和分发边界见 [云端模型说明](../../docs/design/iphone/cloud-models/README.md)。

按用户选择的第 1 张浅色「随身工作稿」打磨，使用 SwiftUI、SF Symbols、UIKit、PhotosUI、Quick Look、AVFoundation；没有 WebView，也不依赖旧 React/Tauri/Python 运行时。桌面 GPUI 与 potato-core 未因本次移动端工作修改。

## 目前可用

2026-09-17 已随 0.2.2（2026091702）发布：移除代码块手动运行按钮及云端计算中间页，需要执行时通过对话调用模型沙箱。聊天/文稿/工具详情统一多语言语法高亮，支持深浅色、保留原始复制内容。29 项针对性单元测试与 5 项 UI 回归通过，见 [截图与验证](../../docs/design/iphone/code-blocks-20260917/README.md)。

2026-09-13 语音启动优化（已发布至 TestFlight：0.2.2 / 2026091304，个人内测组 Testing）：麦克风获准并启动后立即进入录音，网络连接在后台进行；16 kHz PCM 在内存中按顺序暂存，最多 640 KB（20 秒），连好后上传。首次麦克风授权仍需用户操作。连接超时、缓存满或取消会停止录音，不静默丢掉开头；提前结束会先发完缓存再请求最终结果，并分别限制上传与最终转写的等待时间。iPhone 的实际链路仍为手机 → Cloudflare Worker（含云端会话鉴权）→ 豆包；本次降低的是开始说话前的等待，未移除中转，也未测量真机首字延迟。模拟器语音相关 15 项单元测试及 2 条发送/取消编辑 UI 回归通过，慢连接测试覆盖 8 秒开头音频、连接前结束、取消和连接失败；真机麦克风与国内网络需后续验收。

- 多会话与本地持久化：消息、输入草稿、文稿、附件、选中会话、设置在重启后恢复。读取损坏记录时保留原文件，避免覆盖；保存错误会显示提示。
- 原生历史页：按标题/全文搜索、置顶、重命名、软删除与最近删除恢复；切换不覆盖已有会话。
- 对话：Markdown 标题/列表/引用/代码/简单表格、复制、系统朗读、存为文稿、编辑为新分支、重新生成；重新生成保留以前的回复，可切换查看。更改较早的回复会创建分支，原后续对话保留。
- 流式连接：云端持久化任务优先用可恢复 SSE 订阅，旧服务回退轮询；断线只断开读取，停止按钮才停止生成。正文按完整字符以 32 ms 节奏平滑显示，积压自适应追赶；完成、停止、后台和减少动态效果时直接显示收到的全文。游标与正文一起保存，支持进程中断恢复。实现与验证见 `../../docs/design/iphone/streaming-20260916/implementation.md`。
- 输入：系统键盘避让、逐行增高并封顶滚动，长文可展开为原生全屏编辑器；收起保留光标与草稿，可直接发送。删除后高度缩回，500／1000字不截断。
- 附件：系统文件/照片选择、实际沙盒复制、Quick Look 预览、移除、同名文件独立保存；移除后未被任何会话引用的应用附件在下一次成功启动时清理，最近删除中的附件仍保留；图片压缩到最长 1600px JPEG；UTF-8 文本和可提取文字的 PDF 支持发送。单文件 10 MB、每条 20 个、序列化请求最多 4 MiB；扫描 PDF 会明确报错，不假装读取成功。
- 工作文稿：展开、关闭、重新打开、清单勾选、Markdown 编辑、复制、真实 `.md` 系统分享；编辑后的 Markdown 清单仍可勾选；编辑、勾选、替换时保留历史版本，支持预览和恢复，恢复前的内容也会保留。
- 语音输入：沿用桌面豆包流式ASR，录音转为16kHz单声道PCM，经Worker转写；仅申请麦克风权限。点麦克风直接原位开始，实时显示文字与音量；点勾号或在波形区上滑停止采集并等待最终结果后发送，轻点转写文字收尾后打开键盘；波形区左滑或X取消。没有独立键盘按钮。长转写跟随插入位置，手动回看暂停，点“回到最新”恢复。取消保留原草稿/附件，支持光标位置插入；最长60秒，超时、后台或切会话保留可用草稿，不自动发送。回复朗读仍使用系统语音合成。
- 自动联网搜索：模型按问题需要调用Exa，没有联网开关；搜索过程中展示进度，结果返回后继续流式回答。来源面板可查看查询、网页标题、域名和摘要，来源随回复及其版本保存。
- 设置：账号管理，以及即时保存的外观、语言、触感与自定义指令。手动接口、令牌与连接测试位于隐藏的开发者选项；开发者连接修改通过“完成”提交。凭据保存在 Keychain。
- 原生辅助能力：正文动态字号、按钮标签与状态值、主要触控目标 44pt，Reduce Motion 下移除自定义过渡。
- 回复细节：轻量「复制、重试、…」操作栏，18pt 图标与 44pt 点击区域；更多底部面板首项为换模型重新回答，其下保留分享、朗读/停止、选择文字和存为文稿。生成前与输出中状态分开，手动拖动停止自动滚动，点箭头恢复跟随。
- 多图：待发缩略图与附件数量，按选择顺序排列；消息内单图/双列网格，Quick Look从所点图片打开，前后翻页与单图分享。照片与文件导入的图片统一转为最长1600px、最多600KB的JPEG；多轮历史图片会按请求总预算再次缩小，完整请求仍限制4MiB。
- 代码块提供多语言语法高亮与复制，不再提供手动运行按钮。需要执行时，继续对话让模型调用 Python 沙箱，执行日志和图表/文档沿用工具过程、回复附件及历史版本保存。相同像素的 PNG 预览与输出文件合并，保留命名文件。

首次安装进入空白首页并提供登录入口；未登录的发送、语音和重新生成会引导登录。升级保留旧示例和用户内容。固定示例回复只用于自动化测试，不在生产会话中生成。

## 模型与 Worker 接入

2026-09-14 工作树优化（尚未发布）：模型目录沿用本地持久化缓存，6 小时内打开面板不请求网络；启动和回到前台时检查缓存，过期后后台更新。启动与面板共用正在进行的请求，关闭面板不取消更新；刷新失败保留原目录，手动刷新仍可立即请求。模型面板首页移除叉号，使用系统下拉关闭，并提供 VoiceOver escape 操作；子页面保留返回和完成。

2026-09-13 最新云端目录为 DeepSeek V4.1 Flash 与 GPT-5.6，按模型显示已验证的思考档位。云端模式不提供手填模型；刷新后自动迁移已退役的会话选择，保留历史与草稿。Worker 已部署，客户端代码通过 105 项单元测试及 8 项不同 UI 回归，0.2.2 (2026091303) 已上传 Apple，最新状态见 [分发记录](DISTRIBUTION.md)。图片生成待 sub2api 分组开通，见 [模型能力与截图](../../docs/design/iphone/model-capabilities-20260913/README.md)。下方包含此前版本的联调记录。

2026-09-13 工作树修复（尚未发布）：远程草稿及待确认指令绑定具体电脑，首次发送后的追问可从历史会话恢复；旧版缺少目标的记录需核对后认领，保留原操作编号且不自动发送。59 项单元测试与 6 条页面回归通过，详情见 [草稿归属修复](../../docs/design/iphone/ios-redesign-20260913/draft-fix.md)。随后已修复侧栏跟手开关、短拖回弹与输入/代码/原生返回冲突，iPhone 17 的 65 项单元测试和 16 条页面回归通过，见 [侧栏修复](../../docs/design/iphone/ios-redesign-20260913/sidebar-fix.md)。随后接通本机公开思考流，支持折叠、搜索暂停、停止/失败与重启保留；iPhone 17 的 76 项单元测试和 7 条页面回归、SE 的 5 条页面回归及 Release 构建通过，见 [思考过程修复](../../docs/design/iphone/ios-redesign-20260913/reasoning-fix.md)。远程输入区现可选择电脑声明支持的模型与思考档位，每次发送固定配置、重试保持原值，见 [远程模型设置](../../docs/design/iphone/ios-redesign-20260913/remote-model-fix.md)。本机聊天已接入模型目录、会话选择、思考档位与请求参数，见 [本机模型设置](../../docs/design/iphone/ios-redesign-20260913/local-model-fix.md)。远程状态现会区分最近确认、断线和后台，见 [远程状态反馈](../../docs/design/iphone/ios-redesign-20260913/remote-process-fix.md)。远程停止已绑定确认时的运行，见 [停止竞态修复](../../docs/design/iphone/ios-redesign-20260913/exact-stop-fix.md)。[发送回执恢复与未知记录处理](../../docs/design/iphone/ios-redesign-20260913/receipt-recovery.md)也已接通。首页与模型弹层已按简洁参考重做，见 [简洁首页与模型选择](../../docs/design/iphone/simple-home-models/README.md)；完整 iOS 验收仍未完成。

首次使用可从首页或侧栏 → 设置登录 Potato 账号；已有连接时，输入区按钮打开“模型与思考”。自定义连接位于开发者选项：连点设置底部版本号 7 次开启，已有手动连接会保留此入口。填写完整 HTTPS `/v1/chat/completions` 地址、模型名和可选令牌，可以先“测试连接”，再关闭本地体验并点“完成”。测试只发送固定短消息，最大输出16 token，不带历史、附件或自定义指令；仍可能产生少量模型费用，服务必须支持 `max_tokens`。令牌只写入 Keychain，不写本地 JSON；更换服务主机时清空旧令牌。

已有连接后可刷新模型目录、选择服务支持的模式和档位；手动填写模型名称仅限自定义连接。输入框选择按会话保存；更换服务后需重新选择，草稿保留。直接重试沿用当前显示的回复版本的模型和思考档位（旧记录缺少模型信息时使用会话选择）；最后一条回复的「… → 换模型重新回答」允许临时选模型和思考档位，点“重新回答”才发送，取消不改配置。该操作保留旧回复，不改变输入框模型或草稿。

客户端兼容 OpenAI Chat Completions SSE，要求服务返回 `text/event-stream`、`choices[].delta.content` 和 `[DONE]`。支持 [Cloudflare Worker](../potato-worker/README.md) 提供的接口。Anthropic 原生 Messages 需要服务器适配，目前未实现。Worker 已部署到 `https://potato-iphone-api.pal-xu.workers.dev`，模拟器已使用其 `/v1/chat/completions` 接口。模型沿用桌面土豆的 `deepseek-v4.1-flash-expires-on-0910`；供应商密钥保存在 Cloudflare Secret，模拟器 Keychain 保存独立设备令牌。真实连接、文字问答、重启恢复及示例照片问答均通过。

本机消息发送会包括当前会话与附件。新增「远程」可关联桌面 Potato 并继续电脑上的任务；Cloudflare 账号登录与中继已部署，原生模拟器经线上中继联调通过，详情见 [远程控制](../../docs/design/iphone/remote-control/README.md)。本机聊天历史尚未启用通用跨设备同步。Potato Remote 0.2.1 (2026091203) 已进入 TestFlight 个人内测，Apple 后台显示真机已安装，见 [分发状态](DISTRIBUTION.md)。本版修复 Remote 消息右侧气泡、思考与执行折叠、复制/选择/分享及代码边框，见 [回复修复记录](../../docs/design/iphone/remote-control/reply-fix-20260912/README.md)。真机远程任务、App Store 正式发布、完整 VoiceOver 人工验收及相机实时拍摄尚未完成。不是已达 ChatGPT/Claude 全量功能对等的发布版本。

## 构建与运行

Xcode 26.3，iOS 26.3 iPhone 17 模拟器；最低编译部署目标 iOS 17，尚未验证 iOS 17 真机。打开 `PotatoMobile.xcodeproj`，选择 `PotatoMobile` Scheme 运行；客户端没有第三方包依赖。

```sh
cd native/potato-ios
xcodegen generate
xcodebuild -project PotatoMobile.xcodeproj -scheme PotatoMobile \
  -sdk iphonesimulator -derivedDataPath build build
xcodebuild -project PotatoMobile.xcodeproj -scheme PotatoMobile \
  -destination 'platform=iOS Simulator,name=iPhone 17' \
  -derivedDataPath build test
```

模拟器默认使用 ad-hoc 签名及本应用的 Keychain 权限；不要关闭签名，否则凭据保存会失败。真机需在 Xcode 配置自己的开发团队与签名。默认 PotatoMobile 测试使用独立沙盒、独立 Keychain 测试账号和可控示例流，不调用真实模型。

显式运行 `node scripts/provision-desktop.mjs <模拟器UUID>` 可以将原生桌面土豆当前连接导入模拟器。临时文件权限600，导入后删除，解密值不输出或写入源码；导入入口仅存在于DEBUG构建。独立的 `PotatoLiveVerification` Scheme 会通过已配置服务发送小型真实请求，不属于默认离线测试。

iPhone 17 语音改造新增7项状态测试；原生38项单元测试通过，6条针对性UI回归通过（含4条新语音流程），真实豆包原位修改/直接发送两条联调通过。此前自动Exa搜索、来源重启恢复和9条UI回归已通过。此前已通过DeepSeek直连、Worker文字及单图/四图问答、E2B执行及文档预览。iPhone SE前一轮19项通过，文稿版本恢复也单独通过；Worker最新18项测试、类型检查及部署通过。语音联调使用合成音频，真机麦克风、iOS17与完整VoiceOver尚未验收。各轮证据见验收文档。

真实照片测试默认跳过。在已检查图库内容的专用模拟器中，设置 `TEST_RUNNER_POTATO_LIVE_SAMPLE_PHOTO=1`，可运行 `PotatoLiveVerification` 的 `testSimulatorSamplePhotoThroughConfiguredService` 或 `testFourSimulatorPhotosThroughWorker`；分别发送一张或四张系统示例照片，不要对个人图库启用。`TEST_RUNNER_POTATO_LIVE_SANDBOX_SETUP=1` 启用真实模型生成 Python 代码并检查代码块只显示复制、不显示手动运行的测试；这不等于 E2B 执行成功。

功能进度见 [IMPLEMENTATION.md](IMPLEMENTATION.md)，视觉证据与限制见 [design-qa.md](design-qa.md)。

真实沙箱测试同样默认跳过。设置 `TEST_RUNNER_POTATO_LIVE_SANDBOX=1` 并运行 `testCloudExecutionProducesFilesAndPersists`，会通过已配置模型生成测试Python，再发送追问让模型调用E2B生成图表和文档、预览PDF、重启检查。仅使用合成数据，会产生少量模型及沙箱用量。代码块展示测试不调用 E2B。

本轮回复与沙箱接入详见 [调研及历史接入证据](../../docs/design/iphone/replies-and-sandbox.md)。找回的既有E2B实现位于 `chat-web-dev`，自定义模板为 `chat-web-office-pdf`。用户提供有效凭据并明确授权保存后，现有Worker已启用E2B；旧项目部署未改变。

语音方向1的参考图、状态规则和证据见 [语音交互设计](../../docs/design/iphone/voice-redesign/README.md)。`--ui-testing --voice-preview` 是仅限DEBUG的脚本转写，用于确定性UI截图，不接豆包。真实豆包联调使用 `PotatoLiveVerification`、`TEST_RUNNER_POTATO_LIVE_SPEECH=1` 及 Application Support 内的 `PotatoVoiceVerification.pcm`（16kHz、单声道、16位PCM）；测试 `testDoubaoInlineEditKeepsUnsentDraft` 与 `testDoubaoInlineSendWaitsForFinal`，产生少量已配置服务用量。测试后移除合成音频文件。

最新输入面板规格见 [Grok参考与长文编辑](../../docs/design/iphone/compact-composer/README.md)，取代此前语音稿的按钮布局。

2026-09-12最新输入改造：40项单元测试、长文/手势UI回归、iPhone17与SE普通及大字布局检查、两条真实豆包转写编辑/发送联调通过。转写正文为黑色，真实波形从本地PCM音量更新，独立于上传速度；静音归零。


## 界面语言

设置 → 语言提供“跟随系统”（默认）、English、简体中文。选择即时生效并持久化。已有对话、草稿与自定义指令保持原文。文案资源位于 `Sources/Localization/`，新增界面文案通过 `L10n.tr` 读取；此前实现与历史验证见 [语言记录](../../docs/design/iphone/language-20260917/README.md)。
