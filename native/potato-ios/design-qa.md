# iPhone 原生客户端：本轮视觉与交互验收

2026-09-12 · Product Design · 原生 SwiftUI。

final result: passed

通过范围为列出的原生模拟器流程、所选浅色视觉方向、DeepSeek直接文字联调，以及下方新增的 iPhone → Cloudflare Worker → DeepSeek 文字/图片链路。**不代表真机录音、完整VoiceOver、所有系统版本或全量ChatGPT/Claude功能对等。** 持续目标仍有真机与系统提供器验收事项。前文各轮限制保留为当时的历史记录，以末尾最新实测为准。

## 比较证据

![选定稿与当前 SwiftUI 同图对照](qa/revisions/comparison.png)

![输入区细节对照](qa/polish/composer-final.png)

选定图为 `docs/design/iphone/pocket-working-draft-selected.png`，853×1844px。原生 iPhone 17 为 402×874pt / 1206×2622px，去除上方62pt、下方34pt安全区域后为402×778pt。源图按宽度等比缩放到402pt，**没有纵向拉伸**；因此两幅图应用内容高度不同，不能宣称像素一致。

比较状态：浅色、初始示例文稿、未展开、未勾选、键盘关闭。最新图 `qa/revisions/01-working-draft.png` 来自 `RevisionsVerified` XCTest 文稿流程；已打开检查源图和同图比较输入。最终操作入口增加编辑/关闭/版本历史、模式明确标示为本地体验，属于用户要求完善功能后的有意改变。

## 流程与结果

| 步骤 | 原生操作 | 状态与证据 |
| --- | --- | --- |
| 1 | 初始工作文稿 → 展开 → 勾选 | 通过；默认字号四条清单可读，展开后可操作。[初始](qa/polish/working-draft-refined.png) |
| 2 | 编辑 → 保存 → 复制 → 系统分享 → 重启恢复 | 通过；修改与勾选写入本机存储，生成真实 Markdown 分享文件。[编辑](qa/polish/iphone17/02-document-editor.png)、[分享](qa/polish/iphone17/03-share.png) |
| 3 | 新对话 → 键盘输入 → 发送 → 停止 → 重新生成 | 通过；键盘避让、发送状态、已停止提示、固定说明流明确标为本地。[新对话](qa/polish/iphone17/04-new-chat.png)、[键盘](qa/polish/iphone17/05-keyboard.png)、[停止](qa/polish/iphone17/06-stopped.png) |
| 4 | 历史列表 → 搜索消息 | 通过；真实多会话、本机保存，标题与消息可检索。[历史](qa/polish/iphone17/07-history.png)、[搜索](qa/polish/iphone17/08-search.png) |
| 5 | 设置 → 关闭本地体验 → 校验不完整连接 | 通过；缺少有效 HTTPS 地址或模型名称时显示错误，保留输入。[设置](qa/polish/iphone17/10-settings.png)、[校验](qa/polish/iphone17/10-settings-validation.png) |
| 6 | 附件菜单 → 系统照片选择器 → 导入 → 预览 → 关闭 → 移除 | 通过；使用模拟器内置示例照片，实际转换并保存 JPEG，预览显示内容和友好名称，不发送网络。[导入](qa/polish/iphone17/15-photo-imported.png)、[预览](qa/polish/iphone17/16-photo-preview.png) |
| 7 | 语音入口 → 待录音状态 | 入口通过；开始前未录音，空转写不可确认。[语音](qa/polish/iphone17/14-voice-ready.png)。实际授权与识别质量待真机验收 |
| 8 | iPhone SE 小屏与最大辅助字号 | 通过已测布局；正文滚动，导航和输入可触达，长文无需强行挤入一屏。[小屏集合](qa/polish/se-overview.png)、[大字号](qa/polish/se/11-accessibility-text.png) |

## 本轮修复

1. [P1] 极大辅助字号让居中品牌标题溢出并压住内容。固定图标大小，对紧凑工具栏/品牌标识设独立字号上限，正文继续支持最大动态字号。复测不再重叠。
2. [P2] 标题恢复到26pt后，默认文稿末行44pt点击区部分进入操作栏。收紧文稿内边距与小节留白，最后一行完整位于可读区；小屏保留滚动。
3. [P2] 设置中的系统默认占位色过浅。改用明确的次要文字色；日期改为与中文界面一致的区域格式。
4. [P1] 默认全屏 Quick Look 的关闭路径不清晰，自动化只能看到图片。替换为具有友好文件名和固定“完成”按钮的原生预览弹层，实际导入、预览、关闭、移除完整测试通过。
5. [功能] 原型历史会清空修改、退出丢失数据、语音/附件只有示例等问题已被持久化会话、实际导入与语音实现取代。本地模式仍明确说明没有模型推理。

照片测试曾因 iOS 26 将网格暴露为 Image 而非 CollectionView Cell、以及过宽的按钮匹配选中下层控件而失败；修正为观察到的 `PXGGridLayout-Info` 和系统 `Add` 标识。此类失败单独记录为测试定位问题，没有冒充导入功能缺陷或忽略失败后直接宣称通过。

## 五项视觉检查

- 字体：SF/苹方系统字体，文稿标题26pt，正文使用原生body样式；层级清楚。默认与最大辅助字号有截图证据，未宣称完整VoiceOver合规。
- 间距：保持暖白会话、白色前景文稿和固定底部输入；真实安全区域导致内容高度比生成图短。四条默认清单与44pt按钮可用，长文/小屏滚动符合原生适配。
- 色彩：暖白底色、中性灰消息、深正文、克制边框；空输入发送按钮禁用变灰，配置与错误态有明确文字。
- 资源：界面统一SF Symbols。iOS图标沿用项目已有 `scripts/pack/assets/icon.svg` 品牌源，只将背景改成无圆角的1024px全幅，供系统裁切；输出无透明通道，图像清晰，无重新设计品牌。
- 文案：顶部“本地体验”、模型连接页数据发送说明、语音识别说明和保存状态相互一致；不把固定回复、模拟网络fixture或未部署Worker写成真实AI服务。

## 验证与限制

- `build/PolishFinal.xcresult`：20项通过（5条UI、15条单元/网络协议）。
- `build/PolishTests-SE.xcresult`：19项通过，375×667pt iPhone SE/iOS26.3；发生在增加文稿上下文测试与照片预览完善之前。
- `build/PhotoImport-v3.xcresult` 中文稿回归通过，照片测试定位失败；最后留白截图取自已通过的文稿步骤。
- `build/PhotoImport-v5.xcresult`：新增实际照片导入/预览/移除测试通过。
- 最终 `build/ReleasePreviewTests.xcresult`：21项全部通过（6条UI + 15条单元/网络测试）。关闭文稿后即时生成的 `09-accessibility-text` 截图含过渡残影，已从验收集合排除；辅助字号以稳定的11号截图为准。最终通过流程截图已汇总在 `qa/polish/all-flows-final.png` 并复查。
- Worker：8项处理器测试、TypeScript检查、Cloudflare打包dry run、本地workerd健康200/缺少配置503均已通过，未部署或调用真实模型。

Mac仍锁屏，Computer Use人工操作不可用。本轮视觉证据来自真实模拟器XCTest截图并逐张/合图检查；不是竞品实机审查，也不是人工语音授权验收。iOS17、真机、Apple权限交互、云盘文件提供器和云端端到端模型调用仍待验证。完整后续范围见 [IMPLEMENTATION.md](IMPLEMENTATION.md)。

## 后续打磨：内容可恢复性

- 文稿每次编辑、勾选、替换前保留版本；支持先预览再恢复，恢复前的内容再次留存。旧版 JSON 没有版本字段时仍可读取。
- 重新生成保留原回复，停止或失败时仍能切回旧内容；复制、朗读、文稿保存、分享、后续模型上下文使用当前选择的回复。切换较早回复会创建分支，原后续消息保留。历史搜索覆盖旧回复，列表展示当前选择。
- 移除附件后在下一次成功启动时清理无引用文件；最近删除、待发送和分支仍引用的文件保留。损坏状态文件不会触发清理。
- `RevisionsTests.xcresult`：27项通过；`RevisionsFinal.xcresult`：文稿恢复中的即时无障碍值断言失败，其余通过。改为等待实际状态后，`RevisionsVerified.xcresult` **29项全部通过（7 UI + 22 单元/协议）**。没有把失败结果计为通过。
- 截图检查发现恢复按钮深色底上标签不可见，以及版本日期使用英语格式。显式设置按钮白色文字和中文日期；同类语音确认按钮一并修正。

iPhone SE 上 `RevisionsSE.xcresult` 新增版本恢复完整流程通过；[小屏版本预览](qa/revisions/se-18-draft-version-preview.png)无按钮或正文遮挡。

最终视觉修改后的 `RevisionsVisual.xcresult`：3条关联UI流程通过，已复查恢复按钮白字和中文日期。新增页面见 [版本流程合图](qa/revisions/overview.png)、[文稿历史](qa/revisions/17-draft-history.png)、[恢复预览](qa/revisions/18-draft-version-preview.png)、[回复切换](qa/revisions/19-reply-version.png)。

本轮继续属于本机能力与模拟器验证，真实模型、Worker部署和真机权限验收仍未完成。

## 连接接入体验

- 设置新增主动“测试连接”、取消、成功/失败状态；地址、模型或令牌改变后清除旧结果，退出时取消请求。测试成功不会代替保存设置。
- 使用固定短测试消息，最大输出16 token，不包含已有对话、附件或个人回复偏好。收到非空文字且流正常结束才成功；空回复、半途断开、401、非SSE响应均不算成功。可能产生少量模型调用费用，界面在点击前说明。
- 增加超时、DNS、连接中断、HTTPS证书问题的中文反馈，不展示底层错误内的私人URL。普通对话使用同一错误说明。
- `ConnectionTests.xcresult`：32项全部通过（7 UI + 25 单元/协议）。连接请求测试使用真实URLSession配合本地URLProtocol fixture，验证请求隔离和失败边界；没有访问真实供应商。
- `ConnectionSetupUI.xcresult`：新增配置填写与取消流程通过，但截图发现键盘未收起、测试入口不在可视区域。增加原生收起键盘工具栏、下一项输入导航、交互式滚动收起；UI断言同时要求按钮可点击。

`ConnectionSetupFinal.xcresult`：2条设置UI复验通过。已检查 [连接设置截图](qa/connection/setup.png)，按钮与调用说明完整可见；本地体验说明区分普通对话与主动连接测试。

真实服务与真机验证仍待用户提供相应条件。当前模拟器自动测试不能证明真实模型可用，也不能代替VoiceOver或录音质量验收。完整目标审计见 [ACCEPTANCE.md](ACCEPTANCE.md)。

最终联网说明调整后的 `ConnectionCopyFinal.xcresult` 出现一次输入事件失败：键盘弹出时模型字段未取得焦点。改用已实现的原生“下一项”导航后，`ConnectionNavigationVerified.xcresult` 完整配置/收起键盘/按钮可触达/取消不保存流程通过。最终截图取自这一通过结果。未将中间失败计算为通过。

## 最新实测：沿用桌面 DeepSeek

用户明确要求模型与本机土豆一致后，只读原生数据库的active/provider配置，沿用 `deepseek-v4.1-flash-expires-on-0910` 与 `https://api.deepseek.com/chat/completions`。2026-09-12实际请求仍为200并正常结束，未改成本轮公开的新别名，也未修改桌面设置。

本轮修复了两个真实接入问题：

1. 旧构建关闭签名，界面和网络fixture测试无法证明Keychain可写。增加应用自己的application-identifier/keychain权限和模拟器ad-hoc签名；`SignedKeychain.xcresult` 26项单元测试通过，包括独立测试账号的真实写入、读取、更新与删除。一次性开发导入文件权限600，读取后删除；入口仅在DEBUG构建存在。
2. DeepSeek默认开启思考，16 token连接测试可能在输出正文前耗尽预算。短连接测试关闭DeepSeek思考，普通聊天保持原行为；收到finish_reason=length时明确显示输出上限，保留部分回复。Worker保留客户端较小预算，并仅转发白名单结构的thinking参数。

`LiveDeepSeek.xcresult` 真实问答和重启恢复成功，但连接测试因预算耗尽失败。`LiveDeepSeekVerified.xcresult` 连接已成功，断言因状态标签未合并暴露而失败；已修正可访问标签。最终 **`LiveDeepSeekFinal.xcresult` 整条真实UI流程通过**：已有Keychain令牌 → 测试连接成功 → 新对话 → 服务返回POTATO_LIVE_OK → 重启后仍可见。

![真实连接与原生回复](qa/live/overview.png)

另外使用本机生成的128×128纯红图直接请求同一模型，200、完整流结束、正确返回red。此项证明供应商图片接口可用，**不等于iPhone照片经Worker的完整联调**。图片选择/导入/预览仍采用前轮模拟器UI证据。

已检查源码与构建App，不包含解密后的供应商密钥。模拟器当前保存的是同一DeepSeek直连接口与Keychain凭据；云端部署尚未发生。Cloudflare登录有效且目标Worker不存在，但自动审批拒绝了具体的供应商密钥云端转移，需用户明确授权后继续。

协议依据：[DeepSeek Chat Completions](https://api-docs.deepseek.com/api/create-chat-completion/)、[Apple Keychain应用权限](https://developer.apple.com/documentation/security/sharing-access-to-keychain-items-among-a-collection-of-apps)。

最终 `DesktopAlignedRegression.xcresult`：**34项通过（8 UI + 26 单元/网络/Keychain）**。默认测试仍未调用真实模型；真实调用单独记录在LiveDeepSeekFinal。Worker本轮9项处理器测试与TypeScript检查通过，最终dry run打包通过。主模拟器已恢复到用户实际DeepSeek连接，导入临时文件已删除。

## 最新实测：Cloudflare 完整链路

用户明确回复“可以”授权云端密钥保存与部署后，已部署 `https://potato-iphone-api.pal-xu.workers.dev`，版本 `35e7c40c-510c-4ab1-b67f-12da6ce30aaa`。供应商密钥只作为Cloudflare Secret；模拟器Keychain改存独立的随机设备令牌，设置改为Worker地址，保持桌面相同模型。健康检查200，未认证聊天401。未使用VPS。

- `build/LiveWorker.xcresult`：1条真实UI流程通过。设置展示Worker地址，主动连接收到真实回复，新对话返回POTATO_LIVE_OK，重启后消息仍在。
- `build/LiveWorkerPhoto.xcresult`：1条真实UI流程通过。系统照片选择 → 实际JPEG导入 → Quick Look预览 → 发送 → 模型正确描述粉色/洋红花朵、少量黄色花和绿色叶片。发送的是前轮已检查的模拟器内置示例照片，压缩后界面显示837 KB。
- 新增图片测试只在明确设置 `TEST_RUNNER_POTATO_LIVE_SAMPLE_PHOTO=1` 时运行，普通真实文字测试和默认离线测试不会自动读取或发送图库照片。

![Worker连接与真实文字回复](qa/worker/overview.png)

![实际发送的示例照片与模型回复](qa/worker/photo-overview.png)

以上两组合图均从通过的XCTest结果导出并打开核对。照片源与回答对应，设置内令牌不显示明文。此次只新增独立真实图片测试和部署记录，没有修改已通过34项回归的客户端运行逻辑。真机麦克风、签名安装、完整VoiceOver、iOS17和用户云盘提供器仍待验收。

## 最新实测：回复操作、多图与云端计算入口

本轮继续以原生SwiftUI实现第1版浅色方向。改动前截图见 [当前回复审查](qa/reply-audit/01-current-reply.png)，竞品对照依据与历史沙箱来源见 [回复及沙箱研究](../../docs/design/iphone/replies-and-sandbox.md)。没有进行本轮ChatGPT/Claude真机完整审查。

回复新增复制勾选反馈、朗读/停止、选择文字和单条分享；输出前与输出中分别展示等待和回复状态，手动拖动阅读暂停跟随，可点击回到底部。输入图片显示缩略图与数量，消息中按顺序排成网格，Quick Look从点击的图片开始，可前后切换并分享当前图片。图片统一转JPEG，单张最多600 KB，多轮历史按总图片预算再次压缩，避免四图请求超过Worker 4 MiB上限。

验证记录：

- `ReplyGallery.xcresult`：新增回复菜单、文字选择、四图、流式停止、图片切换和重启恢复UI流程通过。
- `ReplySandboxRegression.xcresult`：37项全部通过，包含9条UI和28条单元/网络/Keychain测试。新测试覆盖沙箱请求只发送勾选文件、同源地址，以及执行产物持久化和回复版本保留。
- `MultiImageBudget.xcresult`：图片预算调整后29项关联回归通过。
- `ReplyLiveWorker.xcresult`：四张已检查的系统示例照片经Worker真实问答通过。Python入口用例因父级可访问标识覆盖子按钮而失败，实际按钮已渲染；移除覆盖后，`SandboxSetupVerified.xcresult` 独立用例通过。此复验发送真实代码生成请求，点击运行后明确显示E2B尚未配置，没有声称执行成功。
- Worker：13项测试、TypeScript检查及部署通过；版本 `0a197f44-1da8-4b3d-ad2e-f8627444218f`，部署后健康检查正常。测试覆盖鉴权、缺少E2B配置、输入边界、产物和销毁，不包含真实E2B执行。

![回复操作、四图与真实模型结果](qa/replies/overview.png)

截图25–30来自通过的完整回归；[四图真实回复](qa/replies/31-live-four-images.png)与[云端计算配置提示](qa/replies/32-sandbox-setup-required.png)来自对应真实UI用例。已打开核对，未发现这些截图里的关键操作遮挡。Quick Look测试仍出现UIKit工具栏布局警告，未造成上述切换流程失败；原生捏合缩放与实际朗读音频未单独做设备验收。

E2B目标模板来自旧 `chat-web-dev` 项目的 `chat-web-office-pdf`。当前无法访问历史凭据，Worker只有模板名称，没有E2B Secret。真实Python/CSV分析/图表与文档输出还需恢复账号后验证；当前是用户显式运行代码的入口，尚无模型自动规划、修错、调用工具循环。

## 最新实测：E2B真实执行已接通

2026-09-12 17:02。用户提供完整E2B API Key后，官方认证200，历史 `chat-web-office-pdf` 模板启动成功。自动审批最初要求指定账户持久保存凭据的明确授权；用户回复“授权保存，并继续手机端联调”后，已保存到Pal_xu账户 `potato-iphone-api` 的 `E2B_API_KEY` Secret，部署版本 `0fd1cca6-d9c6-4a18-b4b6-74d54dfa7d9d`。源码、配置和验收文档扫描未发现密钥；设备令牌及桌面DeepSeek配置未改变。

- 使用生产SDK适配直接上传三行合成CSV，Python正确读取并计算合计60，生成PNG、PDF、DOCX、XLSX。已检查图表，提取PDF/Word正文并核对Excel公式与数值。原始产物在 `qa/sandbox/`。
- `LiveE2BFiles.xcresult`：iPhone→Worker→E2B真实运行通过；模型生成Python，手机显式提交，返回图表和三类文档，PDF打开及重启恢复通过。第一轮截图发现同一图表的富预览和输出PNG重复展示。
- 修复：在iPhone保存产物时，对不超过4M像素的单帧、无旋转PNG比较完整sRGB像素摘要；不做缩略图相似合并，保留有文件名的输出。超过预算的图片保留原样，不扩大解码内存。`SandboxArtifactDedup.xcresult` 29项单元测试通过，包括不同PNG元数据但相同像素合并、不同颜色保留及原文件字节保留。
- `LiveE2BVerified.xcresult`：最终真实流程通过，明确断言只显示一张图。重启后读取该测试回复，执行状态complete、stdout正确，4个实际文件存在；PDF/Word包含Total: 60，Excel三行数值10/20/30正确，PNG可解码。对应文件在 `qa/sandbox/worker/`。
- 结束后E2B只读检查返回200，当前运行沙箱为0，未留下测试计算实例。

![真实云端计算与PDF预览](qa/sandbox/overview.png)

截图取自最终通过的测试并已打开核对。系统Quick Look仍有前轮同一条UIKit工具栏警告，实际PDF预览和关闭流程通过。这里的PDF/Word/Excel是小型联调样例，不能视为复杂文档排版质量验收。

本次没有将沙箱Key写入iPhone。手机只持有原来的Worker设备令牌。仍需后续验证：手机系统文件提供器导入CSV/PDF→勾选→分析的完整组合、真机/最低iOS17与完整VoiceOver。当前使用“模型生成代码→用户点击运行”的流程，尚未实现自动Agent工具循环。

## 最新实测：自动Exa搜索与豆包语音

2026-09-12 17:37。沿用桌面DeepSeek模型、Exa及原生豆包配置；用户回复“允许”后，保存Exa/豆包Secret到同一Pal_xu Worker。iPhone没有联网开关，搜索由模型工具调用决定。

- `SearchSpeechFinalUnits.xcresult`：31项通过，覆盖来源SSE/版本持久化、同源语音请求、16kHz PCM转换及停止时排空尾部。此前两轮暴露AVAudioConverter首块残留，未把失败结果算作通过。
- `SearchSpeechUIRegression.xcresult`：9条UI回归通过，覆盖现有回复、附件和语音入口。
- 首轮 `LiveSearchSpeech.xcresult` 两条失败：模型即使收到parallel_tool_calls=false仍返回两个搜索调用；当前Cloudflare默认二进制消息为Blob，语音转发按ArrayBuffer解码导致失败。分别修复批量调用（总预算4次、每个调用都有工具结果）及两端显式binaryType=arraybuffer。
- Worker最终18项测试、TypeScript及dry run通过。模型替身测试验证一轮2次、下一轮3次请求时，最多实际搜索4次，第5个返回预算耗尽结果，最终回答只有一个DONE。
- `LiveSearchSpeechVerified.xcresult`：两条真实UI流程全部通过。Exa返回两次查询、7个去重来源，回答完成后可打开来源面板，重启后仍保留。豆包识别3.6秒合成语句“你好，土豆，帮我整理今天的工作计划。”，确认后仅进入输入框，没有发送消息。
- 最新Worker版本 `2cfc1662-2d0b-475e-8cbc-8e5a717b73d2`。curl核对健康200、聊天/语音/沙箱未认证401；Python urllib在本机返回403，未将该结果当作Worker处理器状态。

[自动搜索回复](qa/search-speech/36-auto-search-reply.png) · [来源面板](qa/search-speech/37-auto-search-sources.png) · [豆包转写](qa/search-speech/38-doubao-transcript.png) · [确认后的输入框](qa/search-speech/39-doubao-input-draft.png)

四张截图来自通过的测试，均已打开检查。来源截图中有一条供应商返回空标题；随后补上使用URL路径/域名的回退名称并重新编译，因此截图保留的是修复前证据。来源摘要保留网页原文，可能含Markdown标记。

语音使用固定合成音频验证真实服务链，不代表真机麦克风、60秒录音、来电中断或噪音条件的验收。回复朗读仍为系统TTS；尚未实现双向实时语音对话。搜索已自动调用，沙箱仍由用户点击运行。实现与官方依据见 [搜索与语音迁移](../../docs/design/iphone/search-and-speech.md)。

## 最新实测：语音方向1的原生实现

2026-09-12。用户选定第1张「就地说，直接发」。沿用SwiftUI、SF Symbols和豆包，改造输入区；上方消息组件保留当前样式，不新建网页原型。

### 比较目标与证据

- source visual truth：`docs/design/iphone/voice-redesign/concept-1-direct-send.png`，853×1844px；编号以该目录 `concept-order.json` 为准。
- implementation：`qa/voice-inline/40-inline-recording.png`，iPhone17，402×874pt / 1206×2622px，@3x。浅色、同样示例对话和听写文字、正在录音、键盘关闭。
- 密度归一化：两张等比缩至390px宽，分别高843px和848px，不纵向拉伸。原生系统状态栏与安全区由iOS提供，参考图未包含相同区域，因此不宣称整屏像素一致。
- [全屏同图比较](qa/voice-inline/comparison.png) 和 [输入区聚焦比较](qa/voice-inline/composer-comparison.png) 均实际打开检查。聚焦分别裁取源图底部42.5%、原生图55.3%–95.8%区域，保留等比像素，比较字号、留白、状态与按钮。
- 40–44为明确的DEBUG脚本转写截图，45–47为真实服务链测试截图。正式麦克风和合成音频联调均按PCM计算音量；时间和波形不照着参考图伪造。

### Findings 与迭代记录

1. **[P1，已修复] UIKit输入框首字后失焦。** 首轮UI测试出现Original只剩O，历史搜索也仅输入首字。将FocusState桥接改为普通焦点Binding，并避免程序更新倒退键盘实时选区；修复后6条针对性回归通过。[原位编辑](qa/voice-inline/42-inline-edit.png) 显示完整原文与追加文字，重启恢复断言也通过。
2. **[P2，已修复] 大字模式挤掉发送文字。** [首轮大字截图](qa/voice-inline/iteration-1-large-text.png) 中按钮可点击但标签不可读。状态/操作区限制至xxxLarge，辅助说明允许换行；正文继续支持最大动态字号。[修复后](qa/voice-inline/44-inline-large-text.png) 三项操作与说明完整可见。
3. **[P2，已修复] 口述正文偏小、顶部留白不足。** [首轮录音](qa/voice-inline/iteration-1-recording.png) 使用20pt title3。改为24pt、按title2比例动态缩放，并增加16pt文本顶部留白；重新捕获40并同图比较，正文层级与两行布局接近所选图。
4. **[P2，已修复] SE正文区域被压缩。** `InlineVoiceSmallScreen.xcresult` 的功能断言通过，但[首轮截图](qa/voice-inline/se/iteration-1-overview.png)显示正文第二行裁切、辅助字号首行也不完整。将可压缩的52–104pt区域改成至少104pt并随字号增高；[修复后同设备/状态](qa/voice-inline/se/overview.png) 中默认两行完整，大字至少显示完整首行，后续文字可滚动，操作区完整。SE为375×667pt、750×1334px，截图等比缩至375px宽比较。`InlineVoiceSmallScreenVerified.xcresult` 两条UI用例通过；iPhone17 `InlineVoiceLayoutFinal.xcresult` 两条也通过，最终40/44及全屏/聚焦比较均重新捕获并打开。

### 五项视觉检查

- **字体：** 系统SF/苹方，录音正文24pt medium随Dynamic Type放大；计时等宽数字，辅助文字caption，发送semibold。控制区限制字号，正文保持滚动。
- **间距：** 原位输入区、波形/状态/操作分层、主要按钮至少48pt；保留当前圆角、细边框和系统安全区。源图与原生宽度不同带来的少量断行/边距差别为P3。
- **颜色：** 暖白背景、白色输入区、墨黑主操作、次要灰；未识别文字时禁用发送，收尾显示进度，取消仍可用。
- **图片/图标：** 录音区无照片或插画资产；使用SF Symbols，波形为音量数据可视化。按设计说明，源图上方星形/助手气泡为示意，保留现有消息组件。
- **文案：** “正在听”“取消”“修改”“发送”和底部提示与稿件一致；删除额外的开始与放入输入框两步。收尾立即反馈，比研究稿拟定700ms阈值更直接回应点击。

### 功能证据与边界

`InlineVoiceUIVerified.xcresult` 的6条针对性UI用例通过；`InlineVoiceFinalVerified.xcresult` 的38项单元测试、2条语音UI用例通过。单元测试包含UTF-16/emoji插入、累积partial替换、final尾字、只发送一次、附件保留、编辑后晚回调，以及失败/空final/后台/切会话/60秒上限处理。

`LiveInlineVoice.xcresult` 的真实豆包两条用例通过：[转写](qa/voice-inline/45-live-inline-recording.png)、[修改后草稿](qa/voice-inline/46-live-inline-edit.png)、[直接发送后模型回复](qa/voice-inline/47-live-inline-sent.png)。语句为“你好，土豆，帮我整理今天的工作计划。”，使用3.6秒合成PCM，未录制个人声音。46在键盘转场采集，持续编辑以42及UI断言为证据。[后台草稿](qa/voice-inline/43-inline-interrupted.png) 保留文字且未发送。

P3后续：少量断行、按钮间距、圆角差异保留原生设计系统，不拉伸截图或绘制假光标。真机麦克风、噪音/蓝牙、实际来电、完整60秒音频、iOS17和完整VoiceOver尚未实机验收；模拟器后台与注入中断不等于真实电话测试。Xcode仅有未使用AppIntents的元数据警告；原生应用不适用浏览器console检查。

Implementation checklist：原位启动、发送收尾、修改/取消、草稿/附件/光标保留、旧回调隔离、真实服务联调、默认/大字/SE截图比较完成。没有剩余P0/P1/P2问题。已安装最终构建并正常启动iPhone17客户端，移除本次合成PCM测试文件；设备令牌与Worker配置保留。

final result: passed

## 2026-09-12 — Grok参考面板与长文本输入

依据：[用户Grok截图](../../docs/design/iphone/compact-composer/grok-reference.jpg)。用户先认可紧凑面板，再明确要求像素级借鉴、移除独立键盘按钮，最后要求正文使用黑色、波形跟随真实声音。最终以这些最新要求优先于参考图灰字和上一轮24pt正文稿。规格见 [compact-composer](../../docs/design/iphone/compact-composer/README.md)。范围为原生iOS输入面板，未改GPUI或Worker。

### 对照与修复

已把参考和相同短句的原生截图等比放在同图检查：[面板对照](qa/compact-composer/reference-composer-comparison.png)、[全屏上下文](qa/compact-composer/reference-full-comparison.png)。保留各自纵横比，面板以相同显示宽度比较。用户参考设备的逻辑宽度和字体设置无法由截图精确确定；不声称整个Grok页面逐像素一致。

- **布局与比例：** 去掉独立键盘按钮和竖排计时，恢复两端圆按钮、中间细波形。33pt视觉圆配44pt触摸区域，26pt面板圆角，左右10pt页边距。初版面板底色过淡、边界不清，已换成接近白色实底。源码没有仿造键盘，系统键盘及品牌区域按Potato现有原生环境保留。
- **正文：** 按用户要求改为黑色、系统Body字号，并支持辅助字号。动态高度基于排版结果，不按字数硬编码。500/1000字不截断；默认iPhone17可见区158pt，原审查为101.33pt；SE按可用空间收紧为108.5pt。普通短句有少量原生字形/断行差异，属P3，不压缩字符或使用图片冒充可编辑文字。
- **表面与层级：** 语音移除实线描边，使用轻阴影；波形黑色、后续点线淡色，正文是主要阅读焦点。上滑/左滑提示为辅助操作，X与勾号可独立完成取消和发送。
- **操作与反馈：** 轻点转写文字收齐final后修改；勾号或波形上滑发送；波形左滑取消。文本拖动只用于阅读，不触发发送手势。原有空结果、后台、失败、晚回调、附件保留和只发送一次的状态保护仍在。
- **长文与跟随：** 文字逐行增高、达到上限滚动，展开编辑同一份草稿，可收起或直接发送。第一、第二轮“回到最新”测试虽通过，截图仍暴露停在正文中部的P1：[迭代1](qa/compact-composer/iteration-1-follow-position.png)、[迭代2](qa/compact-composer/iteration-2-follow-position.png)。修复为布局完成后定位实际插入点；文末使用内容高度计算滚动位置，取消旧滚动惯性。后续53/55/56截图分别核查持续转写、手动回看暂停、恢复后可见正在追加文字。
- **音量：** 移除DEBUG预览`length % 7`周期动画，使用合成音频RMS包络随后归零。真实麦克风在本地PCM采样后计算音量，不等待上传；UI用近期21个音量点更新。静音回归点线，不制造周期运动；减少动态效果有静态替代。

### 可核验状态

[1000字](qa/compact-composer/52-input-1000.png)、[全屏编辑](qa/compact-composer/54-expanded-input.png)、[手动回看](qa/compact-composer/55-voice-review.png)、[回到最新](qa/compact-composer/56-voice-latest.png)、[SE辅助字号](qa/compact-composer/se/44-inline-large-text.png)、[短句最终布局](qa/compact-composer/59-reference-compact-voice.png)。大字模式为了显示最新一句，滚动区域顶部可能露出前一行的一部分；内容可滚动回看，底部X/勾号和展开入口完整可达。

`GrokCompactFinal.xcresult` 8UI通过；`GrokCompactSE.xcresult` 3UI通过；`GrokVoiceMeterFinal.xcresult` 40单元＋3UI通过；最终系统Body黑字布局由`GrokFinalAppearance.xcresult`在iPhone17和SE分别覆盖普通、辅助字号，共4UI通过。测试含PCM音量、静音归零、旧回调隔离、实际滑动手势、取消草稿、直接发送、展开/收起/重启、500/1000字和删除缩回。完整轮次见ACCEPTANCE，早期功能测试通过不代替最终截图审查。

边界：真机麦克风噪音/蓝牙/来电、完整60秒音频、iOS17、完整VoiceOver、系统中文组合输入与文件提供器的组合流程尚未实机验证；RMS单元测试不等同于物理麦克风验收。未改Worker部署，未声称E2B自动Agent循环或实时双向语音已完成。

真实服务补充：`LiveGrokVoiceFinal.xcresult` 两条通过，见 [真实转写](qa/compact-composer/45-live-inline-recording.png)、[原位修改](qa/compact-composer/46-live-inline-edit.png)、[发送后模型回复](qa/compact-composer/47-live-inline-sent.png)。用3.6秒合成中文PCM完成iPhone→Worker→豆包路径；没有采集个人声音。与脚本预览分开记录。

最终系统Body字号的长文复验：`GrokLongTextRelease.xcresult` 通过，重新采集50–57截图并打开56核实最新追加“新增内容”可见。所有本轮P1/P2已修复，剩余P3是参考设备/原生字体断行和未复制的系统键盘上下文差异，不影响输入核心行为。已重新安装并正常启动iPhone17模拟器中的最终构建，移除合成PCM；[正常启动](qa/compact-composer/62-normal-installed.png)仍使用已配置真实模型连接。

final result: passed

正常启动截图额外记录：既有回复中部分emoji显示为缺字方框，属于回复渲染范围，未纳入本轮输入面板修复；这里的passed仅指本轮输入交互范围，不代表整款客户端无缺陷。

## 2026-09-12 ChatGPT 式侧栏与 iPhone 远程控制

新增原生远程界面、本地中继到 Rust 核心的三条模拟器回归通过。最终选定参照为用户的 ChatGPT 截图。审批卡片、登录按钮、侧栏命中与长会话跟随问题已修复，实际截图和准确的验收边界见 [本轮远程视觉验收](../../docs/design/iphone/remote-control/design-qa.md)。真实 Cloudflare Access 登录与外网部署尚未完成。


## 2026-09-13：简洁首页与模型选择

首页收敛为居中欢迎语与底部输入框；本机和远程模型选择统一为「选择模型 → 思考 / 更多模型」。已完成两轮截图对照、iPhone 17 本机与远程 10 条 UI 回归，以及最终视觉修正后的 iPhone 17 / SE 共 4 条复验。此轮范围的 `final result: passed`，对照、截图与具体边界见 [本轮验收](../../docs/design/iphone/simple-home-models/design-qa.md)。未发布 TestFlight。
