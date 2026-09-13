# 远程草稿归属与回执迁移修复

2026-09-13，原 iOS 全面检查的第三轮。修复范围是 SwiftUI 远程草稿、待确认指令和它们的本地恢复；不依赖首页视觉稿选择。首页、侧栏手势、模型/思考配置和过程动画的整体目标仍保留，不能以本次数据修复替代完成。

## 行为变化

- 同账号多台电脑分别保存草稿。身份包含 relay 的协议/主机/端口、账号、电脑 ID，以及有明确类型的会话/项目/新任务范围。电脑改名不改变草稿归属，项目路径与会话 ID 不会碰撞。
- 待确认指令保存具体电脑身份、原文及原操作编号，发送前再次核对目标。更换电脑不能取出另一台电脑的指令，旧版无目标记录也不能直接发出。
- 草稿与待确认指令写入单份带文件保护的 JSON，发送前原子落盘。写入失败时不发起请求；损坏文件保留，不用空记录覆盖。
- 首次发送成功后，在同一次原子写入中把新任务/项目草稿转移到服务端返回的会话。继续输入、返回历史、重启后，都能从同一会话恢复追问。
- 回执到达时保留未发送的编辑。如果历史会话已有另一份草稿，两份分别保留，可通过“查看另外保存的草稿”切换；不自动拼接成一条指令。
- 旧版共享记录不猜电脑归属。用户在“恢复旧版草稿”内核对完整内容和目标后才能认领。认领保留原操作编号，不自动重发；原 UserDefaults 数据保留为档案，认领标记避免另一台电脑重复恢复。损坏旧记录显示独立提示并保留原字节。
- UI 测试草稿改用独立文件和旧版测试 suite；`--reset` 只在 DEBUG 测试入口重置测试文件，不读写个人远程草稿。

新存储位于应用 Application Support 的 `PotatoRemote/drafts.json`，替代 RemoteTaskView 直接使用 UserDefaults.standard。旧字段仅用于显式恢复。网络 RPC 的参数格式未改变，目标身份在手机发送边界验证，现有桌面回执的参数指纹不因此改变。

## 实现与验证范围

实现：[RemoteDrafts.swift](../../../../native/potato-ios/Sources/RemoteDrafts.swift)、[RemoteService.swift](../../../../native/potato-ios/Sources/RemoteService.swift)、[RemoteView.swift](../../../../native/potato-ios/Sources/RemoteView.swift)。跨电脑隔离的原失败测试已转为默认回归，改为实际读写草稿与校验发送目标，不再反射私有 View 字段。

单元覆盖包括：同账号跨电脑及相同项目路径、重命名/协议/端口/账号身份、首轮回执迁移、重启重试保持原文、已编辑追问、历史草稿冲突、延迟回执不清除新请求、确定拒绝后的新操作、旧版无目标记录、旧版追问归属、重复认领、损坏文件与保存失败、错误会话回执、空白指令。

页面覆盖包括：两台电脑切换和重启、首轮发送后的追问从历史重开、旧版确认/取消/恢复不自动发送、超时后重启并编辑再重试，以及既有远程点击导航/配对输入校验。使用 `--remote-draft-preview` 的合成账号与两台合成电脑。

网络恢复用 [回环服务](../../../../native/potato-ios/scripts/remote-draft-fixture.py) 测试真实 URLSession 请求。它只接受固定测试文字、不执行命令、不连接模型。`fixture-timeout` 首次请求已保存结果但返回 504，第二次必须保持相同 ID、目标 URL 与参数才能得到原回执。该证据验证手机的网络和页面恢复链；不是生产中继或真实 desktop core 的故障注入。

最终在 iPhone 17 / iOS 26.3.1 / Xcode 26.3 上通过 **59 项单元测试 + 6 条页面回归，共 65 项，0 失败、0 跳过**。xcodebuild 退出码 0。证据：[测试摘要](../../../../native/potato-ios/qa/ios-audit-20260913/draft-fix/final-summary.json)、[完整日志](../../../../native/potato-ios/qa/ios-audit-20260913/draft-fix/final-tests.log)、[源码指纹](../../../../native/potato-ios/qa/ios-audit-20260913/draft-fix/source-hashes.json)、[请求记录](../../../../native/potato-ios/qa/ios-audit-20260913/draft-fix/fixture-requests.jsonl)、[重试核对](../../../../native/potato-ios/qa/ios-audit-20260913/draft-fix/retry-verification.json)。本轮测试服务已停止。

已查看并接受 [旧稿归属核对](../../../../native/potato-ios/qa/ios-audit-20260913/draft-fix/attachments/E1A87FC3-035F-44C3-9324-4681F93F261D.png)、[只恢复不发送](../../../../native/potato-ios/qa/ios-audit-20260913/draft-fix/attachments/950096FE-0FA5-4DB2-AAEE-BA61D9612D8F.png)、[重试后保留编辑内容](../../../../native/potato-ios/qa/ios-audit-20260913/draft-fix/attachments/FE937A05-E5E7-476F-90D1-F4015BA571F3.png) 三张截图，说明与控件无截断；这不等于大字/小屏整体视觉验收。待确认期间的发送按钮虽然已禁用，视觉仍沿用旧版深色样式，后续输入区设计需明确区分禁用态。

早一轮页面测试发现了测试自身的光标假设：系统把插入点置于开头，断言却预设新字在末尾。现在在重试前记录实际编辑的完整文本，再断言回执后逐字保持一致；服务端另行核对两次请求的原文与编号，未放宽不可变发送要求。首次尝试的日志也已保存，不把它称为通过。

## 本轮没有解决的事项

本次 504 回执丢失恢复不等于解决 core 的 409 未完成回执。电脑在预留操作后、结果落盘前退出的找回机制仍待实现与故障注入。旧版已丢失的目标元数据也无法凭程序重建，只能由用户核对后恢复。

尚未做真机升级、iOS 17、锁屏期间写入失败或生产账号迁移验证，未发布 TestFlight。侧栏的两条滑动失败测试仍未修复；首页待选，模型/思考协议、过程动画、离线状态和完整移动端回归仍在原目标范围内。

## 复验

先启动仅回环监听的服务；日志只包含合成测试数据：

```sh
python3 native/potato-ios/scripts/remote-draft-fixture.py --log /tmp/potato-ios-draft-fixture.jsonl
```

在另一个终端运行：

```sh
DRAFT_RUN=$(mktemp -d /tmp/potato-ios-draft.XXXXXX)
TEST_RUNNER_POTATO_IOS_DRAFT_UI=1 xcodebuild \
  -project native/potato-ios/PotatoMobile.xcodeproj -scheme PotatoMobile \
  -destination 'platform=iOS Simulator,name=iPhone 17' \
  -derivedDataPath native/potato-ios/build -parallel-testing-enabled NO \
  -resultBundlePath "$DRAFT_RUN/checks.xcresult" \
  -only-testing:PotatoMobileTests \
  -only-testing:PotatoMobileUITests/RemoteUITests/testAccountDeviceDraftsStaySeparateAcrossNavigationAndRelaunch \
  -only-testing:PotatoMobileUITests/RemoteUITests/testLegacyDraftRequiresExplicitTargetConfirmation \
  -only-testing:PotatoMobileUITests/RemoteUITests/testAcknowledgedRemoteDraftReopensInItsConversation \
  -only-testing:PotatoMobileUITests/RemoteUITests/testTimedOutSendRetainsOriginalPayloadThroughRelaunchAndRetry \
  -only-testing:PotatoMobileUITests/RemoteUITests/testRemoteNavigationSearchAndDrawer \
  -only-testing:PotatoMobileUITests/RemoteUITests/testEmptyRemotePairingValidationAndLocalChatReturn test
```

完成后停止测试服务。不要把回环合成测试的结果称为真实电脑操作完成。
