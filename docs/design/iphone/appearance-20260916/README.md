# iPhone 外观选择

2026-09-16，已发布至 TestFlight **0.2.2（2026091606）**，既有个人内测组 Testing，中文更新说明读回一致。见 [发布记录](release-2026091606/README.md)。

设置 → 外观提供「亮色 / 暗色 / 自动」。切换可预览，点击保存后保存在本机；取消或下拉关闭恢复此前已保存的选择。自动跟随系统，旧版 workspace 没有外观字段时默认自动，聊天与附件数据不迁移或清空。

外观应用到当前 UIWindow，覆盖 SwiftUI 弹层和 UIKit 输入控件；自动模式清除窗口外观覆盖。移除 App 与 Info.plist 的固定亮色设置。共享颜色支持明暗适配，覆盖聊天、文稿、资料库、远程、模型选择与回复操作；按钮前景与背景成对切换，图片及 PDF 原始内容保持原样。

## 验证

- iOS 26.3 原生模拟器，完整 157 项单元测试通过，包括旧设置解码、三种模式持久化和主要文字/按钮颜色对比度检查。
- 系统暗色下 4 项 UI 回归通过：外观预览/保存/取消/重启、连接设置、资料库预览/复用/重启、远程导航。
- 系统亮色下 2 项 UI 回归通过：外观选择持久化/取消，以及连接设置滚动/取消。已逐张核对 [自动跟随系统亮色](light-system/appearance-04-automatic-settings.png) 与 [主页面](light-system/appearance-05-automatic-workspace.png)，从暗色切回自动时弹层正确同步。测试模拟器恢复原来的系统亮色。
- 已检查 [暗色设置](dark-system/appearance-02-dark-settings.png)、[自动跟随系统暗色](dark-system/appearance-04-automatic-settings.png)、[系统暗色时强制亮色](dark-system/appearance-01-light-settings.png)、[资料库](dark-system/library-01-grouped.png)、[输入区](dark-system/library-05-composer.png)、[远程](dark-system/remote-home.png)。
- 真机及 iOS 17 尚未人工验收；本轮仅发布 iPhone 客户端，未部署 Worker。

## 证据

最终暗色回归日志：`/tmp/potato-appearance-final-dark-tests.log`；结果：`native/potato-ios/build/Logs/Test/Test-PotatoMobile-2026.09.16_22-00-22-+0800.xcresult`。

亮色回归日志：`/tmp/potato-appearance-final-light-tests.log`；结果：`native/potato-ios/build/Logs/Test/Test-PotatoMobile-2026.09.16_22-02-50-+0800.xcresult`。源码及测试输入见 [source-sha256.json](source-sha256.json)。
