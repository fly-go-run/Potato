# 精简加载提示

2026-09-17，原生 iPhone 客户端工作树修改，尚未发布。

- 远程目录移除“正在更新”“正在连接电脑”及为这些文字预留的空行；缓存列表保持显示，设备状态灯独立更新。
- 首次读取目录保留骨架，移除骨架上方的加载文字。离线和连接失败提示保留。
- 普通对话等待回复、整理搜索结果时只显示小动画；正文输出时移除额外的“正在回复”一行。
- 远程对话初次加载与正常执行时移除重复状态小字，保留动画和 VoiceOver 状态。断线、状态过期、等待批准／回答及失败／停止提示保留。过程详情入口不变。

## 验证

iOS 26.3 独立模拟器 PotatoDirectoryVerification，使用本地合成数据，无线上账号或真实电脑请求。

- 4 条目录 UI 回归通过：首次加载、缓存重启无跳位、离线／错误保留列表、确认空目录。
- 1 条普通对话 UI 回归通过：等待、思考、回复、重开后历史保留。
- 2 条远程过程 UI 回归通过：思考／工具／正文／完成，以及断线保留草稿和重连。
- 已检查目录、普通对话等待、远程正文截图；等待的小动画保留，重复文字移除。

结果：`native/potato-ios/build/Logs/Test/Test-PotatoMobile-2026.09.17_07-48-04-+0800.xcresult` 中 5 条通过，2 条远程测试因 runner 环境变量未传入而跳过；通过临时 xctestrun 注入测试开关重跑，两条均通过，结果在 `/tmp/potato-quiet-process.xcresult`。

截图：

- [重开缓存列表](03-relaunch-cached-connecting.png)
- [连接成功](04-relaunch-connected.png)
- [普通对话等待](quiet-reply-waiting.png)
- [远程正文输出](remote-body-active.png)
- [远程断线保留草稿](remote-offline-draft-keyboard.png)
