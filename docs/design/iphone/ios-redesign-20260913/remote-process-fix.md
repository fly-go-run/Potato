# iOS 远程状态与过程反馈 · 2026-09-13

本轮修复 SwiftUI 远程任务页把旧快照当作持续执行证据的问题。修改尚未发布，core 和中继协议没有变化。

## 行为

- 当前状态集中在输入区上方：准备、思考、工具执行、正文回复、等待批准/回答、完成、停止与失败，均从实际快照推导。过程组使用稳定 ID、默认折叠，不再各自重复执行动画。
- 连接失败立即停止执行动画，保留内容与草稿，显示“连接中断，任务状态未确认”和上次确认状态。确认过期时停用新发送、停止、审批和回答；同编号的待确认发送仍可由用户重试查询结果。
- 每两秒读取任务；快照有效期为八秒，从请求开始时间计量。即使网络请求悬挂，也会独立转入“正在确认任务状态”。耗时超过有效期才返回的响应不会重新启用旧状态。
- 进入后台/离开页面停止更新；返回前台须重新确认，不能直接恢复旧执行动画。此举不会向电脑发送停止指令。减弱动态效果使用静态活动图标。
- 任务读取与模型目录读取独立进行，慢模型列表不再挡住首屏内容。仅在确认有效性改变时刷新过期提示，避免为时钟每秒重建长 Markdown。
- 展开过程会暂停自动追到最新消息，后续快照保留展开状态。尚无思考文字时明确显示“尚未收到思考文字”，不生成假内容。
- 辅助状态、设备说明及操作按钮的字号上限为系统 XXXL；正文、过程详情和输入文字保留用户的辅助大字号。状态允许换行，网络详情放入可滚动内容区。

## 验证与证据

使用本机回环端口 19014 的合成远程服务，实际经过 URLSession；它不执行模型或电脑命令。测试服务提供受控状态、断线、延迟响应与审批回执。测试发送的文字仅为合成草稿。iPhone 17 联合回归 **110 项通过、0 失败、0 跳过**，包括 95 项单元检查、6 条远程状态流程、5 条远程模型流程、2 条回执草稿流程、1 条原生返回和 1 条本机思考流程。iPhone SE 六条远程状态流程全部通过。见 [17 摘要](../../../../native/potato-ios/qa/ios-audit-20260913/remote-process-fix/iphone17-summary.json)、[SE 摘要](../../../../native/potato-ios/qa/ios-audit-20260913/remote-process-fix/iphone-se-summary.json)、[17 日志](../../../../native/potato-ios/qa/ios-audit-20260913/remote-process-fix/iphone17-tests.log)。

初次截图揭示最大辅助字号把输入框挤到键盘后面：原来的 isHittable 断言只能证明部分可点击，不能证明完整可见。修复后增加输入框底边不越过键盘顶边、完整位于屏幕内的断言。另将动画数量断言限定为屏幕内可见元素，排除原生下拉刷新的屏外指示器。

随后为禁用发送按钮增加灰色外观，iPhone 17 与 SE 的大字离线流程各补测一次并通过。见 [17 最终外观](../../../../native/potato-ios/qa/ios-audit-20260913/remote-process-fix/iphone17-chrome-summary.json)、[SE 最终外观](../../../../native/potato-ios/qa/ios-audit-20260913/remote-process-fix/iphone-se-chrome-summary.json)。110 项联合回归和 SE 六条流程采用增加灰色外观前的代码；补测采用最终外观，禁用条件保持相同。

最终另录制一条真实页面流程，测试通过；[录屏](../../../../native/potato-ios/qa/ios-audit-20260913/remote-process-fix/remote-process.mp4)与[逐秒预览](../../../../native/potato-ios/qa/ios-audit-20260913/remote-process-fix/video-contact-sheet.png)已检查，包含思考、工具、正文、完成后没有执行动画的画面。录屏检查启用 `POTATO_IOS_PROCESS_VIDEO=1`，在完成后停留四秒，避免 XCTest 立即关闭应用；这仅影响测试停留时间。[录屏测试摘要](../../../../native/potato-ios/qa/ios-audit-20260913/remote-process-fix/video-summary.json)。初次录屏不作为最终完成状态的证据。

Release 模拟器构建（arm64/x86_64）通过，产物不含本轮合成服务地址或启动开关。见 [构建日志](../../../../native/potato-ios/qa/ios-audit-20260913/remote-process-fix/release-build.log)、[产物检查](../../../../native/potato-ios/qa/ios-audit-20260913/remote-process-fix/release-check.json)、[源码摘要](../../../../native/potato-ios/qa/ios-audit-20260913/remote-process-fix/source-sha256.json)。Xcode 26.3、iOS 26.3.1（23D8133）；没有部署 Worker、更新 TestFlight 或改变生产凭据。

![SE 最终大字离线键盘](../../../../native/potato-ios/qa/ios-audit-20260913/remote-process-fix/iphone-se-chrome-attachments/00408AE5-46C6-44D7-A24E-E8FE343D6708.png)

## 边界与后续

轮询提供最近确认的状态，不等于连续实时连接证明；设备上真实任务仍可能在两次快照间改变。没有新增虚构的思考计时。最低 iOS 17、完整 VoiceOver 人工检查与线上真机联调仍待完成。

源码还显示停止请求只携带会话编号，未像运行中补充指令一样绑定具体运行编号。打开停止确认到实际操作之间若运行被替换，存在停止另一轮任务的风险；本轮尚未故障注入验证，应随后补充精确绑定。

409 已接收但未写入最终回执的问题仍需独立恢复协议，不能通过清空 pending 或自动换编号重发解决。首页视觉方向与其余完整验收仍见 [总矩阵](README.md)。
