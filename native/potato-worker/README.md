# Potato iPhone · Cloudflare Worker

2026-09-19 **20 张聊天图片支持已部署**，版本 `9e1a65a0-89d2-4c81-baaf-16e718fbc780`，回读确认 100% 流量且健康接口正常。基于上一线上版本隔离发布，仅调整 `src/index.ts` 图片数量校验；92 项回归、类型检查和 dry run 通过。每条消息最多 20 张内联图片（含文字最多 21 个内容段），仍保留 4 MiB 请求上限；代码执行工具输入文件限制独立。配套 iOS 构建 2026091901 已发布至 TestFlight 个人内测组，见 [发布记录](../../docs/design/iphone/attachments-20260919/release-2026091901/README.md)。

2026-09-17 **代码示例执行策略已部署**，版本 `8d8d69b8-6452-4c49-9ae9-4173cea1c8c0`，100% 流量。写/解释代码默认不调用沙箱，明确运行或实际数据/文件任务按需执行；代码在前、对应输出在后。此次隔离发布仅包含两处提示规则调整，未携带工作树中的远程排队改动。91 项回归和三条真实模型验证通过，见 [部署与验证](../../docs/design/iphone/code-blocks-20260917/worker-policy-release/README.md)。

2026-09-16 **云端异步回复任务已部署**（版本 `0a601f87-f1b3-4a0c-a0dc-365aef1ac97a`）：生成由云端任务持有，手机断网或退出后可恢复补读；发送去重、显式停止与正文/游标一致保存已接入。部署顺序、验证和边界见 [异步回复说明](../../docs/design/iphone/async-replies-20260916/README.md)。

2026-09-13 最新模型更新：手机云端聊天目录精简为 DeepSeek V4.1 Flash（`deepseek-flash`）与 GPT-5.6，思考档位按官方文档及真实请求验证；43 项测试通过并已部署。图片生成仍受当前 sub2api 分组权限限制。当前目录、测试证据与发布版本见 [模型能力验证](../../docs/design/iphone/model-capabilities-20260913/README.md)，优先于下方历史记录。

2026-09-13 更新：已启用**仅指定邮箱可用的云端模型账号模式**，复用 DeepSeek 与 sub2api 本地配置；云端登录拥有独立权限，不授予远程电脑控制。普通模型、目录、语音和沙箱现要求云端账号会话，旧固定设备令牌不再用于这些接口。配置、同步脚本及验证见 [云端模型与授权](../../docs/design/iphone/cloud-models/README.md)。下方单用户设备令牌配置是此前版本的历史说明。

轻量的个人使用 API，给 SwiftUI 客户端提供 `POST /v1/chat/completions`。目前只支持 **OpenAI Chat Completions 兼容上游**，不是 Anthropic 原生 Messages API 适配器。部署在 Cloudflare，不需要 VPS。

## 已实现

- 请求支持 OpenAI Chat Completions 的 assistant `tool_calls` → 配对 `tool` → assistant 文字历史；校验全请求调用 ID 唯一及连续、完整配对，手机按回复整段裁剪工具历史。

- SSE 增量回复、客户端取消信号传递；自动工具循环仅保留有上限的当前轮内容。
- Exa 自动搜索：现有模型决定是否调用，兼容一轮多个调用，每条回复最多4次；来源与搜索状态通过 SSE 返回。
- 豆包流式语音：`GET /v1/audio/transcriptions` WebSocket，16kHz单声道PCM，最多60秒；沿用设备令牌，供应商凭据仅存在Worker。
- 设备连接令牌与上游 API key 分离；恒定时间比较；缺少密钥/模型白名单时拒绝调用。
- 模型白名单、请求最大 4 MiB、消息上限 200、输出 token 上限、20 次/60 秒限速绑定。
- `GET /v1/models` 沿用设备鉴权，读取同源上游目录，只返回白名单内模型的名称与能力；目录缺失时标明为配置列表，不宣称模型在线可用。界面所选 `thinking` 与 `reasoning_effort` 会转发到直接聊天及搜索后的调用。
- 只转发明确支持的字段；拒绝远程图片 URL；图片必须内联，文本/PDF 由客户端提取文字。
- 上游地址只在部署配置中设置，强制 HTTPS，禁止自动重定向与携带 URL 凭据。
- HTTP 错误去除上游敏感正文；不记录消息、附件或凭据；返回可追踪 request ID。

这是**单用户连接方案**，不是多租户账号系统。设备令牌不要打包进 App 或公开分发。限速按 Cloudflare 地点执行，不能视为严格的全局账单上限。R2 文字历史同步与记忆检索已实现（默认关闭，见下文）；附件仍由 iPhone 本地保存。可靠后台记忆任务队列尚未实现。

## 本地验证

```sh
npm ci
npm run types
npm run check
npm test
npm run build  # wrangler deploy --dry-run，不发布
```

2026-09-13 当前工作树 36 项测试与类型检查通过，覆盖聊天、远程中继、沙箱、自动 Exa、豆包协议、思考转发与模型目录。搜索分支的 `reasoning_content` 转发保留同帧正文并检查跨搜索轮 UTF-8 累计预算，见 [思考修复记录](../../docs/design/iphone/ios-redesign-20260913/reasoning-fix.md)。新增目录和档位转发见 [本机模型设置](../../docs/design/iphone/ios-redesign-20260913/local-model-fix.md)。这些修复尚未部署；测试使用网络/沙箱替身，不能代替线上联调。此前 dry run 打包证据仍仅代表当时版本。

## 配置与部署

1. 在 `wrangler.jsonc` 设置自己的完整 `UPSTREAM_URL`、逗号分隔的 `ALLOWED_MODELS` 以及 `MAX_OUTPUT_TOKENS`。当前白名单与本机土豆的 DeepSeek 模型一致；空列表会安全拒绝请求。
2. `npx wrangler login` 登录自己的 Cloudflare 账号。
3. 用 `npx wrangler secret put CLIENT_TOKEN` 保存至少 32 字符的随机设备令牌，用 `npx wrangler secret put UPSTREAM_API_KEY` 保存供应商密钥。不要把真实值写进 Git、日志或聊天。
4. `npx wrangler deploy` 发布后，将 `https://<worker域名>/v1/chat/completions` 填入 iPhone 设置，填白名单中的模型名和**设备令牌**，关闭本地体验模式。
5. 以普通问答、图片、断网、停止生成和额度错误做真实联调。`GET /health` 只证明处理器存活，不代表模型可用。

2026-09-12 经用户明确授权后，已部署到 `https://potato-iphone-api.pal-xu.workers.dev`，版本 `35e7c40c-510c-4ab1-b67f-12da6ce30aaa`。`GET /health` 返回200，未带设备令牌的聊天请求返回401；iPhone模拟器经Worker真实连接、问答与重启恢复通过 `LiveWorker.xcresult`，系统示例照片问答通过 `LiveWorkerPhoto.xcresult`。非敏感部署记录见 [deployment.json](deployment.json)。

`scripts/deploy-desktop.mjs <模拟器UUID>` 会读取桌面当前供应商凭据、上传 Cloudflare Secret、部署，并生成新的设备令牌导入模拟器；每次运行会轮换令牌，请勿将它当作普通状态检查。临时凭据文件权限600并在结束时删除，凭据不会写入源码或部署记录。常规代码更新使用 `npx wrangler deploy`，保留已有Secret。

免费额度适合轻量请求；图片/长上下文的 JSON 处理可能超过免费 CPU 限制，不能承诺完整 AI 应用永久免费。模型服务单独计费。

## 云端模型清单（手机可改）

服务商、接口地址和 API 密钥仍由 `scripts/sync-cloud-models.mjs` 写入 `CLOUD_PROVIDERS` Secret；启用哪些模型、默认用哪个，改由 `CloudModelSettings` Durable Object 保存，管理员在 iPhone「设置 → 云端模型」中增删。

- `GET /v1/models`：返回启用清单，附 `revision` 和 `can_edit`。未保存过清单时沿用 `CLOUD_PROVIDERS` 里的精选模型。
- `GET /v1/models/available`（管理员）：实时读取各服务商 `/models` 目录，过滤嵌入、语音、图像等非对话模型。
- `PUT /v1/models/enabled`（管理员）：`{models, default_model, revision}`。只接受已启用、精选或服务商目录中存在的模型；`revision` 不一致返回 409。
- 管理员由 `CLOUD_ADMIN_EMAILS`（逗号分隔，Secret）指定；未设置时任何人都不能修改。
- 新加入的模型不带思考参数（显示“服务默认”），只有精选或官方文档确认的模型才有思考档位。
- 同步脚本重新部署服务商后，已保存的清单保持不变；属于已删除服务商的模型会自动失效。

## 桌面 Exa 与豆包

已获用户授权，将桌面 Exa 与原生豆包配置保存为 `EXA_API_KEY`、`DOUBAO_API_KEY`、`DOUBAO_APP_ID` Secret。当前豆包使用新API Key模式，APP_ID为空；资源为 `volc.seedasr.sauc.duration`。普通代码部署保留这些Secret。

iPhone没有联网开关。Worker向模型提供web_search，搜索完成后继续回答并引用来源。语音采用桌面已有的ASR，iPhone点麦克风原位转写，点发送收齐最终结果后提交，也可先修改；回复朗读仍使用iOS系统TTS。迁移实现、调用预算及验收边界见 [搜索与语音记录](../../docs/design/iphone/search-and-speech.md)。

文档依据：[Cloudflare 流式示例](https://developers.cloudflare.com/workers/examples/openai-sdk-streaming/)、[Workers 最佳实践](https://developers.cloudflare.com/workers/best-practices/workers-best-practices/)、[Rate Limiting](https://developers.cloudflare.com/workers/runtime-apis/bindings/rate-limit/)。

## Python 沙箱试接

新增 `POST /v1/sandbox/run`，沿用设备令牌，独立限制3次/60秒。请求为 `{ "code": "print(1 + 2)", "files": [] }`；文件为安全名称与base64，最多4个/合计约2MB。Python执行60秒，每次新建E2B沙箱，结束销毁，TTL120秒兜底，容器禁止外网。输出包含执行状态、stdout/stderr、文字和最多8个图片/文档产物。

`E2B_TEMPLATE=chat-web-office-pdf` 复用用户旧 `chat-web-dev` 项目的Office/PDF模板名。2026-09-12已验证用户提供的E2B凭据及该模板，并在用户明确授权后保存 `E2B_API_KEY` Worker Secret；未改变设备令牌和模型密钥，未修改旧项目部署。没有配置该Secret的其他环境仍返回503。不要通过执行旧项目部署脚本迁移，以免改变旧服务。

真实验证已通过：同一SDK适配读取合成CSV（三行合计60），生成PNG、PDF、DOCX、XLSX；iPhone经Worker实际运行Python，取回图表与上述三类文档，打开PDF并重启恢复。SDK直连的CSV输入验证与手机生成文件验证分开记录，没有把它们称为手机系统文件提供器的完整导入验收。结果与截图见 [原生验收记录](../potato-ios/design-qa.md)。

图片从Code Interpreter的富结果收取，文档保存在 `/home/user/output` 后取回。支持PNG/JPEG/PDF/CSV/TXT/MD/DOCX/XLSX，单文件2MB，总base64不超过4MB。客户端只显示明确提交的Python执行结果；尚未启用自动工具规划循环。详细比较与历史证据见 [回复与沙箱调研](../../docs/design/iphone/replies-and-sandbox.md)。

## iPhone 远程控制（本地实现）

原生手机可通过中继继续桌面项目与会话、回应审批/提问并停止任务。新增 Cloudflare 同账号关联，桌面登录后需明确开启远程访问。2026-09-12 中继与 Access 登录已部署至 `https://potato-remote.recodex.top`，实际验收状态见 [发布记录](../../docs/design/iphone/remote-control/release.md)；配置与限制见 [远程控制说明](../../docs/design/iphone/remote-control/README.md)。


## 跨对话检索与记忆

`RECALL_BUCKET` 绑定 `potato-personal-recall` R2 Standard 桶；检索在 Worker 内执行，不再需要 E2B。部署新环境前创建该桶。iOS 在“更多 → 记忆与历史”启用后，文字历史按内容版本增量同步；Worker 在内存中对候选文件进行关键词/日期检索，来源经验证后流式返回。

接口为 `GET /v1/recall/status`、`POST /v1/recall/sync`、`POST /v1/recall/memory`，都沿用账号认证。聊天请求可传 `recall: { enabled: true, auto_memory: false, timezone: "Asia/Shanghai" }`；这些参数不会转发给模型供应商。未配置 R2 时显式返回 503。旧客户端未开启时沿用现有聊天流程。

设计、容量、删除语义、测试和发布边界见 [实施记录](../../docs/design/iphone/cross-chat-recall/implementation.md)。此目录与 iOS 包需一同发布才能启用；创建 R2 桶不等于已发布。

## 云端回复实时订阅

`GET /v1/chat/jobs/{id}/events?after={cursor}` 使用原有云端账号鉴权，按持久化序号补读，再推送 SSE。每个 `data:` 是原分页协议的 `{state,last,events,failure?}` JSON；只有消费到 `last` 后终态才生效。客户端按最后成功应用的序号重连。

事件持久化后唤醒订阅者，快速事件以 40 ms 窗口合并；读取按背压拉取，不为慢客户端积攒额外事件队列。每任务最多 8 个订阅，15 秒心跳、60 秒连接轮换。断开订阅不取消 alarm 拥有的生成；`POST /cancel` 才停止任务。原 GET 分页接口保留，方便旧客户端和回退使用。上线时先部署 Worker，再分发 iOS；2026-09-16 已部署版本 `51e229f1-6acf-4cf6-b620-98a831882d61`，100% 流量；真实云端订阅和断线续接验证通过，见 [发布记录](../../docs/design/iphone/streaming-20260916/release-2026091607/README.md)。
