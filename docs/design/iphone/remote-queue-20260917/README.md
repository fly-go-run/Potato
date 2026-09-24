# iPhone 远程会话：排队与打断发送

2026-09-17。范围：仅 iPhone 远程电脑会话、Rust 共享核心远程协议、中继操作白名单。手机普通聊天不增加队列。复用 GPUI 已有持久化 outbox；不重复实现手机调度器。

## 调研与界面参考

已逐张查看仓库中保存的真实公开截图：

- [Codex 发送菜单与队列](../../../../native/potato-gpui/design/follow-up-queue/references/codex-composer.png)：队列贴在输入框上沿，Queue / Steer 分开，单条附带管理操作。
- [Claude Code CLI](../../../../native/potato-gpui/design/follow-up-queue/references/claude-cli.png)：待发文本在输入框上方；可取回编辑，Esc 打断。这是 CLI 截图，不代表 Claude 桌面版。
- [原图来源记录](../../../../native/potato-gpui/design/follow-up-queue/references/README.md)。图片只证明对应版本外观。

本次重新读取官方文档：

- [Codex/ChatGPT 桌面 Follow-up behavior](https://learn.chatgpt.com/docs/reference/settings)：选择引导当前运行或等下一轮。这里不能推定所有普通 ChatGPT 网页/手机聊天版本都相同。
- [Claude Code 排队时机](https://code.claude.com/docs/en/interactive-mode#queue-messages-while-claude-works)：普通消息可能在当前工具调用结束后进入同一轮；Esc 中断后发送待发消息。

Potato 的“排队”明确等整轮结束；“打断并发送”停止当前轮并优先执行所选消息，保留其他条目次序。它不是单纯在工具边界补充当前轮的 Steer。

## 手机交互（按用户提供的对话截图修订）

- 正在运行或已有待发消息：点击发送后，消息立即进入主对话滚动区域，完整右对齐气泡显示；默认排队。输入提示为“继续补充…”，继续输入不主动收起键盘。
- 每条气泡上方只显示轻量状态（发送中 / 排队中 / 打断中 / 已暂停）和“…”菜单；从该消息选择“打断并发送”、编辑或删除。没有输入框上方的独立队列，也没有发送前选择菜单。
- 排队消息不折叠、不截断；随对话滚动。电脑确认接收前保留本地发送状态，网络结果不明时显示“发送待确认”，不会假装成功入队。
- 编辑使用独立编辑器，保留输入草稿；若保存失败，编辑文字留在面板。编辑期间消息可能正常开始执行，此时拒绝修改已发送消息。
- 手动停止暂停队列；失败与电脑重启沿用核心的暂停/恢复规则。点击“继续发送”后恢复自动发送。
- 模型选择针对后续消息，入队保存所选模型；不修改当前运行的配置。
- 仅桌面声明 outbox_protocol=1 时启用；旧电脑保留“补充当前任务”行为。手机普通聊天保持原交互。

## 可靠性与协议

- `chat` 快照增加版本与脱敏队列：只返回 ID、文字、状态、附件数，不传请求上下文、凭据或内联附件。
- `send` 可指定 delivery_mode=queue/interrupt；未指定保持旧手机行为。
- `outbox` 支持 save/delete/promote/pause/resume，按 chat_id 解析服务器会话，不能由手机传任意 session_id。
- 手机在发请求前持久化发送方式、操作 ID 与打断目标。重试沿用原操作 ID；服务器收据和 outbox 接收记录共同防重复。
- 立即发送和队列条目提升，在 outbox/runs 锁内核对 expected_run_id 后才改变队列并取消。过期请求返回 412，不影响替代任务；空目标仅匹配确实空闲的会话。
- 核心只在旧运行释放后派发下一条；派发保留 remote_operation_id 以便历史与收据核对。
- 快照先读取队列，再读取历史；历史用户消息携带发送操作 ID。手机按操作 ID 去重，在待发送气泡变为正式历史时保持一份内容；相同文字的不同消息仍分别保留。成功修改队列后，丢弃更早发起的轮询结果。
- 队列由电脑 SQLite 持久化，手机离开不影响电脑执行。电脑仍需运行；重启恢复为暂停。既有快照大小限制仍适用。

## 验证与发布

前一版底层验证：iPhone 179 项单元测试、远程核心 20 项、中继 11 项、核心队列 9 项（队列和远程测试有重叠）、排队模型真实线协议测试以及 Worker TypeScript 检查通过。

本轮将 UI 改为对话内气泡，新增立即显示、回执与历史按操作 ID 去重的单元测试，以及服务端历史标识测试。使用独立 iPhone 17 / iOS 26.3.1 模拟器回归：连续四条显示在主对话中、编辑、删除、发送后从单条菜单打断，并检查派发后只保留一条正式消息。最终结果：29 项 iPhone 单元测试、2 项 UI 测试、21 项核心远程测试全部通过；详见 [iPhone 测试摘要](inline-test-result.json)。

截图：[对话内中文连续消息](queue-inline-chinese.png) · [单条消息菜单](queue-inline-chinese-actions.png) · [打断中](queue-inline-interrupting.png)。旧的 queue-four-messages / queue-send-options / queue-interrupting 图片是被本次替换的第一版，不代表当前设计。

独立合成服务：`native/potato-ios/scripts/remote-queue-fixture.py`，仅监听 localhost:19017，不调用模型或真实远程电脑。UI 回归入口 `RemoteQueueUITests`；测试会检查专用服务是否可用，未启动时自动跳过。

iPhone 端已随 TestFlight 0.2.2（2026091701）发布；桌面与中继尚未随本次部署。功能完整启用仍需配套更新。上线顺序：先放行中继 outbox 操作，再更新桌面核心，最后发布手机端。仅装新版手机、电脑仍旧版时不会出现新队列入口。
