# iPhone Remote 原生视觉验收

日期：2026-09-12。范围：用户选择的 ChatGPT 式侧栏与 Remote 信息架构，及新增原生远程任务流程。实际客户端为 SwiftUI PotatoMobile，iPhone 17 模拟器，iOS 26.3，402 × 874 pt、3×截图。不是浏览器原型截图。

## 参照与实际结果

用户的 [Remote 参照](references/chatgpt-user-remote.jpg) 和 [侧栏参照](references/chatgpt-user-sidebar.jpg) 与当前模拟器截图并列查看后评估。

| 页面 | 实际截图 | 结果 |
| --- | --- | --- |
| 远程首页 | [首页](implemented/remote-home.png) | 顶部居中标题、两侧圆形按钮、水平设备胶囊、置顶/项目/会话分组、底部搜索/语音/新建符合参照层级。设备与会话为明确的 DEBUG 视觉样例。 |
| 抽屉导航 | [侧栏](implemented/remote-sidebar.png) | 侧栏约屏宽 74%，主内容右移并圆角；浅灰选中行、最近对话、底部对话按钮与设置。只呈现 Potato 已有功能。 |
| 搜索 | [搜索状态](implemented/remote-search.png) | 搜索过滤项目/会话，清除后恢复列表；按钮具有标签，真实 UI 测试完成操作。 |
| 未关联设备 | [登录空态](implemented/remote-empty.png) | 说明同账号关联与开启访问，登录为主要操作，配对为次要操作；按钮文字清楚。 |
| 实际连接 | [本地联调电脑](implemented/remote-real-connected.png) | 来自真实本地中继与 Rust 核心的设备/项目目录，不是远程页面样例。 |
| 执行与结果 | [首次响应](implemented/remote-real-response.png)、[批准后](implemented/remote-real-approved.png) | 原生核心返回的合成模型结果；可继续同一会话，工具详情折叠并可展开。 |
| 人工批准 | [审批卡片](implemented/remote-real-approval.png) | 原因、执行命令、工作目录与沙箱外执行属性可见，完整详情可展开；拒绝与允许一次可区分。 |
| 停止 | [停止结果](implemented/remote-real-stopped.png) | 用户确认停止后，运行按钮消失，最新消息区域显示“本轮任务已停止”。 |

## 本轮修复与复核

| 级别 | 发现 | 修正与证据 |
| --- | --- | --- |
| P1 | 侧栏滑入动画期间的命中位置不稳定，搜索与导航回归失败 | 保持侧栏在后方挂载，只平移主内容；三条导航/本机聊天回归通过，最新远程导航复测通过。 |
| P1 | 空态黑色登录按钮因样式继承而文字不可辨 | 明确白色文字与黑色胶囊背景；最新空态截图复核通过。 |
| P1 | 审批完整 JSON 占满视口，关键动作难以找到 | 默认展示操作摘要/命令/目录，完整证据折叠；真实批准一次的端到端测试通过。 |
| P2 | 长会话没有自动跟随，停止提示在可见区域外 | 增加最新内容跟随；手动阅读时停止跟随，提供“回到最新消息”；最新截图和停止状态断言通过。 |
| P2 | 已完成的历史工具调用仍显示“正在执行” | 历史行采用稳定的“电脑执行步骤”，运行状态在任务级单独显示；最新停止截图没有误导性运行标签。 |

当前上述验收范围内无未解决的 P0/P1/P2；这是限定视口和已测路径的结论，不代表全量发布验收。

参照差异：品牌为 Potato，暖白色与系统字体沿用原生应用；文案为中文；多电脑“全部”视图补充设备归属；没有添加不存在的图片、插件或定时任务入口。聊天正文、真实设备名称、工作目录和审批理由由实际内容决定，不为截图伪造数据。

## 自动化证据

- `/tmp/potato-remote-e2e-ui-v4.xcresult`：最新三条 RemoteUITests 全部通过，0 failure。其完整联调测试经实际 Worker/SQLite DO/WebSocket/Rust 核心，模型侧为合成服务；覆盖配对、两轮对话、提问、实际一次性批准、停止。
- `/tmp/potato-remote-ui-v4.xcresult`：此前两条远程 UI 回归与本机聊天/停止/历史搜索回归通过。此处的 `ui-v4` 与上面的 `e2e-ui-v4` 为不同运行。
- `/tmp/potato-remote-worker-final.log`：26 项 Worker 测试、TypeScript 检查通过。
- `/tmp/potato-remote-core-final.log`：6 项原生远程测试通过。
- `/tmp/potato-remote-ios-final-build.log`：最后补充过期账号本地退出处理后的 iOS 构建通过；没有改变已保存的界面布局。
- `/tmp/potato-remote-gpui-final.log`：GPUI 编译检查通过。
- `/tmp/potato-remote-worker-dry-run.log`：部署干跑通过，没有发布。
- [持久验证记录](verification.json) 保存本轮涉及文件摘要和命令/结果；实际图像已进入本目录，不依赖临时 Xcode 目录。

## 尚未覆盖

蜂窝网络、真机签名、休眠唤醒、iOS 17、新远程流程的大字号/窄屏、完整 VoiceOver 人工验收和远程语音真机录音未做。GPUI 本轮只验证编译及共享核心协议，没有用 SwiftUI 截图替代桌面视觉证据。中继是受信任方，尚无端到端加密。详见 [实现与边界](README.md)。

## 线上验收追加（2026-09-12）

真实 Cloudflare 浏览器登录及 iPhone 模拟器经线上中继的完整任务流程已通过，退出清理测试也通过。修复真实表单 Origin 拒绝和键盘变化时的懒布局循环；最终运行记录与边界见 [发布记录](release.md)。本轮是临时 Rust 核心和合成模型，未替代 GPUI 桌面视觉或 iPhone 真机验收。线上停止状态截图见 [remote-real-stopped.png](implemented/live/remote-real-stopped.png)。
