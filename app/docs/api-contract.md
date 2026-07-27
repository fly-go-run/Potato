# QwenPaw 新前端 · 后端契约（Phase 0 产出）

> 本文是 REFACTOR_PLAN.md §3 的细化版，是新前端与后端之间**唯一**的契约。
> 所有字段均已对照后端源码核实；`app/fixtures/` 下是从真实后端抓取的样本。
> TypeScript 类型的权威定义在 `app/src/lib/protocol/types.ts`，两者冲突时以本文引用的后端源码为准并同步修正。

后端源码参照（仓库根相对路径）：

| 主题 | 源码 |
|---|---|
| SSE 事件状态机 | `src/qwenpaw/runtime/envelope.py` |
| SSE 外层帧 / turn_usage / rate_limited | `src/qwenpaw/app/channels/console/channel.py:380-530`、`src/qwenpaw/app/channels/base.py:1137-1260,1798-1870` |
| 数据模型 | `src/qwenpaw/schemas.py` |
| 聊天/上传/push-messages 路由 | `src/qwenpaw/app/routers/console.py` |
| 会话路由 | `src/qwenpaw/app/chats/`（经 `/api/chats`） |
| 审批路由 | `src/qwenpaw/app/routers/approval.py` |
| 鉴权路由 | `src/qwenpaw/app/routers/auth.py` |
| 模型/Provider 路由 | `src/qwenpaw/app/routers/providers.py`（prefix `/models`） |
| 旧前端参考实现（历史分组） | `console/src/pages/Chat/sessionApi/index.ts:205-310` |
| 旧前端参考实现（用量表） | `console/src/pages/Chat/turnUsage.ts` |

---

## 1. 通用约定

- 所有接口在 `/api` 前缀下，同源访问（dev 模式 Vite 代理到后端端口）。
- 鉴权启用时所有请求带 `Authorization: Bearer <token>`；token 存 `localStorage["qwenpaw_auth_token"]`。
- 401 → 清 token 跳 `/login`。
- 多 Agent 时带 `X-Agent-Id` 头（v1 只用默认 Agent，可不带）。

### 1.1 鉴权

| 接口 | 说明 |
|---|---|
| `GET /api/auth/status` | `{enabled, has_users}`；`enabled=false` 时跳过登录页 |
| `POST /api/auth/login` | `{username, password}` → `{token}` |
| `POST /api/auth/register` | 同上（首次使用，`has_users=false` 时） |

## 2. 聊天流（核心）

### 2.1 发送

`POST /api/console/chat`，`Content-Type: application/json`：

```json
{
  "input": [{"role": "user", "content": [
    {"type": "text", "text": "..."},
    {"type": "image", "image_url": "<upload 返回的存储名>"},
    {"type": "file", "file_url": "<存储名>", "filename": "原文件名.pdf"}
  ]}],
  "session_id": "console:default",
  "user_id": "default",
  "channel": "console",
  "stream": true,
  "request_context": {"approval_level": "AUTO"}
}
```

- `approval_level` ∈ `STRICT | SMART | AUTO | OFF`（`src/qwenpaw/config/config.py:1702`），
  默认值取 `GET /api/workspace/running-config` 的 `approval_level` 字段。
- 首条消息自动建会话并由 LLM 起标题，**无需**先建 chat。
- 返回 `text/event-stream`。必须用 `fetch` + `ReadableStream` 读（POST 无法用 EventSource）。

### 2.2 SSE 帧协议

每帧 `data: <json>\n\n`。JSON 分四类，判别方法见 `types.ts`：

1. **`object:"response"`** — 整轮生命周期。`status`: `created` → `in_progress` → 终态
   `completed | failed`。终帧的 `output` 含全部已完成消息（可用于兜底校正），
   `failed` 时带 `error: {code, message}`。
2. **`object:"message"`** — 消息信封。`type` ∈ `message | reasoning | plugin_call |
   plugin_call_output | function_call | function_call_output | mcp_tool_call |
   mcp_tool_call_output | progress | result`。
   同一 `id` 出现两次：`in_progress`（开卡，content 为空）和 `completed`（终值，content 为完整内容块数组）。
3. **`object:"content"`** — 内容块，凭 `msg_id` 归属消息：
   - `type:"text"` + `delta:true`：文本增量，按 `(msg_id, index)` 累积；
     `delta:false` 帧是该块终值（**替换**累积值，不追加）。
   - `type:"data"` + `data.arguments`：工具调用参数。首帧含 `call_id`/`name`/空 `arguments`；
     后续 delta 帧 `data` **只含** `arguments` 增量片段（拼接）；`delta:false` 帧为终值。
   - `type:"data"` + `data.output`：工具结果。每帧的 `output` 是**当前累积全量**（替换即可，
     不是增量）；`delta:false` 且带 `data.state`（如 `"success"`）的帧是终帧。
     富媒体结果时 `output` 是 JSON 数组字符串：`[{"type":"image","source":{...}}, {"type":"text","text":"..."}]`。
   - `type:"image"/"audio"/"video"`：完整 data-URI 一次性到达（无增量）。
4. **无 `object` 的杂帧**：
   - `{"type":"turn_usage", "session_id", "usage", "context_usage"}` — 尾帧，
     `context_usage.context_usage_ratio` 驱动上下文用量指示。
   - `{"type":"rate_limited", "error", "alternatives"}` — 限流，提示并给出可切换模型。
   - `{"error": ...}` — 兜底错误。

其他必须处理的规则：

- `sequence_number` 单调递增，可用于乱序防御（心跳帧会重复发 response，以 seq 判新旧）。
- reasoning 消息（`type:"reasoning"`）的文本同样走 `object:"content"` 增量，UI 折叠显示。
- 消息 `metadata.clear_history === true` → 清空本地当前会话历史（`/clear` 指令的实现方式）。
- 心跳：服务端可能重发 `object:"response"` `in_progress` 帧，幂等处理。

### 2.3 fixture 样本（真实抓包，2026-07-27，模型 gpt-5.6-sol）

| 文件 | 内容 |
|---|---|
| `app/fixtures/sse/simple-text.sse.txt` | 纯文本回复完整一轮（response 生命周期 + 文本增量 + turn_usage 尾帧） |
| `app/fixtures/sse/tool-call.sse.txt` | 带 `execute_shell_command` 工具调用一轮（plugin_call → 参数流 → plugin_call_output → 总结文本） |
| `app/fixtures/http/chat-history-tool-call.json` | `GET /api/chats/{id}` 的历史消息形态（含 user / plugin_call / plugin_call_output / message） |

`stream.ts` 的单测必须以这些文件为输入（fixture 驱动），并额外覆盖：
半帧切断（`data: {"te` 断在 TCP 边界）、`clear_history`、`rate_limited`、`failed` response、乱序 seq。

### 2.4 停止与重连

- 停止：`POST /api/console/chat/stop?chat_id=<chat uuid>`。
- 重连：刷新后 `GET /api/chats/{id}` 若 `status:"running"`，向 `POST /api/console/chat`
  发 `{"reconnect": true, "session_id": ..., "user_id": ..., "channel": "console"}` 接回同一条流。

## 3. 会话

| 接口 | 说明 |
|---|---|
| `GET /api/chats` | `ChatSpec[]`：`id`、`name`、`status`（`running` 标记）、`pinned`、`session_id`、`updated_at`… |
| `GET /api/chats/{id}` | `{messages, status}`，形态见 history fixture |
| `PUT /api/chats/{id}` | 改名 / 置顶 |
| `DELETE /api/chats/{id}` | 删除 |

**历史 → UI 分组规则**（参考旧实现 `sessionApi/index.ts:205-310`）：
按顺序扫描 `messages`，`role:"user"` 开新回合；连续的非 user 消息（reasoning、
plugin_call、plugin_call_output、message…）合并为同一个助手回合，回合内按原顺序渲染
（reasoning 折叠块、工具卡、文本气泡交错）。`plugin_call` 与 `plugin_call_output`
凭 `data.call_id` 配对渲染成一张工具卡。

## 4. 附件

| 接口 | 说明 |
|---|---|
| `POST /api/console/upload` | multipart 字段 `file` → `{url}`（存储名，回填到消息 content） |
| `GET /api/files/preview/{name}?token=<token>` | 图片/文件预览（`<img>` 直连需 query token，因为无法带 header） |
| `GET /api/settings/upload-limit` | 大小限制 |

## 5. 审批（HITL）

- 轮询 `GET /api/console/push-messages`（2.5s 间隔）→ `{messages, pending_approvals}`；
  按 `root_session_id` 过滤到当前会话，渲染内联审批卡。
- `POST /api/approval/approve` / `POST /api/approval/deny`：
  `{request_id, session_id: <root_session_id>, scope?: "exact" | "similar"}`。
- 批准后原 SSE 流自动继续（服务端 Future 解除阻塞），前端无需重连。
- composer 上的审批级别选择器写入每次请求的 `request_context.approval_level`；
  初始值取 `GET /api/workspace/running-config`。

## 6. 模型设置

| 接口 | 说明 |
|---|---|
| `GET /api/models` | provider + 模型列表 |
| `GET /api/models/active` | `{active_llm: {provider_id, model}, effective_max_input_length}`；`active_llm` 为空时**禁止发送**并引导去设置页（旧版行为） |
| `PUT /api/models/active` | 设置活动模型 |
| `POST /api/models/{provider_id}/config` | 配置 API key / base URL |

## 7. Phase 1 交付结构约定

- SSE 解析器：`app/src/lib/stream.ts`，**纯函数**（`(chunk: string, state) => {frames, state}`
  的增量 parser + 把 frame 归并进会话状态的 reducer），不碰 fetch/store，单测全部走 fixture。
- 消息 store：zustand，放 `app/src/stores/`。
- API 封装：`app/src/lib/api.ts`（fetch 包装、token 注入、401 处理）。
- UI 组件：`app/src/components/chat/`（气泡、markdown、reasoning 折叠、通用工具卡、Shell 特化卡）。
- 样式只用 `tokens.css` 暴露的语义类（`bg-surface`、`text-ink`、`border-line`、`bg-accent`…），
  **禁止硬编码色值**；token 文件本身只有架构方（Claude）改。
- 新增依赖需先报备，不得引入 antd / @agentscope-ai/*。
