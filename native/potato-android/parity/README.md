# iOS → Android 0.2.0 详细对照

对照基准是本次开始时冻结的 **iOS 0.2.2（2026091304）**，不是旧网页客户端。冻结源码的 39 个文件摘要见 [ios-source-baseline.json](ios-source-baseline.json)。没有修改 iOS、桌面 GPUI、Rust 后端或线上 Worker。

旧 Android 0.1.0 的差异确实超出系统外观：有不同的页面结构，也有缺失的记忆、附件图库、侧栏及状态处理。本轮围绕这些差异重做了应用内界面并补上功能。下面按“原差异—实现—验证”逐项记录，测试通过不等同于所有平台细节逐像素相同。

| 范围 | 0.1.0 与 iOS 的差异 | 0.2.0 的实现 | 验证内容 |
| --- | --- | --- | --- |
| 首次进入/首页 | 旧版额外显示副标题、快捷问题，图标和色调不同；没有首次示例文稿。 | 恢复 iOS 同文案的周末计划；新对话使用原版 PotatoMark、暖白背景和简洁欢迎语。示例不进入历史检索。 | 首次样例、空消息禁发、重启与新建行为 |
| 输入框 | 灰色输入区，展开按钮常驻，模型与发送按钮层级不同。 | 白色圆角输入卡，模型胶囊、加号、麦克风和发送图标；按长文溢出显示展开；键盘弹出时缩小正文区。 | 长草稿、1.5 倍字号、展开编辑、跨会话草稿隔离 |
| 侧栏/历史 | 居中弹框，只列少量最近记录。 | 左侧栏和主页面偏移；资料库、远程、全部最近记录、置顶与搜索；底部对话/设置；左滑关闭。 | 12 条合成会话、入口与搜索；横向附件不触发侧栏 |
| 模型和思考 | 弹窗逐层堆叠，重新选择同一模型丢失思考档位。 | 模型/思考/更多模型三层底部面板，选中勾选、推荐项和搜索；同模型保留档位，换模型采用服务默认值。 | 同模型重选、取消、服务声明档位、云端列表 |
| 回复操作 | 助手标签和文本按钮多，旧消息也有重试按钮。 | 复制、重试、更多图标；仅末条回复可原位重答；旧消息分支；换模型重答不改输入区模型或草稿。 | 真实 HTTPS 流式、停止、取消换模型、重试原参数 |
| 回复版本 | 版本入口和状态缺少内联对应。 | 回复旁上一版/下一版与页码；保存每版正文、思考、模型、执行信息；历史来源绑定所选回复版本。 | 版本往返及版本标识恢复，旧回复保留 |
| 正文/代码/计算 | Markdown 与计算入口较粗，代码块不能逐个执行。 | 正文层级、横向代码和表格、代码复制；每个 Python 代码块有独立计算入口和结果面板，附件按选择提交。 | Markdown 表格与请求序列化；真实 E2B 服务未用个人账号验收 |
| 工作文稿 | 文稿是弹窗，编辑直接改变原文。 | 聊天内文稿面板、展开/收起、取消/保存、清单勾选、历史预览和恢复、复制及 Markdown 分享；顶部编辑操作。 | 取消不写回、恢复保留历史、首次示例和自定义文稿 |
| 照片/文件 | 待发只显示名称，图片依赖外部查看器。 | 72dp 缩略图、单个删除、四图上限、单图/双列已发图；应用内图库翻页/缩放，PDF 分页预览和文件分享。 | 四图删除与发送、图库翻页、两页 PDF 实际渲染 |
| 语音 | 额外的编辑/发送行，手势缺失；空最终结果会覆盖部分转写。 | 紧凑波形与取消/确认；左滑取消、上滑发送、点文本结束编辑；按原光标合并，空最终结果保留部分文字但不发送。 | 真实 AudioRecord + 本地 WSS 夹具，空 final 与草稿保留 |
| 设置/登录 | 居中表单、云端仍露出手填字段，取消语义不同。 | 分组底部面板、顶部取消/保存、云端隐藏手填模型与接口、触感开关、连接测试。 | 取消不修改设置，凭据加密与服务地址隔离 |
| 记忆与历史 | Android 缺失该功能。 | 同步和自动记忆开关、排除会话、手动记忆编辑/忘记、来源定位；仅同步已完成文字，附件与示例排除。 | 实际 HTTPS 请求、同步回执、409 冲突停止覆盖、关闭后仅清理排除项 |
| 远程首页/管理 | 逐个弹窗选电脑，缺聚合首页与完整管理。 | 全部/电脑筛选、置顶/项目/会话分组、底部搜索与新任务；配对、移除本机配对、退出手机登录、撤销电脑远程访问。 | 空状态与非法配对、移除单台保留另一台凭据；账号撤销未访问真实服务 |
| 远程任务 | 文本操作按钮，过程分类、模型选择和待确认指令不统一。 | 图标输入区、三层模型选择、跟随电脑；区分思考/工具/回复与失败/取消；状态过期限制操作；保留待确认操作编号和目标。 | 真实 HTTPS 夹具的丢失回执、在途新草稿保留、过程状态分类 |

## 实际截图

左侧为 iOS 模拟器实际页面，右侧为 Android 模拟器实际页面。两台模拟器分辨率、系统字体与状态栏不同；附件和语音使用合成内容，重点核对布局、入口与交互状态。

| 页面 | iOS | Android 0.2.0 |
| --- | --- | --- |
| 空白首页 | [查看](/Users/liuxu/lifeProjects/Potato/native/potato-android/parity/baseline-ios/01-home.png) | [查看](/Users/liuxu/lifeProjects/Potato/native/potato-android/parity/updated-android/final/01-home.png) |
| 模型面板 | [查看](/Users/liuxu/lifeProjects/Potato/native/potato-android/parity/baseline-ios/captures/D6F2B6E8-E9F9-461C-8F72-368821D0AC7E.png) | [查看](/Users/liuxu/lifeProjects/Potato/native/potato-android/parity/updated-android/final/02-models.png) |
| 回复更多 | [查看](/Users/liuxu/lifeProjects/Potato/native/potato-android/parity/baseline-ios/captures/BAE9AD2A-13D2-44EF-A18D-A066BE344A65.png) | [查看](/Users/liuxu/lifeProjects/Potato/native/potato-android/parity/updated-android/final/05-reply-menu.png) |
| 首次示例文稿 | [查看](/Users/liuxu/lifeProjects/Potato/native/potato-android/parity/baseline-ios/details/B817ED66-8A67-47A0-B25B-2D446101FFEF.png) | [查看](/Users/liuxu/lifeProjects/Potato/native/potato-android/parity/updated-android/final/12-first-run-document.png) |
| 设置 | [查看](/Users/liuxu/lifeProjects/Potato/native/potato-android/parity/baseline-ios/details/AFC5239D-FD4F-437C-95E2-360C5F060806.png) | [查看](/Users/liuxu/lifeProjects/Potato/native/potato-android/parity/updated-android/final/08-settings.png) |
| 侧栏 | [查看](/Users/liuxu/lifeProjects/Potato/native/potato-android/parity/baseline-ios/details/3384A0B1-0887-4755-B26B-EEA8CF35103D.png) | [查看](/Users/liuxu/lifeProjects/Potato/native/potato-android/parity/updated-android/final/09-sidebar.png) |
| 待发四图 | [查看](/Users/liuxu/lifeProjects/Potato/native/potato-android/parity/baseline-ios/details/2D4A5212-4E4E-435E-92EB-BCD64A785F57.png) | [查看](/Users/liuxu/lifeProjects/Potato/native/potato-android/parity/updated-android/final/16-pending-images.png) |
| 语音转写 | [查看](/Users/liuxu/lifeProjects/Potato/native/potato-android/parity/baseline-ios/details/5C1C44FB-5F36-40C5-A0FF-F384F0835895.png) | [查看](/Users/liuxu/lifeProjects/Potato/native/potato-android/parity/updated-android/final/13-voice.png) |
| 远程空状态 | [查看](/Users/liuxu/lifeProjects/Potato/native/potato-android/parity/baseline-ios/remote/ABA5C742-C379-4990-8009-0A00A2596135.png) | [查看](/Users/liuxu/lifeProjects/Potato/native/potato-android/parity/updated-android/final/11-remote-empty.png) |

Android 补充检查：大字体与长输入、展开编辑、图库、发送后图片网格、PDF 分页，见 `updated-android/final/14-*` 至 `19-*`。旧 Android 实际首页保留在 `baseline-android/01-home.png`。并排浏览所有对照图可打开 [comparison.html](comparison.html)。

## 验证边界

- 系统字体、状态栏、键盘、照片/文件选择、分享、权限和朗读使用 Android 实现；不会呈现 iOS 系统组件。应用内图标用真实矢量图，品牌图使用 iOS 原资源。
- 侧栏已支持滑动开关，但拖拽跟手曲线和底部面板动画仍由 Android 实现。原生控件字形、字重、滚动和行距有平台差异；不宣称逐像素复制。
- 最终自动测试、签名、安装和旧版覆盖升级结果见 [../ACCEPTANCE.md](../ACCEPTANCE.md)。服务协议测试使用合成 HTTPS/WSS 夹具，未使用个人账号操控真实电脑。
- 尚未连接 OPPO Find X8s+。ColorOS 安装流程、实际麦克风质量、国内移动网络、真实账号登录/远程电脑/E2B 端到端需要在该手机环境确认。现有模拟器证据不能代替真机结果。
