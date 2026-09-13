# iPhone 远程控制：调研、设计与实施

日期：2026-09-12。用户要求把手机远程控制桌面 Potato 当作目标实现，并调研其他产品的真实界面后设计本产品。

## 已查阅的参照

| 产品 | 来源与图片 | 对 Potato 的启发 |
| --- | --- | --- |
| ChatGPT Remote | 用户提供的 [远程页](references/chatgpt-user-remote.jpg)、[侧栏](references/chatgpt-user-sidebar.jpg)；[官方远程连接文档](https://learn.chatgpt.com/docs/remote-connections) | 电脑筛选、项目、置顶与会话共存；手机继续同一宿主任务；文件、工具与权限由宿主提供。 |
| Claude Code | [官方 Remote Control 文档](https://code.claude.com/docs/en/remote-control)；[ClaudeDevs 发布的设备图](https://x.com/ClaudeDevs/status/2090933157243863142)，[下载图](references/claude-official-devices.jpg) | 设备在线状态独立于任务状态；恢复连接时继续同步，避免手机断开就取消电脑任务。图片由图片搜索定位并直接下载，发布页正文未成功抓取；行为依据官方文档。 |
| Happy | [官网](https://happy.engineering/)与[官方手机截图](references/happy-official.png) | 同一会话内呈现工具步骤与文件 diff；让手机用户有足够证据决定下一步。官网说明会话端到端加密，这也是 Potato 后续传输审查的参照。 |
| Jump Desktop | [官方 iOS 页面](https://jumpdesktop.com/download-web-ios.html)、[官方设置说明](https://support.jumpdesktop.com/hc/en-us/articles/216424003-Install-Jump-Desktop-on-your-iPad-iPhone-Mac-or-Windows-device)、[官网图片](references/jump-official-ios.png) | 设备连接入口明确；其桌面画面与指针操作适合直接遥控。Potato 当前优先提供任务式控制，现有 Cua 驱动尚未开放截图，因此不把屏幕直播当作已实现能力。 |

以上是公开文档与图片调研，没有登录这些竞品的真实账户完成端到端体验验收。

## 最终界面与功能

用户明确选择「界面的话，还是按 chatgpt 的来做吧」。最终采用原始截图中的抽屉侧栏、顶部电脑筛选、置顶/项目/会话平面列表、底部搜索/语音/新建。保留 Potato 名称、现有暖白配色与 SwiftUI 系统字体；侧栏只展示已有可用功能。此前 [设计 1](design-1.png)、[设计 2](design-2.png)、[设计 3](design-3.png) 与 [生成记录](generation-prompts.json) 保留为探索，不是选定方案。

登录流程：电脑与 iPhone 登录同一 Cloudflare 身份 → 电脑明确开启远程访问 → 手机自动发现电脑 → 选择项目或已有会话 → 发起/继续任务、查看进度、审批、回答提问、停止。手机离开页面后任务继续在电脑运行，回来后重新读取状态。账号退出和单电脑撤销分别生效。既有一次性配对作为备用路径保留。

这是任务式远程控制：手机调用电脑上的原生 Potato 运行时及其工具、项目和权限。未实现屏幕直播、鼠标触控板或远程桌面画面。

## 实现位置

| 层 | 实现 |
| --- | --- |
| Rust 核心 | `native/potato-core/src/remote.rs`：主动出站 WebSocket、重连/心跳、账号与备用配对、会话读取、持久操作收据、发送/停止/审批/提问/置顶。复用当前任务与权限链；终态落盘供断线后读取。 |
| GPUI 桌面 | `settings.rs` 提供登录、核对码、显式开启、状态与退出；`backend.rs` 启动桥接；后台变化通知刷新当前桌面会话。 |
| Cloudflare | `native/potato-worker/src/remote.ts`、`remote-auth.ts`：设备/账号/登录 Durable Object，验证 Access JWT、账号设备目录、角色隔离、撤销与受限命令中继。保留原有聊天、搜索、语音和计算接口。 |
| SwiftUI 手机 | `WorkspaceView.swift`、`LibraryView.swift` 增加侧栏；`RemoteService.swift`、`RemoteView.swift` 实现设备筛选、登录/配对、项目与会话、远程任务、审批和提问。凭据进入 Keychain；发送前保存不可变操作编号及参数，未知结果按相同编号重试。 |

具体登录机制、官方依据与上线配置见 [Cloudflare 账号关联](cloudflare-login.md)。实际模拟器图片与修复记录见 [本轮视觉验收](design-qa.md)。

## 验证与复现

2026-09-12 的验证覆盖原生 SwiftUI → 本地真实 Worker/SQLite Durable Object/WebSocket → 原生 Rust 核心。模型响应使用合成 HTTP 服务；审批测试仅执行固定 `echo POTATO_APPROVED`，工作区是新建临时目录。没有把旧 React/Tauri/Python 测试当成本轮证据。

- Worker 27 项测试与 TypeScript 检查通过，含 8 项远程协议/身份测试：配对竞争与响应丢失、重连、权限旋转、身份隔离、JWT/Origin/CSRF、撤销重放、注册/退出并发。
- Rust 核心 6 项定向测试通过：同一运行中的任务继续与去重/重启、停止与置顶持久化、审批/提问会话隔离、实际审批详情与回答、账号登录及密封凭据/显式开启、过期账号退出后的重新登录恢复。
- GPUI `cargo check --locked` 通过；桌面设置界面未进行完整人工视觉验收。
- iPhone 17 模拟器覆盖侧栏/设备页搜索、空态/无效配对/返回本机聊天，以及真实本地中继的连续对话、提问、一次性批准和停止。具体运行记录与截图见视觉验收文档。

仓库根目录执行：

```sh
cargo +1.96.1 test --locked --manifest-path native/potato-core/Cargo.toml remote_tests --lib
cargo +1.96.1 check --locked --manifest-path native/potato-gpui/Cargo.toml
cargo +1.96.1 build --locked --manifest-path native/potato-core/Cargo.toml --example remote_fixture
```

Worker 目录执行 `npm test`、`npm run check`、`npm run build`（最后一个仅部署干跑）。如需复现完整本地联调，在该目录运行 `node test/remote-e2e.mjs /tmp/potato-remote-test`。启动后目录中生成 `pairing.json`，将其中 `pairing_code` 通过环境变量 `TEST_RUNNER_POTATO_REMOTE_PAIRING_CODE` 传给 Xcode，并运行 `PotatoMobileUITests/RemoteUITests/testNativeRemoteRoundTrip`。该测试默认跳过，不在普通测试中访问真实电脑。辅助进程最长 15 分钟自动退出，也可以向 `fixture.json` 中的本次 PID 发送 SIGTERM 清理。

## 外网与真机边界

2026-09-12 已部署 Worker 与 Access 登录，默认远程域名为 `https://potato-remote.recodex.top`，两端真实账号登录已通过。部署版本、外网全流程结果及安装包见 [发布记录](release.md)。验收使用临时电脑工作区，尚未开启用户日常桌面工作区的远程访问。

传输使用 HTTPS/WSS，当前 Cloudflare 中继是受信任方，可以接触命令与响应明文；没有实现手机与电脑之间的端到端加密。中继不持久保存会话正文，设备身份、会话哈希和撤销信息存于 Durable Object。原生应用会话最长三十天，Access 策略变化不等于即时撤销已有 Potato 会话，可在应用中主动退出或撤销电脑。

当前远程显示最近 120 条消息，单条最多 8,000 字符，并显示截断提示；更早/更长内容需在电脑查看。尚未做分页、推送通知、真实 iPhone 签名/TestFlight、蜂窝网络/休眠恢复及完整 VoiceOver 人工验收。新远程语音入口复用已有转写链，但本轮没有新增真实麦克风联调记录。
