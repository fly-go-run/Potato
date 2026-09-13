# 原始目标验收审计

2026-09-12。目标未完成，不能以本机测试通过替代真实接入与真机体验。未缩减原目标，也未宣称完整替代ChatGPT/Claude。

| 原目标要求 | 当前直接证据 | 未完成/证据边界 |
| --- | --- | --- |
| iPhone原生、选择的第1版浅色 | SwiftUI Sources、Xcode构建、qa/revisions/comparison.png 源稿对照 | 已验证iOS26.3模拟器；最低iOS17与真机未验收 |
| 对话/输入/键盘 | PrototypeTests发送/停止/重新生成/搜索；LiveDeepSeekFinal直接模型联调；LiveWorker经Worker连接、问答、重启恢复 | 真实额度耗尽/断网组合仍待验证 |
| 历史/内容不丢失 | WorkspaceTests持久化与损坏保护；RevisionTests与ReplyBranchTests；文稿恢复UI | 本机持久化证据，不代表跨设备同步 |
| 附件 | 实际系统照片导入/预览/关闭/移除UI；文本/PDF提取测试；LiveWorkerPhoto示例照片经Worker识别正确 | 用户云盘提供器端到端仍需设备登录与文件 |
| 文稿预览/编辑/分享 | 文稿UI编辑/清单/复制/系统分享/重启；版本恢复与SE截图 | 系统分享面板与实际.md已验证，不代表所有第三方接收应用 |
| 设置/连接/反馈 | DesktopAlignedRegression 34项通过；SignedKeychain真实写读；LiveWorker真实连接成功截图与问答通过 | 真机权限尚未验证 |
| 无障碍/原生体验 | 最大辅助字号与SE模拟器检查、按钮标签与44pt目标、Reduce Motion代码 | 完整VoiceOver、实际麦克风授权与识别质量尚未验证 |
| Cloudflare后端、无VPS | potato-iphone-api已部署；健康200/未认证401；9项处理器测试；iPhone经Worker真实文字与照片问答通过 | 单用户设备令牌；没有账号系统、云端同步或严格全局费用上限 |
| 对标ChatGPT/Claude | 已研究官方发布者截图并按所选方向实现核心流程 | 不是竞品实机完整审查或全量功能对等 |

用户明确要求沿用本地土豆模型后，从原生数据库只读取得当前配置：DeepSeek / deepseek-v4.1-flash-expires-on-0910 / OpenAIChatModel。尽管名称含到期日期，2026-09-12实测仍返回200与完整流。模拟器已导入同一配置到Keychain，直接供应商真实文字流通过，合成红图识别正确。桌面设置未改变。

此前自动审批要求明确授权；用户回复“可以”后，已将供应商密钥保存为Cloudflare Secret，部署 `https://potato-iphone-api.pal-xu.workers.dev`，并为模拟器Keychain配置独立设备令牌。`LiveWorker.xcresult` 通过连接测试、真实文字问答、重启恢复；`LiveWorkerPhoto.xcresult` 通过系统花朵示例照片导入、预览和真实图片问答。截图见 `qa/worker/`，部署记录见 `../potato-worker/deployment.json`。真实手机签名、录音、VoiceOver与云盘提供器仍需设备条件。

## 回复、多图与E2B增量

`ReplySandboxRegression.xcresult` 完整37项通过；图片预算调整后的 `MultiImageBudget.xcresult` 29项通过。回复操作、文字选择、四图排序/预览切换及重启恢复已验证。`ReplyLiveWorker.xcresult` 中四图真实模型用例通过，另一个Python入口用例因父级可访问标识覆盖按钮而失败；移除覆盖后 `SandboxSetupVerified.xcresult` 独立复验通过。未把中间失败的整套结果记为通过。

当时Worker13项测试与类型检查通过；用户提供有效E2B凭据并明确授权保存后，Secret变更部署版本为 `0fd1cca6-d9c6-4a18-b4b6-74d54dfa7d9d`，复用历史 `chat-web-office-pdf` 模板。原有503未配置测试保留为早期证据。

`LiveE2BFiles.xcresult` 真实iPhone→Worker→E2B执行、PNG/PDF/DOCX/XLSX回存、PDF预览和重启恢复通过。截图发现预览与PNG文件重复，按完整像素去重后 `SandboxArtifactDedup.xcresult` 29项单元测试通过；`LiveE2BVerified.xcresult` 复验记录见design-qa。同一SDK另以三行合成CSV验证输入上传和合计60。手机系统文件提供器→勾选→分析的完整组合仍待验证，沙箱尚未实现自动Agent工具循环。

详细历次测试、失败修复与截图见 [design-qa.md](design-qa.md)；产品能力边界见 [IMPLEMENTATION.md](IMPLEMENTATION.md)。

## 自动搜索与豆包语音增量

用户明确授权迁移桌面Exa与豆包凭据后，已保存到Worker Secret。`SearchSpeechFinalUnits.xcresult` 31项单元测试、`SearchSpeechUIRegression.xcresult` 9条UI回归通过；`LiveSearchSpeechVerified.xcresult` 两条真实UI流程通过：模型自动调用Exa、来源展示及重启保留；3.6秒合成中文音频经iPhone→Worker→豆包转写，确认后仅放入输入框。没有联网开关，仅Exa一个搜索引擎。最终来源无标题回退显示另经编译验证。

Worker18项测试与类型检查通过，版本 `2cfc1662-2d0b-475e-8cbc-8e5a717b73d2`；部署后健康200，聊天/语音/沙箱未认证均401。语音覆盖ASR，不是实时双向语音对话；真机麦克风、长录音、来电中断未验收。Exa已自动调用，E2B仍是显式点击运行。截图与中间失败原因见design-qa。

## 语音方向1：原位开始，直接发送

2026-09-12。已按选定参考图实现原生交互：点麦克风直接连接录音并实时转写；发送结束采集，排空PCM尾部并等final后提交一次；修改在收尾后打开系统键盘；取消恢复原文字与附件。按进入时的UTF-16选区插入，后台、切会话、失败、空final与60秒上限保留未发送草稿，旧录音事件不能覆盖手工修改或写入新会话。

- `InlineVoiceUnits.xcresult`：38项单元测试通过（新增7项语音状态测试）。
- `InlineVoiceUI.xcresult`：首轮13条UI用例中3条失败，暴露输入框首字后失焦及一次回复菜单点击失败；不算作全部通过。
- `InlineVoiceUIVerified.xcresult`：修复焦点绑定后，4条新语音流程及历史搜索、回复/多图两条回归共6条通过。覆盖直接发送、修改后输入与重启恢复、取消保留原文、后台保留未发送草稿、大字操作可达。
- `InlineVoiceFinalVerified.xcresult`：录音正文改为24pt动态字号、紧凑操作区限制至xxxLarge后，38项单元测试及正常/大字模式两条UI用例再次通过。
- `LiveInlineVoice.xcresult`：真实豆包原位修改、直接发送两条通过。3.6秒合成中文PCM由模拟器走现有Worker与豆包；修改仅留草稿，直接发送等待尾字并收到当前DeepSeek模型回复。
- `InlineVoiceSmallScreenVerified.xcresult` 与 `InlineVoiceLayoutFinal.xcresult`：修复SE正文高度后，两种设备分别通过正常/大字模式两条语音UI用例，最终截图重新对照检查。正常字号两行完整，最大辅助字号首行完整且可滚动，三个操作完整可见。

UI脚本只存在DEBUG双开关 `--ui-testing --voice-preview`，和真实豆包联调分开；没有用脚本文字证明识别成功。首个Final构建因默认沙盒无法连接CoreSimulator退出70；工具自动审批允许系统服务访问后正常执行。Worker本轮未改接口或部署，模型与凭据沿用现有配置。小屏与最终截图迭代详见 [design-qa.md](design-qa.md)。

真机麦克风授权与音质、噪声/蓝牙、完整60秒音频、实际来电、iOS17与完整VoiceOver仍未实机验收。模拟器后台和注入的音频中断状态已覆盖，不等同真实电话测试；尚不提供连续双向语音对话。

## Grok参考面板、长文编辑与本地音量反馈

2026-09-12。用户进一步要求按Grok截图收紧布局、移除键盘按钮、黑色文字突出重点、波形响应真实声音。最终行为：文字逐行增高、封顶滚动、展开为同一份原生草稿；语音X取消、点击文字修改、勾号或波形上滑直接发送，波形左滑取消。所有发送仍等待final，不产生额外“确认放入输入框”步骤。

- `CompactComposerFirst.xcresult`：38单元＋5UI通过；`CompactComposerSE.xcresult`：3UI通过。首轮功能断言未发现回到最新的实际滚动位置错误，不能把这些通过解释为视觉验收通过。
- `CompactComposerVerified.xcresult`：38单元＋4UI通过，但截图仍发现回到最新停在中间。改为布局完成后定位插入点、文末使用实际contentSize偏移，并停止旧滚动动画；后续截图56可见最新追加文字。
- `GrokCompactFinal.xcresult`：8UI通过，含精确500/1000字、展开/收起/发送、缩回、点击转写文字修改、取消/后台/重启保留、长语音展开、大字可达和参考短句。
- `GrokCompactSE.xcresult`：SE上3UI通过，含长文、辅助字号、波形上滑发送/左滑取消。
- `GrokVoiceMeterFinal.xcresult`：40单元＋3UI通过。新增PCM音量强弱/正负幅度/静音/最大值测试，以及波形静音归零和旧事件隔离。真实音量在本地音频采样后更新，不等待网络上传；DEBUG预览取模循环已经移除。

默认字号、键盘打开时实测：iPhone17正文26.33pt起步，500和1000字均158pt；SE起步26.5pt，长文108.5pt。长文不截断，最大高度按设备可用空间收紧，展开提供更多阅读空间。实机麦克风/噪音/蓝牙、iOS17、完整VoiceOver、中文拼音组合与系统文件提供器附件的组合操作仍待真机验收；本轮不把代码保护等同于全部实测通过。

`GrokFinalAppearance.xcresult` 在iPhone17及SE分别通过短句和辅助字号，共4UI用例；`LiveGrokVoiceFinal.xcresult` 两条真实豆包联调通过：轻点文字收尾进入键盘编辑、勾号发送等待final后获得真实模型回复。使用3.6秒合成中文PCM，不是个人录音，也不替代真机麦克风验证。没有重新部署Worker或变更模型配置。

`GrokLongTextRelease.xcresult` 通过最终字号下的完整长文UI复验，重新检查500/1000字、展开/收起/发送、持续转写的回看与恢复；截图56实际可见新增文字。最终构建已安装到iPhone17模拟器并正常启动，合成PCM已移除；真机安装/TestFlight不在本次已完成范围。
