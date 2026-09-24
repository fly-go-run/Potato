# iPhone 远程语音输入统一

2026-09-17，工作树修改，尚未发布 TestFlight。

用户截图中的远程「新对话」仍使用独立的“取消／正在听／完成”工具栏，忽略录音音量事件，与普通对话的原位语音面板不同。

## 修改

- `RemoteTaskView` 复用 `VoiceComposer` 和 `VoiceComposerPanel`，移除旧 `RemoteDictation`。普通对话继续使用同一状态机。
- 录音时显示原位转写、实时音量波形、叉号取消与勾号发送；波形左滑取消、上滑发送，点文字结束录音后打开键盘。欢迎文案和普通输入工具在录音时隐藏。
- 转写沿 UTF-16 光标范围插入；取消恢复录音前草稿。长转写可滚动、回到最新及展开编辑。
- 仅在主动发送且收到非空最终转写后调用远程发送入口。先保存草稿，再检查当前远程发送条件；使用原有电脑归属、操作编号、模型选择和发送回执流程。
- 中断、后台、失败、空最终结果和 60 秒上限都不自动发送。迟到结果不能覆盖手工编辑或再次触发发送。

## 验证

iOS 26.3 模拟器 PotatoParityiOS：

- 首轮全量 167 项 iOS 单元测试通过。
- 新增远程草稿回调与异常路径测试后，11 项语音状态测试通过。
- 12 条普通／远程语音 UI 测试全部通过，包含首页语音入口、取消及编辑、大字号、后台保留、长文展开和远程发送。
- 本机 loopback 远程服务只接收合成输入；日志确认完整转写的 `send` 请求恰好 1 次。未调用真实电脑或模型；测试服务已停止。
- 另补跑 1 条亮色首页入口 UI 测试通过。亮色、暗色与大字号截图已检查，见本目录 PNG。测试转写及音量由 DEBUG fixture 提供；未重新验证真机麦克风或线上豆包链路。

测试结果：

- `native/potato-ios/build/Logs/Test/Test-PotatoMobile-2026.09.17_07-01-55-+0800.xcresult`
- `native/potato-ios/build/Logs/Test/Test-PotatoMobile-2026.09.17_07-07-23-+0800.xcresult`
- `native/potato-ios/build/Logs/Test/Test-PotatoMobile-2026.09.17_07-11-30-+0800.xcresult`

长文展开测试记录了 UIKitToolbar 视图层级警告，普通对话原有展开测试也出现同一警告；两条流程断言均通过。

本次只修改用户截图对应的原生 SwiftUI iPhone 客户端及测试；桌面 GPUI、potato-core 和 Worker 未修改。
