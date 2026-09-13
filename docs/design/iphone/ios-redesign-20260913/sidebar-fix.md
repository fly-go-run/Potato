# iOS 侧栏手势修复 · 2026-09-13

本轮只实现独立的手势与滚动修复，首页三张概念图仍待选择。没有发布新版本，也没有完成模型/思考设置、回复过程动画或全部 iOS 审查。

## 行为与实现

- 根页面右拖打开侧栏、左拖关闭；画面跟随触点移动，短拖返回原状态，距离与有限速度投影共同决定是否完成。
- 一次触摸只判断一次方向，纵向滚动不会半途变成抽屉拖动。横拖成立后取消底下的点击，避免侧栏打开同时进入项目。
- 输入框、语音区域、设备横向筛选、代码和表格保留自己的手势；弹出的设备管理页不能带动背后的工作区。
- 远程子页面停用抽屉识别，保留系统返回。聊天区原先用于暂停追底的 SwiftUI `DragGesture` 会阻挡边缘返回，现改为观察已有 `UIScrollView` 的纵向拖动，不安装第二个竞争识别器。
- 打开/关闭时收起键盘并保留草稿；侧栏搜索词也保留。保留显式开关按钮与无障碍关闭动作；减弱动态效果时取消松手后的补间动画，直接拖动仍跟手。
- `SidebarGesture.swift` 使用公开 UIKit API；弱引用记录实际排除区域，卸载时移除识别器与滚动观察。进入后台、窗口宽度变化或离开远程根页时取消未完成的拖动。

本地“回到最新消息”按钮原先被外层 `conversation` 无障碍标识覆盖。已把该标识移至实际滚动容器，使按钮恢复独立标识；截图与事件诊断确认暂停追底本身正常，未通过弱化断言掩盖失败。

## 验证证据

Xcode 26.3，iOS 26.3.1（23D8133）模拟器：iPhone 17 **81 项通过**（全部 65 项单元测试及 16 条相关页面回归）；iPhone SE 第三代 **18 项通过**（6 项方向/阈值单元测试及 12 条页面回归）。两次均 **0 失败、0 跳过**，不是 99 个独立用例。测试只使用独立沙盒与合成设备/消息，不调用真实模型或向真实电脑执行任务。

页面回归覆盖根页完整右拖、左拖、短拖取消、点击收回、真实纵向滚动、代码/表格横移、设备筛选与输入编辑、草稿与搜索词保留、两级系统返回、弹窗隔离、大字/减弱动态效果、暂停与恢复追底、语音取消和发送手势。

- [iPhone 17 结果摘要](../../../../native/potato-ios/qa/ios-audit-20260913/sidebar-fix/iphone17-summary.json)、[原始日志及完整命令](../../../../native/potato-ios/qa/ios-audit-20260913/sidebar-fix/iphone17-tests.log)、[截图索引](../../../../native/potato-ios/qa/ios-audit-20260913/sidebar-fix/iphone17-attachments/manifest.json)。最终原始结果包：`/tmp/potato-sidebar-final-10.xcresult`。
- [iPhone SE 结果摘要](../../../../native/potato-ios/qa/ios-audit-20260913/sidebar-fix/iphonese-summary.json)、[原始日志及完整命令](../../../../native/potato-ios/qa/ios-audit-20260913/sidebar-fix/iphonese-tests.log)、[截图索引](../../../../native/potato-ios/qa/ios-audit-20260913/sidebar-fix/iphonese-attachments/manifest.json)。原始结果包：`/tmp/potato-sidebar-se-11.xcresult`。
- [验证源码 SHA-256](../../../../native/potato-ios/qa/ios-audit-20260913/sidebar-fix/source-hashes.json)。本轮未增加线上接口、模型调用或自动发送路径。

已打开核对最终构建的普通侧栏、XXXL 字号侧栏和代码/表格截图。以下是实际测试截图，不是设计概念图：

![iPhone 17 侧栏展开](../../../../native/potato-ios/qa/ios-audit-20260913/sidebar-fix/iphone17-attachments/E7C2ED46-2F46-4CF5-AFDA-FBDD845F69BB.png)

SE 的普通侧栏、XXXL 侧栏与收起键盘后草稿截图也已打开核对。XXXL 下“资料库”“对话”换行且布局拥挤，保留为后续大字号设计待办；这次只证明关键触点与手势可用。

![iPhone SE 侧栏展开](../../../../native/potato-ios/qa/ios-audit-20260913/sidebar-fix/iphonese-attachments/17A6E9EA-DAE7-40D6-98E7-53D7B8F42EAB.png)

[跟手打开及点击收回录屏](../../../../native/potato-ios/qa/ios-audit-20260913/sidebar-fix/open-and-backdrop.mp4) 与下列相隔约 0.2 秒的中间帧，来自 `/tmp/potato-sidebar-final-7.xcresult` 同期模拟器录屏。这轮的手势用例通过，但追底按钮标识测试失败；最终构建随后修正标识，不能把该轮称为全绿。视频为实际测试画面，无合成动画。

![连续拖动中间帧](../../../../native/potato-ios/qa/ios-audit-20260913/sidebar-fix/opening-frames.png)

失败证据保留在 `native/potato-ios/qa/ios-audit-20260913/sidebar-fix/`：`swiftui-conflicts.log` 记录早期实现的误触与排除区域问题；`native-back-failure.log` 记录返回手势冲突；`scroll-observer-failure.log` 记录按钮标识导致的测试失败。曾尝试显式显示子页导航栏，测试仍失败，已撤回该尝试；实际截图表明返回按钮原本就存在。

## 验证边界

模拟器回归不能替代真机的手感、麦克风和完整 VoiceOver 验收。最低支持版本 iOS 17 仍缺本轮运行证据。大字号测试只验证侧栏开关与关键触点，不能据此声称整个应用的大字布局已合格。实时生成、断线恢复、模型参数与思考状态仍按主审查清单继续。
