# 验证记录

## 最新：改用原生默认玻璃

按用户要求移除自定义玻璃投影遮罩与细轮廓线。此次 GlassChromeTests 5 / 5 与 Release 模拟器构建通过，见 [最新截图与验证记录](native-default/README.md)。下方第二轮记录对应切换前版本。

## 第二轮材质打磨 · 最终代码

2026-09-16，Xcode 26.3 / iOS 26.2 SDK / iOS 26.3 模拟器。下列结果对应本轮最后的原生玻璃、羽化投影、状态栏底层和紧凑输入区实现。

| 验证 | 结果 | 原始记录 |
| --- | --- | --- |
| iPhone 17 | 11 / 11 通过 | `/tmp/potato-glass-polish-final.xcresult`、`/tmp/potato-glass-polish-final.log` |
| iPhone SE | 5 / 5 通过 | `/tmp/potato-glass-polish-se.xcresult`、`/tmp/potato-glass-polish-se.log` |
| Release 模拟器构建 | BUILD SUCCEEDED | `/tmp/potato-glass-polish-release.log` |

iPhone 17 覆盖 GlassChromeTests 全部 5 项、LongInputTests 全部 2 项、远程草稿隔离及旧草稿确认 2 项、全屏侧栏手势 1 项、语音直接发送 1 项。SE 使用同一 Debug 构建执行 GlassChromeTests 全部 5 项。SE 与主回归存在重复案例，不把设备间重复执行计为新增独立测试。

目视复查：阅读中的正文与气泡、顶部导航、输入区、欢迎页、侧栏圆角、键盘和 SE 最大辅助字号。截图已更新为本轮最后代码；此前截图保留为 `before-polish-*`，`polish-scroll-edge-comparison.png` 是本轮中间步骤，仅用于相同旧样例的滚动边缘比较。

阅读测试改用专用的 DEBUG 合成对话，覆盖长段落、粗体、列表与用户气泡，断言回到末尾后最后一段内容和操作按钮可见。长历史自动追底仍由既有 SidebarUITests 覆盖，其第一轮记录保留在下方。本轮没有重新运行全部第一轮 23 项。

未上传 TestFlight，未进行 iOS 27 真机验收。系统玻璃在不同系统版本、辅助功能设置和设备上的外观可能不同；此处截图是 iOS 26.3 模拟器的实际结果。

## 第一轮验证记录（历史）

2026-09-16，Xcode 26.3 / SDK 26.2 / 模拟器 iOS 26.3。

23 项主回归通过。之后对侧栏最大字号和整行触控范围做了局部修正，追加 SE 定向回归及 iPhone 17 阅读/侧栏完整路径验证，均通过。各轮存在重复案例，不把重复执行次数当成独立测试数量。

最终 Release 模拟器构建成功，日志 `/tmp/potato-glass-release-verified.log`，构建目录 `/tmp/potato-glass-release`。此次未进行 Apple 上传或 TestFlight 分发。

新增回归覆盖：侧栏关闭后新建/输入响应，页面关闭遮罩覆盖全屏，当前会话选中，阅读追底与末条操作不被遮挡，键盘上方发送按钮，大字号侧栏按钮尺寸，减少透明度的材质回退。同步修正旧 SidebarUITests 中与既有产品标题不符的“新远程任务”选择器为“新对话”，实际返回手势断言保留。

视觉检查发现的 SE 最大字号按钮膨胀已修正，截图已复查。iOS 27 真机、iOS 17 回退运行时、真实麦克风网络链路与完整 VoiceOver 人工验收不在本轮证据范围；语音和远程测试使用隔离测试数据。

## iPhone 17 主回归

结果：23 / 23 通过。原始日志：`/tmp/potato-glass-final.log`。

- `PotatoMobileUITests.GlassChromeTests testControlsRespondAfterClosingSidebar`：通过
- `PotatoMobileUITests.GlassChromeTests testKeyboardAndNewChatKeepControlsReachable`：通过
- `PotatoMobileUITests.GlassChromeTests testLargeTextKeepsComposerAndDrawerUsable`：通过
- `PotatoMobileUITests.GlassChromeTests testReadingSidebarAndLatestMessageRemainAccessible`：通过
- `PotatoMobileUITests.GlassChromeTests testReducedTransparencyKeepsControlsReadable`：通过
- `PotatoMobileUITests.LongInputTests testInputShrinksAfterDeletingLines`：通过
- `PotatoMobileUITests.LongInputTests testLongInputAudit`：通过
- `PotatoMobileUITests.PrototypeTests testLargeTextLayout`：通过
- `PotatoMobileUITests.PrototypeTests testNewChatKeyboardStreamingStopAndHistory`：通过
- `PotatoMobileUITests.RemoteUITests testAccountDeviceDraftsStaySeparateAcrossNavigationAndRelaunch`：通过
- `PotatoMobileUITests.RemoteUITests testLegacyDraftRequiresExplicitTargetConfirmation`：通过
- `PotatoMobileUITests.SidebarUITests testCodeAndTableCanPanWithoutOpeningDrawer`：通过
- `PotatoMobileUITests.SidebarUITests testConversationScrollStillPausesAndResumesFollowing`：通过
- `PotatoMobileUITests.SidebarUITests testDevicePickerAndComposerKeepTheirHorizontalGestures`：通过
- `PotatoMobileUITests.SidebarUITests testFullSurfaceDragOpensAndBackdropCloses`：通过
- `PotatoMobileUITests.SidebarUITests testKeyboardDismissalPreservesLocalDraft`：通过
- `PotatoMobileUITests.SidebarUITests testNestedRemotePagesKeepNativeBackGesture`：通过
- `PotatoMobileUITests.SidebarUITests testPresentedDeviceSheetDoesNotMoveWorkspace`：通过
- `PotatoMobileUITests.SidebarUITests testReduceMotionAndLargeTextStillAllowOpenAndClose`：通过
- `PotatoMobileUITests.SidebarUITests testShortSlowDragsReturnToTheirStartingState`：通过
- `PotatoMobileUITests.SidebarUITests testSidebarSearchKeyboardDismissesOnClose`：通过
- `PotatoMobileUITests.SidebarUITests testVerticalScrollActuallyMovesContentWithoutOpeningDrawer`：通过
- `PotatoMobileUITests.VoiceInteractionTests testDirectSpeechSend`：通过

## iPhone SE 五项布局回归

结果：5 / 5 通过。原始日志：`/tmp/potato-glass-se.log`。

- `PotatoMobileUITests.GlassChromeTests testControlsRespondAfterClosingSidebar`：通过
- `PotatoMobileUITests.GlassChromeTests testKeyboardAndNewChatKeepControlsReachable`：通过
- `PotatoMobileUITests.GlassChromeTests testLargeTextKeepsComposerAndDrawerUsable`：通过
- `PotatoMobileUITests.GlassChromeTests testReadingSidebarAndLatestMessageRemainAccessible`：通过
- `PotatoMobileUITests.GlassChromeTests testReducedTransparencyKeepsControlsReadable`：通过

## SE 最后字号修正

结果：2 / 2 通过。原始日志：`/tmp/potato-glass-se-final.log`。

- `PotatoMobileUITests.GlassChromeTests testLargeTextKeepsComposerAndDrawerUsable`：通过
- `PotatoMobileUITests.GlassChromeTests testReadingSidebarAndLatestMessageRemainAccessible`：通过

## SE 最后触控范围修正

结果：2 / 2 通过。原始日志：`/tmp/potato-glass-touch-final.log`。

- `PotatoMobileUITests.GlassChromeTests testControlsRespondAfterClosingSidebar`：通过
- `PotatoMobileUITests.GlassChromeTests testLargeTextKeepsComposerAndDrawerUsable`：通过

## iPhone 17 最终代码阅读/侧栏

结果：1 / 1 通过。原始日志：`/tmp/potato-glass-visual-final.log`。

- `PotatoMobileUITests.GlassChromeTests testReadingSidebarAndLatestMessageRemainAccessible`：通过
