# iOS 远程模型与思考设置 · 2026-09-13

已为远程任务接通原生模型面板与实际请求参数：从输入区选择电脑上的模型和已声明的思考档位，选择按目标电脑/会话保存，每次发送把模型与档位固定进持久化指令。工作树实现尚未发布；本机直连聊天的模型目录和思考设置仍待接通，不能把本轮远程实现当作全部目标完成。

## 用户行为

输入区默认“跟随电脑”。面板列出模型名称、所属服务和选择标记；思考区只显示电脑声明的档位，不按模型名称猜测能力。未知能力可用“服务默认”；电脑已有的手工配置可保留并明确显示。刷新位于导航栏，模型与档位占据主要内容区。

跟随电脑会在发送前读取当时的模型与档位，并固定到本次指令；明确选择“服务默认”会在实际模型请求中省略思考档位，而不是重新套用电脑的已保存档位。改变手机选择不会写入电脑全局模型设置。选择随新任务草稿迁移到首次创建的会话，其他电脑及新的空白任务保持各自选择。

运行中的补充指令沿用该任务配置，不能在中途切换模型；新协议还把补充指令绑定到一个具体运行编号。原运行已结束、被另一个运行替换，或原本空闲的会话已经开始运行时，拒绝改变发送语义，保留草稿并要求刷新后重新发送。已接收指令的重试仍先返回原回执。

待确认发送显示其固定模型和档位，并停用修改入口。网络失败后的重试、重启恢复、继续编辑下一条草稿都不改写原发送参数。模型删除或能力改变会提示重新选择，不会默默换模型或降档。

## 调用链

- core 的既有 `overview` 返回额外的版本化 `model_catalog`，仅包含公开名称、模型标识、已声明档位和当前选择。没有新增中继操作、供应商地址、密钥或手机凭据。
- `RemoteModelCatalog.resolve` 在发送前核验当前能力。`RemotePendingSend` 保存 `modelChoice` 或 `expectedRunID`，目标仍由 `RemoteTargetIdentity` 校验；`RemoteDraftRepository` 沿用原子的草稿迁移及回执处理。
- core `remote_mutation(send)` 把 `model_choice` 传到单轮 `remote_model`，任务启动时验证允许模型及档位，生成独立连接配置。`reasoning_effort: null` 显式清除本轮覆盖值。
- 既有模型层将档位写入 Chat Completions 的 `reasoning_effort` 或 Responses 的 `reasoning.effort`。设置不修改 `active` 或供应商模型配置。
- `expected_run_id` 在持有运行锁的 `steer` 中再次核验；不匹配/已结束返回 412。手机把此明确拒绝从 pending 状态释放，并保留文字。409 未完成回执的旧问题仍另外待处理。
- 老记录缺少新字段仍可读取，旧 pending 不会在解码或重试时补上新的模型覆盖。旧电脑不提供目录时，面板说明需更新；原有跟随电脑发送仍可用，但没有新协议的配置固定与精确运行绑定保证。

## 验证证据

使用 Xcode 26.3、iOS 26.3.1（23D8133）模拟器。手机页面通过真实 URLSession 请求 loopback 合成远程服务；core 测试独立捕获其真正发出的 HTTP 请求。二者分别验证协议边界，不冒充手机经线上中继到真实供应商的完整联调。

- core 相关 11 项通过；共享 core 单元回归 **166 项通过、0 失败、11 项既有 opt-in 检查未启用**。未启用项依赖真实桌面驱动、供应商或宿主沙箱，不是本轮新增跳过。见 [相关测试](../../../../native/potato-ios/qa/ios-audit-20260913/remote-model-fix/core-targeted-tests.log)、[完整单元日志](../../../../native/potato-ios/qa/ios-audit-20260913/remote-model-fix/core-tests.log)。
- 实际 HTTP 捕获覆盖两种模型协议、明确档位及服务默认省略参数；另覆盖目录不含地址/密钥、未知/失效能力拒绝、拒绝前不创建聊天、电脑配置不变、回执去重和运行编号匹配。
- iOS 单元检查包含原有回归及新增模型解析、服务默认、旧数据解码、运行编号、跨电脑隔离、选择保存、回执迁移和不可变参数。

最终 iPhone 17 **89 项通过、0 失败、0 跳过**：全部 81 项单元测试，5 条模型面板流程，2 条旧草稿回归和 1 条原生返回回归。见 [摘要](../../../../native/potato-ios/qa/ios-audit-20260913/remote-model-fix/iphone17-summary.json)、[日志与完整命令](../../../../native/potato-ios/qa/ios-audit-20260913/remote-model-fix/iphone17-tests.log)、[截图索引](../../../../native/potato-ios/qa/ios-audit-20260913/remote-model-fix/iphone17-final-attachments/manifest.json)。原始结果 `/tmp/potato-remote-model-final-7.xcresult`。

SE 五条模型流程通过后，截图发现大字图标与设备状态拥挤；修复后又在 SE 验证大字号下滚动选档、关闭面板、打开键盘、输入及发送完整指令，该条通过。前后均 0 跳过；五条与补验并非六个独立用例。见 [SE 五条摘要](../../../../native/potato-ios/qa/ios-audit-20260913/remote-model-fix/iphonese-summary.json)、[日志](../../../../native/potato-ios/qa/ios-audit-20260913/remote-model-fix/iphonese-tests.log)、[修复后大字号摘要](../../../../native/potato-ios/qa/ios-audit-20260913/remote-model-fix/large-text-summary.json)、[日志](../../../../native/potato-ios/qa/ios-audit-20260913/remote-model-fix/large-text-tests.log)。原始结果分别为 `/tmp/potato-remote-model-se-4.xcresult` 和 `/tmp/potato-remote-model-large-6.xcresult`。

[Release simulator 构建通过](../../../../native/potato-ios/qa/ios-audit-20260913/remote-model-fix/release-build.log)，不等于真机签名或 TestFlight 发布。[合成请求检查](../../../../native/potato-ios/qa/ios-audit-20260913/remote-model-fix/request-check.json)核对了 4 组重复操作编号的完整目标和参数均不变，运行中追问都绑定指定 run 且不带模型覆盖；[原始请求记录](../../../../native/potato-ios/qa/ios-audit-20260913/remote-model-fix/fixture-requests.jsonl)仅含本轮合成数据。

模型页面测试与现有远程草稿测试一样需要显式启用，普通测试不会在缺少 fixture 时误报产品失败。先在单独终端运行 `python3 native/potato-ios/scripts/remote-draft-fixture.py --models --log /tmp/model-fixture.jsonl`，再用 `TEST_RUNNER_POTATO_IOS_DRAFT_UI=1 xcodebuild ... test`；完整测试选项见上述日志。89 项运行后仅补充了这个测试入口条件，断言和产品源码未改变，[测试工程另行 build-for-testing 验证通过](../../../../native/potato-ios/qa/ios-audit-20260913/remote-model-fix/test-harness-build.log)。前一次 SE 自动测试未滚动就查找未加载的档位行，修正为实际滚动后通过；[失败日志](../../../../native/potato-ios/qa/ios-audit-20260913/remote-model-fix/se-before-scroll-test-fix.log)保留。没有移除选择值或请求参数断言。

## 实际截图

已打开核对两种尺寸的原生模型面板、未知能力、待确认发送及大字号输入。以下为合成服务驱动的应用截图，不是设计概念图。

![最终 iPhone 17 模型与思考面板](../../../../native/potato-ios/qa/ios-audit-20260913/remote-model-fix/iphone17-final-attachments/8C01A22F-5912-46B6-970F-2D24EFA7F3A1.png)

![SE 大字号键盘与发送按钮](../../../../native/potato-ios/qa/ios-audit-20260913/remote-model-fix/large-text-attachments/237D0430-2A73-4A23-8F0F-3A7928A099FA.png)

[当前源码 SHA-256](../../../../native/potato-ios/qa/ios-audit-20260913/remote-model-fix/source-hashes.json)。

## 大字号检查

实际 SE 截图发现远程页的设备名与状态互相挤压，以及固定 44pt 的语音/发送按钮中图标随系统字号放大而越界。已让无障碍字号的设备与状态纵向排列，图标保持适合触点的尺寸，文字仍按系统字号缩放；模型标签可换行，输入框在大字号下最多显示三行。空白新任务不再自动滚到介绍底部。

模型列表是原生可滚动列表，最大字号下可以滚动到档位、选择并关闭。不能把这几项检查扩展成整个应用的无障碍验收。

## 未完成范围

本机直连模型/思考设置、远程陈旧过程状态与断线恢复、409 未完成回执、首页所选方案实现仍待继续。没有发布新 iOS 版本或更新运行中的桌面客户端；线上中继、真实模型、真机、最低 iOS 17 和完整 VoiceOver 人工操作均无本轮完成证据。
