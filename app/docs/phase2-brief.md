# Phase 2 任务包：审批 + 附件 + 设置（执行者：Codex）

延续 Phase 1（交付与已知限制见 `app/docs/phase1-report.md`）。契约见 `app/docs/api-contract.md` §4/§5/§6；本文补充已对照后端源码核实的细节。约束与 Phase 1 完全相同（只动 `app/`、不动 `tokens.css`、不新增依赖、只用语义色类）。

## 交付物

### 1. 审批（HITL）

- 轮询服务：当前会话打开时每 2.5s `GET /api/console/push-messages`（可带 `?session_id=`）。
  返回 `{messages, pending_approvals}`；`pending_approvals` 是**全部**待审批，前端按
  `root_session_id === 当前 session_id` 过滤（后端 `src/qwenpaw/app/routers/console.py:396-449`）。
  每项字段：`request_id`、`session_id`、`root_session_id`、`tool_name`、`tool_params`、
  `severity`、`findings_count`、`findings_summary`、`source_type`、`driver`、`created_at`、
  `timeout_seconds` 及 display 字段（读 `approval_display_fields` 确认）。
- 审批卡内联在对话流末尾：工具名 + 参数摘要（可展开完整参数）+ severity 标识 +
  批准 / 拒绝按钮 + 「批准同类」次级操作（`scope:"similar"`）。
- `POST /api/approval/approve|deny`，body `{request_id, session_id: <root_session_id>, scope?}`；
  scope 小写 `"exact" | "similar"`，默认 exact。403 表示 root session 不匹配；404 已失效
  （超时/已处理）——失效时静默移除卡片。
- 批准后原 SSE 流自动继续，**不要**重连。deny 后流也会继续（agent 收到拒绝结果）。
- 流式进行中才需要轮询；空闲会话不轮询（省电）。切会话/组件卸载要清 interval。

### 2. 附件上传

- 把 Phase 1 暂存的 `pendingImages` 接上真实上传：发送时先逐个
  `POST /api/console/upload`（multipart 字段 `file`）→ `{url}`（存储名），
  消息 content 变为 `[{type:"text",...}, {type:"image", image_url:"<url>"}...]`。
- 非图片文件也支持（文件选择器放开限制）：`{type:"file", file_url, filename}`。
- 渲染：气泡内图片用 `GET /api/files/preview/{name}?token=<token>`（`<img>` 无法带
  header 所以 token 走 query；本机鉴权关闭时无 token，则不带 query 参数）。
  文件显示为文件名 chip。历史消息里的 image/file content block 同样要渲染。
- 上传前检查 `GET /api/settings/upload-limit`，超限提示。上传失败要把错误显示出来
  且不发送消息（用户输入不丢）。

### 3. 设置页（替换骨架 `SettingsView.tsx`，单页分区）

- **模型**：`GET /api/models` 列出 provider 与模型（响应形态读
  `src/qwenpaw/app/routers/providers.py`，prefix `/models`）；选择后 `PUT /api/models/active`；
  provider 的 API key / base URL 配置 `POST /api/models/{provider_id}/config`。
  保存后刷新 store 的 activeModel，让 composer 立即解除禁用。
- **主题**：浅色 / 深色 / 跟随系统 三选，用现有 `src/lib/theme.ts` 的
  `getThemePreference/setThemePreference`。
- **语言**：zh / en。自建轻量 i18n（`src/lib/i18n.ts`：两份 dict + `t(key)` +
  localStorage 持久化 + 触发重渲染的 zustand/store 或 context，**不引入依赖**）。
  把现有组件里的硬编码中文文案全部抽进 dict。

### 4. Backlog 清理（来自 Phase 1 review）

- Shiki 改精简 bundle（`shiki/core` + 动态按需语言，常用集合：ts/js/tsx/json/bash/python/
  html/css/md/yaml/sql/go/rust/java/c/cpp/diff；未知语言退化为普通 pre），消除 500kB+ chunk 警告。
- `sendMessage` 里 `window.history.replaceState` 换成 react-router `navigate(..., {replace:true})`，
  注意不能触发 ChatView 重新 openChat 打断进行中的流（现有 `chatId !== activeChatId` 守卫应已覆盖，验证之）。

### 5. 自查报告 `app/docs/phase2-report.md`

对照 REFACTOR_PLAN.md Phase 2 验收标准逐条说明；记录联调过程与已知限制。

## 验证方式

- `npm test`（为审批过滤/上传 content 组装/i18n dict 完整性补单测）+ `npm run build` 通过。
- 真实联调（桌面版后端，端口 `lsof -nP -iTCP -sTCP:LISTEN | grep qwenpaw` 查）：
  1. 审批走通两条路：composer 审批级别切到 STRICT，发一条会执行 shell 的消息 →
     审批卡出现 → approve → 工具执行、流继续;再来一轮 → deny → agent 收到拒绝并回复。
  2. 上传一张图片完成一轮对话，刷新后历史中图片正常显示。
  3. 未配置模型引导、主题切换、语言切换人工过一遍。
  4. 联调会话测完 `DELETE /api/chats/{id}` 清理，不留 running。
- dev server 可能已在 5174 运行（本会话起的，连着桌面后端），直接用即可；被占用就让 vite 自动换端口。
