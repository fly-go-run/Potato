# iPhone 云端异步回复

2026-09-16。Worker 已部署，真实云端任务及断连后补读验证通过；iOS 0.2.2（2026091604）已加入既有个人内测组，Testing 与中文说明均读回确认。最终分发状态见 [发布记录](release-2026091604/README.md)。

用户提供的 iPhone 截图显示：正文生成到一半，重新进入应用后变成“上次生成已中断”。本次明确针对截图所示的原生 iOS 云端聊天及其 Cloudflare Worker；未修改 GPUI/potato-core、远程电脑任务或旧客户端。

## 原因与行为

此前 `WorkspaceStore.generate` 直接消费 `/v1/chat/completions` 的 SSE；Worker 将请求取消信号转发给模型和工具；重启恢复又把所有 streaming 消息变为 stopped。手机仅保存了已收到的部分内容。

新云端聊天流程：

1. iPhone 先保存稳定的回复 ID、原账号/服务和精确请求正文，再调用 `PUT /v1/chat/jobs/{id}`。凭据仍只放 Keychain。
2. Worker 按“账号 + 回复 ID”定位 SQLite Durable Object，事务保存请求并调度 alarm，然后返回接收状态。重复 ID/相同请求返回原任务，不重新生成；不同请求返回 409。
3. alarm 承担模型及工具调用，独立于手机 HTTP 连接，顺序保存 SSE 事件。仍复用现有云端模型路由、搜索、记忆与 Python 工具循环。
4. 手机使用 `GET /v1/chat/jobs/{id}?after={cursor}` 分页补读。正常约每 750 ms 读取一次；断网、超时和临时服务错误会重试，页面提示正在恢复，不把手机连接失败当作模型生成失败。
5. 每页的正文、思考、工具结果、文件引用与游标一并保存。保存失败则回滚本页，避免重启后重复文字或遗漏输出。接受回执后移除本地请求正文。
6. `POST /v1/chat/jobs/{id}/cancel` 是明确的云端停止命令。客户端先保存停止意图，断网后恢复也会送达；服务端即使尚未见到发送请求也保留停止标记，防止延迟 PUT 再次启动模型。
7. 重启时含云端任务 ID 的 streaming 消息继续恢复；旧直连消息仍按原方式保留中断内容。

## 存储与边界

- 输出及临时请求最长保留 7 天；完成/失败/停止后立即删除云端请求正文。到期清除输出，保留很小的去重标记，防止旧 ID 重新触发生成。本机已经收齐的历史不受此期限影响。
- 创建、读取、停止都验证云端账号，DO 名字由认证身份构造。未带账号无法访问其他账号任务。
- 任务输出保持现有 16 MB 总预算；大工具事件分块写 SQLite，避免单值上限，切片避免拆开 Unicode 代理对。
- 退出、后台和手机网络抖动不再主动取消已接收的云端任务。提交尚未抵达服务端时不能开始执行；重新打开/联网后按相同 ID 自动确认或提交。
- 上游模型失败、输出 token 上限、工具超时仍可能导致失败；不得假装完整成功。DO 运行时若崩溃，恢复 alarm 会保留部分结果并报失败，不自动重跑可能已经执行的工具副作用。纯手机断网不触发这条路径。
- 不提供推送通知，也不等同于全部聊天历史跨设备同步。自定义直连接口不具备此后台协议，保持原 SSE 行为。
- 旧版已经中断的回复没有服务端任务 ID，无法自动恢复其剩余生成。

## 验证

使用合成账号/模型，不读取个人聊天、不调用计费模型。

- Worker `npm run check`、`npm test`：88 项通过，新增 6 项实跑 Miniflare/SQLite/alarm 测试。覆盖无人读取时完成、并发/回执丢失重试去重、请求冲突、跨账号隔离、停止先于发送、执行中停止、缺少结束标记、输出上限、大段中文/emoji 事件与游标补读。
- Worker `npm run build`：Wrangler dry run 通过，新增 `CHAT_JOBS` 绑定及追加迁移 `chat-job-v1`，随后已正式部署。
- iOS：全量 154 项单元测试通过；随后增加写盘失败案例，8 项异步回复专项与 4 条原生聊天过程 UI 回归通过。专项覆盖重启补读、回执丢失重发原请求、部分正文后的断网恢复、离线停止意图恢复、重复页去重、分页完整性、非法序列回滚、写盘失败回滚及新发送走任务接口。
- 日志：`/tmp/potato-async-worker-tests.log`、`/tmp/potato-async-worker-build.log`、`/tmp/potato-async-ios-tests.log`、`/tmp/potato-async-ios-targeted.log`、`/tmp/potato-async-ios-final.log`。iOS xcresult 在 `native/potato-ios/build/Logs/Test/`。
- 随后已完成线上模型联调与客户端断连后同任务补读，已发布 TestFlight；用户 iPhone 真机杀进程/蜂窝弱网验收仍未进行。

## 发布顺序

先部署 Worker（保留现有 Secrets，应用新增 DO 绑定/迁移），再发布新 iOS。旧 iOS 仍可使用原 `/v1/chat/completions`。新客户端依赖任务接口；不要先发客户端，否则会显示接口不存在。

实现依据：[Durable Objects alarms](https://developers.cloudflare.com/durable-objects/api/alarms/) 提供不依赖前台请求的任务执行入口。普通 HTTP `waitUntil` 不承担本方案的整个生成生命周期。
