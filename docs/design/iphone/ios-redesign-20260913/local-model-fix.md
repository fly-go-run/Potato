# iOS 本机模型与思考设置 · 2026-09-13

本机聊天现可从输入区打开原生模型面板，读取服务模型目录、选择思考模式和档位，并把选择用于实际请求。实现位于 SwiftUI 客户端及必要的 Worker 接口，尚未发布。

## 行为

- 选择按会话保存，重启后恢复；新对话从连接设置中的默认模型开始。切换模型清除上一模型的思考选项。
- 默认省略思考字段；关闭思考发送 `thinking.type=disabled` 并清除档位；开启及档位只展示已声明支持的值。自定义地址没有目录时可手动输入模型 ID，使用服务默认。
- 点击发送后同时固定地址、模型、模式、档位和本轮凭据。之后修改设置不会把新凭据发给旧地址。凭据仍只保存于 Keychain。
- **重新生成是一条新请求，使用当前选择**；旧回复版本保存原模型、模式及档位，可在版本切换后从回复菜单查看。远程待确认指令的重试则继续使用原始不可变参数，二者语义不同。
- 更换服务后，旧会话的选择不会套用到新服务。发送或重新生成前要求重新选择，保留草稿和原回复。
- 模型目录按服务地址缓存；刷新过程中更换服务或凭据，旧响应不能写入新配置。面板可刷新、搜索、手动输入并进入连接设置。

## 能力来源与接口

`LocalModelService` 从同源 `/models` 读取名称与可选能力。新 Worker `GET /v1/models` 沿用设备鉴权和限速，只返回上游目录与 `ALLOWED_MODELS` 的交集；只透传公开名称、模式与档位，不返回地址、密钥或任意元数据。禁止自动重定向；响应最多 1 MB、500 项，上游读取最多 15 秒。上游 404/405 时返回带 `configured` 标记的白名单，这仅表示配置允许，不证明当前可调用。

能力缺失时只对官方 HTTPS 地址上的两个精确公开 ID（`deepseek-v4-flash`、`deepseek-v4-pro`）使用官方文档声明的模式与 `low/high/max` 档位。服务明确返回空数组时保持不可选；未知型号、同名前缀、第三方代理和其他参数格式均不据此推断能力。依据：[DeepSeek 思考模式](https://api-docs.deepseek.com/guides/thinking_mode/)、[Chat Completions 参数](https://api-docs.deepseek.com/api/create-chat-completion/)，检查于 2026-09-13。

Worker 现在验证并转发 `reasoning_effort`，拒绝非字符串、非法格式及“关闭思考同时设置档位”。直接聊天及搜索后的继续调用保持原模型参数。客户端连接探测也不再按模型名前缀擅自关闭思考，仅在声明支持时使用关闭模式。

没有改动部署白名单、上游地址、Secret 或分发版本。当前配置的私有型号未提供可验证能力时仍使用服务默认；不能从名称推断可用性或过期状态。Worker 变更需要后续部署，当前线上服务不能视作已经具备这些新接口。

## 验证

测试使用独立存储、Keychain 测试账号与合成模型。URLSession 实际构造请求，测试协议捕获参数；页面回复回显收到的模型与思考字段。没有向真实供应商发送用户消息。

单元测试覆盖目录路径及鉴权、大小限制、无目录回退、明确不支持覆盖文档默认、未知能力、实际请求字段、请求开始时固定地址及凭据、旧数据解码、会话隔离、重启、重新生成的版本保存、服务变更保留草稿，以及刷新响应跨地址或凭据失效。

页面流程覆盖重启后的选择、关闭思考清除档位、切换模型重新生成与旧版本、通过面板进入设置后更换服务并重新选择，以及最大辅助字号下滚动选档、关闭面板、打开键盘和发送。

- **iPhone 17：98 项通过、0 失败、0 跳过**。包括全部 91 项单元测试、4 条模型页面流程、2 条原有设置/聊天历史流程和 1 条思考过程/重启流程。见 [摘要](../../../../native/potato-ios/qa/ios-audit-20260913/local-model-fix/iphone17-summary.json)、[完整日志与命令](../../../../native/potato-ios/qa/ios-audit-20260913/local-model-fix/iphone17-tests.log)、[截图索引](../../../../native/potato-ios/qa/ios-audit-20260913/local-model-fix/iphone17-attachments/manifest.json)。
- **iPhone SE：4 条模型流程全部通过**；缩短面板说明后，最大辅助字号流程再次通过。见 [完整流程摘要](../../../../native/potato-ios/qa/ios-audit-20260913/local-model-fix/iphone-se-summary.json)、[最终大字摘要](../../../../native/potato-ios/qa/ios-audit-20260913/local-model-fix/iphone-se-final-summary.json)。已查看普通字号、最大辅助字号和键盘截图；小屏需要滚动列表，顶部完成按钮与输入区发送按钮均可达。
- **Worker：36 项通过、0 失败、0 跳过，类型检查通过**。验证目录鉴权、同源请求、白名单过滤、公开字段清理、404 回退、重定向/超量响应拒绝，以及聊天和搜索后继续调用的参数。见 [测试日志](../../../../native/potato-ios/qa/ios-audit-20260913/local-model-fix/worker-tests.log)、[类型检查](../../../../native/potato-ios/qa/ios-audit-20260913/local-model-fix/worker-check.log)。
- **Release 模拟器构建通过**（arm64/x86_64），测试服务地址、启动参数和模型替身类符号未进入产物。见 [构建日志](../../../../native/potato-ios/qa/ios-audit-20260913/local-model-fix/release-build.log)、[产物检查](../../../../native/potato-ios/qa/ios-audit-20260913/local-model-fix/release-check.json)、[验证时源码摘要](../../../../native/potato-ios/qa/ios-audit-20260913/local-model-fix/source-sha256.json)。此结果不等于已完成真机签名或分发。

环境为 Xcode 26.3、iOS 26.3.1（23D8133）模拟器。原始结果保留于 `/tmp/potato-local-model-final-1.xcresult`、`/tmp/potato-local-model-se-1.xcresult` 与 `/tmp/potato-local-model-se-final.xcresult`。首次 SE 四条流程采用缩短说明前的文案，最终大字补测采用最终文案；模型协议和行为没有变化。

![本机模型面板，合成服务](../../../../native/potato-ios/qa/ios-audit-20260913/local-model-fix/iphone17-attachments/3578E08F-2B91-4ABD-BA1C-5BEBD759F9BC.png)

![iPhone SE 最大辅助字号和键盘](../../../../native/potato-ios/qa/ios-audit-20260913/local-model-fix/iphone-se-final-attachments/D1928781-FA7B-4615-A119-03C83E65FD17.png)

## 边界

这些检查不代替 TestFlight 真机或线上中继/供应商联调，不证明所有服务都支持扩展模型目录。模型目录刷新前无法知道服务未通知的能力变化；手动 ID 的实际可用性仍由服务验证。完整 VoiceOver 人工验收、最低 iOS 17 运行时、首页方案与远程断线过程仍待完成，见 [总验收矩阵](README.md)。
