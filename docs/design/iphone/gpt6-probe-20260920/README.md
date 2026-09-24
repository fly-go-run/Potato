# GPT-6 上游兼容性探测

2026-09-20，按用户要求只测试，不发布客户端或 Worker，不更新云端模型目录。

使用现有 sub2api 服务配置，在内存中读取凭据，仅发送合成的短算术问题；结果不包含凭据、用户聊天或附件。

- 上游 `/models` 返回 HTTP 200，包含 `gpt-6` 与 `gpt-6-astra`。
- 请求的精确模型为 `gpt-6`，未替换为 Astra。
- Chat Completions 流式请求：默认参数和 `reasoning_effort=low` 均返回 HTTP 200、`text/event-stream`、正确答案 `323`、`finish_reason=stop` 与 `[DONE]`。
- 两次请求分别约 4.28 秒、1.77 秒。均为单次短请求，不能代表持续延迟或稳定性。
- 响应 model 字段均为 `gpt-6`；未独立验证上游内部模型映射。
- iPhone 当前仓库的云端目录策略仍只为 sub2api 列出 `gpt-5.6`。上游可调用不等于已在 iPhone 云端列表开放；本次没有修改该策略，也未实测线上云端目录。

本次覆盖模型目录与上游短文本 SSE 协议，不包含 iPhone 端到端 GPT-6、图片、工具调用或全部思考档位。原始脱敏结果见 `results.json`。

## 用户授权后的云端配置更新

同日用户随后要求“加上吧”，已将 `sub2api/gpt-6` 加入云端目录，并同步仓库策略，保留原有两个模型及 DeepSeek 默认模型。GPT-6 仅声明实测的 `low` 档位，客户端另可选服务默认。

- 仅执行 `wrangler secret put CLOUD_PROVIDERS`；未上传 Worker 代码或发布 iPhone 安装包。
- 变更前版本：`9e1a65a0-89d2-4c81-baaf-16e718fbc780`。
- Secret 更新版本：`7b585467-da66-4355-bea9-dd6dd19bf927`，部署来源 `secret`，流量 100%。
- 更新后立即回读短暂仍为旧目录；稍后回读确认三项模型全部生效，未重复提交更新。
- 使用已有云端登录态经线上 Worker 发起两次合成短文本请求，默认和 `low` 均 HTTP 200、正确返回 `323` 且收到 `[DONE]`，约 3.35 秒、1.88 秒。见 `cloud-catalog.json`、`cloud-chat-results.json`。
- 8 项目录、鉴权、路由相关测试通过。未进行此次 GPT-6 的 iPhone UI 端到端测试；现有客户端通过“更多模型 → 刷新模型列表”读取新目录。
- 现有 GPT-5.6 条目继续发送精确 ID `gpt-5.6`。OpenAI 官方定义该别名指向 Sol；sub2api 内部映射未独立审计。官方来源：https://developers.openai.com/api/docs/models/gpt-5.6-sol

## 明确使用 GPT-5.6 Sol

2026-09-20 用户明确要求使用 `gpt-5.6-sol` 与 `gpt-6`，已将前者替换原目录中的 `gpt-5.6` 别名，显示名称为 GPT-5.6 Sol。保留 GPT-6、DeepSeek 条目及原默认模型。

- 上游精确 ID `gpt-5.6-sol` 默认和 low 请求均成功，响应 model 字段均为 `gpt-5.6-sol`。见 `sol-upstream-results.json`。
- 仅更新 `CLOUD_PROVIDERS`，新版本 `ac5b9242-d351-477a-8f33-c7632323e746`，来源 `secret`、100% 流量；未上传代码或发布 iPhone 包。
- 线上目录回读确认 `sub2api/gpt-5.6-sol` 与 `sub2api/gpt-6`。见 `sol-cloud-catalog.json`。
- 两个精确型号分别经线上 Worker 返回 HTTP 200、正确答案 `323` 与完整 SSE 结束标记。见 `sol-and-gpt6-cloud-chat-results.json`。
- 8 项相关目录、权限与路由测试通过。iPhone 需要刷新模型列表；未在本轮执行 iPhone UI 验收。
