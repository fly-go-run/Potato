# iOS 第二轮检查：可复现缺陷与实现边界

后续状态：本页保存修复前的检查证据。已修复跨电脑草稿隔离、首轮会话草稿迁移与测试存储隔离，并验证 504 回执丢失后的同编号重试，见 [草稿修复与 65 项验证](draft-fix.md)。随后已修复 [侧栏、返回与滚动冲突](sidebar-fix.md)，iPhone 17 的 81 项检查通过。现已修复 [本机思考流与搜索转发](reasoning-fix.md)。下文为当时证据；随后接通 [远程模型/档位覆盖](remote-model-fix.md)。本机目录与实际思考参数也已接通，见 [本机模型设置](local-model-fix.md)。远程断线与后台状态已修复，见 [远程状态反馈](remote-process-fix.md)。[停止任务现已绑定确认时的运行](exact-stop-fix.md)，避免旧确认误停新一轮。[409 回执恢复与未知记录处理](receipt-recovery.md)已接通：仅凭精确持久记录恢复，无法判定的记录保留原编号供用户核对。

2026-09-13。本轮继续原目标，在首页视觉方向待选期间检查调用链和回归。没有修改产品运行时代码；新增四项检查测试，其中三项在当前实现下失败，一项为通过的竖向滚动对照。以下所有问题仍待修复，不能把测试基线通过当作问题解决。

## 本轮权威结果

Xcode 26.3，iPhone 17 模拟器。模拟器列表称运行时 iOS 26.3；本次 xcresult 报告实际系统 **26.3.1 / 23D8133**。使用当前工作树重新编译的 SwiftUI iOS 工程、独立本地测试工作区与合成远程设备，未向真实电脑发送任务。

| 测试 | 结果 | 能证明什么 |
| --- | --- | --- |
| 既有 PotatoMobileTests 全部 43 项 | 43 通过 | 此次编译后的本地存储、流解码、语音等既有单元测试基线 |
| `testAccountDraftsAreScopedToTargetComputer` | **失败** | 同账号的两台不同电脑，实际 RemoteTaskView 草稿键相同 |
| `testSidebarOpensWithHorizontalDrag` | **失败** | 根页面从左向右拖动未打开抽屉，实际进入了项目 |
| `testSidebarClosesWithHorizontalDrag` | **失败** | 点击打开后，抽屉内从右向左拖动未关闭 |
| `testVerticalScrollDoesNotOpenSidebar` | 通过 | 此次纵向滚动未误开抽屉；不代表尚未实现的手势已正确锁轴 |
| `testRemoteNavigationSearchAndDrawer` | 通过 | 点击抽屉、搜索与清空搜索的对照路径正常 |
| `testEmptyRemotePairingValidationAndLocalChatReturn` | 通过 | 无设备入口、非法配对码提示及返回本地聊天正常 |

针对性运行共 **6 项：3 通过、3 失败、0 跳过**，xcodebuild 退出码 65。失败是正常测试断言产生，不是编译失败，也没有用 expected-failure 包装。新增四项测试暂用 `TEST_RUNNER_POTATO_IOS_AUDIT=1` 显式启用，修复时应提升为默认回归，不能一直跳过。

证据：

- [基线摘要](../../../../native/potato-ios/qa/ios-audit-20260913/unit-summary.json)、[原始日志](../../../../native/potato-ios/qa/ios-audit-20260913/unit-tests.log)。43 项基线运行发生在新增四项检查之前。
- [缺陷回归摘要](../../../../native/potato-ios/qa/ios-audit-20260913/regression-summary.json)、[原始日志](../../../../native/potato-ios/qa/ios-audit-20260913/regression-tests.log)。
- [测试附件索引](../../../../native/potato-ios/qa/ios-audit-20260913/regression-attachments/manifest.json)，包含原始截图、失败层级和 XCTest 录屏。已人工查看下面两张失败截图。
- 原始 xcresult 留在 `/tmp/potato-ios-audit-20260913-unit.xcresult` 与 `/tmp/potato-ios-audit-20260913-regressions.xcresult`；持久证据已保存到上面的仓库 QA 路径。

![左拖后抽屉仍然打开](../../../../native/potato-ios/qa/ios-audit-20260913/regression-attachments/60452136-7626-46A3-ABC4-87E35E50D560.png)

![右拖后进入项目，未打开抽屉](../../../../native/potato-ios/qa/ios-audit-20260913/regression-attachments/153D4DD8-75B6-4E16-9643-4FAAC70CF8CB.png)

## 优先修复项

### P1：同账号多电脑共用草稿及待确认指令

`RemoteDevice.account` 为账号凭据定义身份，不包含电脑 ID。同账号共享凭据是合理的，但 `RemoteTaskView.init` 用它加 `chatID / projectPath / new` 生成草稿键。两台电脑的新任务必然同键，两个电脑上相同项目路径也会同键。新回归从两份实际 View 读取私有存储身份，已确认键相同；不是在测试里重写键公式。

同一键还保存 `-pending`。代码路径允许在 A 电脑发送超时后，返回 B 电脑的新任务，读到 A 的 pending，再把该载荷通过当前 B 的 RPC 重试。后者是源码确认的风险路径，本轮没有真的向两台电脑执行指令。对既有聊天可能报找不到聊天；对 `chatID == nil` 的新任务则可能在错误电脑建立任务。修复必须把草稿和 pending 都绑定到 relay、账号、电脑、会话/项目范围；旧版无法确定归属的记录必须保留待确认，不能猜目标后自动迁移发送。

位置：[RemoteService.swift](../../../../native/potato-ios/Sources/RemoteService.swift) 的 `RemoteDevice.account`；[RemoteView.swift](../../../../native/potato-ios/Sources/RemoteView.swift) 的 `draftKey`、`onAppear`、`transmit`。

### P1：已接收但未落回执的指令可能永远无法解除待确认

core 先写 `remote_receipt:{id}` 指纹，执行后再写结果。如果电脑在两次写入之间退出，重试只得到 409“该操作已接收”。iOS 对 409 保留 pending，发送按钮因 pending 非空禁用；刷新成功也不解除 pending。新任务此时 `chat == nil`，刷新直接返回，缺少用操作 ID 找回任务的入口。当前代码可证明恢复路径缺失；本轮未做真实断电故障注入，不声称已经复现实机卡死。

修复应查询持久回执与派生 session，区分已执行、明确未执行和未知，支持找回会话；不能直接换操作 ID 重发，也不能静默清空未知指令。

位置：[core remote.rs](../../../../native/potato-core/src/remote.rs) 的 `remote_command` 预留回执；iOS `transmit`、`refresh` 和 pending 操作区。

### P1：模型和思考只改界面会无效，当前传输链缺字段

本机 `ConnectionSettings` 没有思考能力/档位，实际聊天请求只发送 model/messages/stream；SSEDecoder 不接收 `reasoning_content`，ChatMessage/回复版本也没有对应过程字段。远程 `RemotePendingSend` 和 core 的 `remote_mutation(send)` 均未携带模型/effort 覆盖。必须接通能力来源、持久化、不可变重试载荷和实际请求，不能以选择器显示值作为已生效证据。

思考内容只展示服务公开返回的内容/摘要；没有数据时保持真实等待状态。需覆盖思考先到、正文先到、两者同帧、只有思考无正文、停止、失败及旧记录兼容。桌面已有能力声明机制，不能按模型名字猜档位。

### P2：侧栏没有拖动处理

当前 `WorkspaceView` 仅布尔开关和最终 offset，没有抽屉拖动状态。XCTest 确认左右方向均失败，点击对照正常。下一步除使两条失败转绿，还要检查短拖取消、纵向锁轴、键盘/语音、横向代码内容、子页原生返回和 Reduce Motion。不能只用根页的长距离拖动证明整个手势范围正确。

### P2：刷新失败仍把旧的运行状态当作当前状态

`RemoteTaskView.running` 仅看最后一份 snapshot；刷新异常只赋 error，不标记快照过期。因此断线时页面可同时显示网络错误、“正在执行”和循环过程提示。应显示“连接中断，最后确认正在执行”，保留内容，但不能表现成已确认持续运行；恢复连接后按新状态更新。此项为源码证据，尚待可控断线 UI 验证。

另外 `RemoteStore.refresh` 恢复成功时未清空上一次刷新 error，会出现已在线但仍留旧错误提示。清理需要按本次刷新所有设备结果处理，不能让后一台成功掩盖前一台失败。

### P2：新任务得到会话 ID 后，草稿存储范围没有转换

`draftKey` 是初始化时的常量；首次发送成功仅把 `chat` 更新为服务端会话。之后在这个页面写追问，仍保存到原来的 `new` 或项目槽。退出后从历史打开该会话会读取另一份键，草稿看似丢失；进入“新任务”又可能看到原会话的追问。源码路径已确认，须在修复时做“首轮成功 → 写追问 → 返回 → 历史重开”的 UI 回归，并让 pending 与草稿一起迁移。

### P2：远程长中文快照的服务器与客户端大小预算不一致

core 允许最近 120 条、每条 8,000 字符；iOS `RemoteService.request` 却在完整响应超过 2,000,000 字节时失败。用 120 条每条 8,000 个中文字的合成合法快照计算，JSON 为 **2,889,635 UTF-8 字节**，不含大型审批详情或额外 live。这是上限不兼容的确定性证据，尚未作为真实服务端端到端用例执行。

需要整体 UTF-8 字节预算与分页/截断标记，不能仅按每条字符数限长；不要仅把手机接收上限无界调大。消息截断也应与完成/失败标记分离，保证用户知道手机展示不全。

### P2：两套输入体验仍不一致

本机已有长文展开、附件、原位语音波形和选择区管理；远程仍为 1–6 行 TextField、独立录音完成栏，缺少长文展开与附件入口。更改远程首页时应复用已经验证的输入能力，并明确手机附件如何发往电脑；不能只让外观接近而保留功能断层。远程语音目前设计为转写留稿，切勿在无明确产品决定时改成自动执行电脑指令。

### P2：远程项目子页可能长期显示旧列表

`RemoteProjectView` 接收一次传入的 `[RemoteChat]`，未观察 RemoteStore。即使主页刷新到了新增会话，已打开的子页也没有直接的订阅更新路径。需用“项目内新建 → 返回项目列表”和电脑另端新增任务验证；源码表明依赖方式有缺口，当前还未用双端更新实测定论。

### P2：远程草稿没有沿用测试存储隔离

RemoteStore 在 `--ui-testing` 使用 `PotatoRemoteUITests`，Workspace 使用临时测试根；RemoteTaskView 却直接访问 `UserDefaults.standard`，且 `--reset` 不清其草稿/pending。UI 测试之间可能互相污染；专用模拟器也不能把这等同于真实数据完全隔离。修复时应统一注入存储与生命周期，回归中禁止使用个人账号或真实任务。

## 修复与完整验收顺序

1. 先隔离目标身份、草稿、pending 与重试恢复，保留已有内容，防止误发或重复执行。
2. 根据已展示的三张图选择首页方向，再落实侧栏、模型面板、输入与真实过程状态；当前尚未选择，不推断用户偏好。
3. 将上述失败用例转为默认回归；补多设备、首轮会话迁移、409 恢复、断线状态和中文响应预算测试。
4. 在 iPhone 17 与小屏、大字、Reduce Motion 检查实际界面；完成长文、附件、语音、审批/提问、停止、后台恢复与历史回归。
5. 最低 iOS 17、真机手势/麦克风及 VoiceOver 尚缺本轮证据，不能用模拟器截图替代。任何发布操作另按既有授权范围进行。

## 重现命令

从仓库根目录运行。结果目录每次新建，保留失败证据；只运行离线测试，不启用 LiveVerification。

```sh
AUDIT_RUN=$(mktemp -d /tmp/potato-ios-audit.XXXXXX)
TEST_RUNNER_POTATO_IOS_AUDIT=1 xcodebuild \
  -project native/potato-ios/PotatoMobile.xcodeproj -scheme PotatoMobile \
  -destination 'platform=iOS Simulator,name=iPhone 17' \
  -derivedDataPath native/potato-ios/build -parallel-testing-enabled NO \
  -resultBundlePath "$AUDIT_RUN/regressions.xcresult" \
  -only-testing:PotatoMobileTests/RemotePresentationTests/testAccountDraftsAreScopedToTargetComputer \
  -only-testing:PotatoMobileUITests/RemoteUITests/testSidebarOpensWithHorizontalDrag \
  -only-testing:PotatoMobileUITests/RemoteUITests/testSidebarClosesWithHorizontalDrag \
  -only-testing:PotatoMobileUITests/RemoteUITests/testVerticalScrollDoesNotOpenSidebar \
  -only-testing:PotatoMobileUITests/RemoteUITests/testRemoteNavigationSearchAndDrawer \
  -only-testing:PotatoMobileUITests/RemoteUITests/testEmptyRemotePairingValidationAndLocalChatReturn test
```
