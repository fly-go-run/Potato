# iPhone 搜索后追问 HTTP 400 排查

2026-09-16。范围：用户提供的 iPhone 截图所对应的 SwiftUI 客户端与云端 Worker。首次排查未部署；随后按用户明确授权完成下述云端补丁发布，未发布 iOS。

## 结论

手机与线上 Worker 的消息协议版本不一致。新版 iOS `ChatService.toolHistory` 会在后续请求中加入 assistant `tool_calls` 及配对的 `role: tool` 搜索/代码执行历史；线上 Worker 的 `validate` 只接受 system、user、assistant，因此在调用模型之前返回 HTTP 400 `Invalid message role.`。

客户端 `ChatService.events` 将所有 HTTP 400 显示成“服务无法处理请求，请检查模型名称和接口兼容性。”，隐藏了真正的角色校验错误。截图上一轮有“查看 9 个来源”，与此触发条件一致。没有读取用户手机的完整对话或取得截图对应的请求 ID，因此结论依据线上代码与同形合成请求复现。

## 核验证据

- `wrangler deployments list` 显示当前 100% 流量版本为 `1a91e9e5-9a5f-4659-9cc4-01e2f9925eb2`，部署时间 `2026-09-13T12:31:32.876Z`，说明为 `Model-selected Python tool with iOS execution results 2026091306`。
- 通过 Cloudflare API 只读下载当前线上脚本，确认其消息角色白名单为 `["system", "user", "assistant"]`，没有请求侧的工具历史支持。
- 本地 `native/potato-worker/src/index.ts` 已实现工具历史配对与转发，但该支持晚于线上版本；此前 `ios-architecture-review-20260913/fix-batch-1-report.md` 也明确记录“未部署或发布”。
- 对下载的线上实际 `validate` 函数执行合成请求：普通 user 消息通过；加入一组搜索调用、结果、最终回复及追问后，稳定返回 400 `Invalid message role.`。同一请求经本地 `validate` 通过且保留工具消息。
- 将本地校验后的同一合成搜索历史请求发往现有 DeepSeek 官方配置，模型为 `deepseek-flash`，沿用默认思考设置并提供 `web_search` 工具：HTTP 200、回复 `OK`、收到 `[DONE]`。仅使用合成内容，没有发送用户截图对话。
- `node --test test/api.test.ts`：14 项全部通过，覆盖工具历史直通/搜索路径与非法配对拒绝。

## 处理建议

将已验证的工具历史兼容代码部署到手机使用的 Worker；手机已有对应发送逻辑，本故障不需要重新发布 iOS。部署后应经真实云端入口验证“有搜索历史的追问”和普通新对话。此次没有执行部署。

临时可新建对话继续；原会话中反复重试仍会携带同样的工具历史，更换模型也无法绕过 Worker 的角色校验。新会话再次产生搜索/执行记录后的追问仍可能触发，因此只是临时措施。

后续可让客户端识别云端稳定错误码并保留请求 ID，避免所有 400 都被描述成模型配置问题。

## 已部署与线上复验

用户授权部署后，新版本 `1475115f-7a87-414f-b90c-0ea611047332` 已承接 **100%** 流量，两个现有入口 `potato-remote.recodex.top` 与 `potato-iphone-api.pal-xu.workers.dev` 均绑定该 Worker。

发布采用线上旧版本的最小补丁：从当前 `src/index.ts` 编译 `validate()`，仅替换线上包中的同名函数，前后其余字节完全一致。保留线上其余运行逻辑，不包含本地尚未部署的记忆检索等变动。发布差异见 [validator-hotfix.patch](search-followup-400-20260916/validator-hotfix.patch)，构建与线上回读校验见 [build.json](search-followup-400-20260916/build.json)、[verification.json](search-followup-400-20260916/verification.json)。

首次尝试部署整个本地 Worker 时，Cloudflare 拒绝 `limits.cpu_ms = 30000`：当前账户为免费套餐，不支持自定义 CPU 限额，错误码 `100328`。该尝试未改变线上版本。随后最小补丁沿用线上配置（没有自定义 CPU 限额），通过 `wrangler deploy --keep-vars --config /tmp/potato-followup-hotfix/wrangler.json` 发布成功。本地完整配置未改；后续全量发布仍须单独处理套餐/CPU 兼容性，不能认为本地全部 Worker 变动已上线。

验证结果：

- 本地当前源码：TypeScript 检查、80 项测试、预构建通过。
- 最终最小补丁包：在 Miniflare 实际 Worker 运行时通过 14 项检查，覆盖直通/搜索路径的普通聊天、配对工具历史、非法配对及未授权拒绝，另有语法检查和部署预构建通过。
- 使用已有云端验收账号，经手机实际 `/v1/chat/completions` 入口发送合成内容，带 `sandbox.enabled: true`，模型为 `deepseek/deepseek-flash`，沿用默认思考设置。
- 部署前搜索后追问：HTTP 400 `Invalid message role.`，请求 ID `4ca2bde6-b1ab-4603-9568-bb58c79edeb3`。
- 部署后普通聊天：HTTP 200、`POTATO_FOLLOWUP_OK`、完整 `[DONE]`，请求 ID `5b5354fe-1356-4d40-901b-cf9d90426289`。
- 部署后同形搜索后追问：HTTP 200、`POTATO_FOLLOWUP_OK`、完整 `[DONE]`，请求 ID `a436e82d-23a0-4cd0-8a84-34811d371519`。
- 模型目录正常；线上所有绑定（含 Secret 名称）、普通变量、兼容日期/标志、CPU 限额及可观测性配置与发布前逐项相同。Secret 值未读取或重写。
- 回读线上代码 SHA-256：`2fc465ed4b3b08914529eb2101e70c2d847dcda245c2ee3d54e2212ad4c6a653`，与最终发布包完全一致。

不需要升级 iPhone App；可直接在原对话重试。以上是线上接口验收，没有代替用户在真机原会话中的验收。
