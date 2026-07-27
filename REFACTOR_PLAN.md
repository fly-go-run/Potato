# QwenPaw 前端重构方案：Codex 风格的简洁办公 Agent 客户端

> 目标：把 QwenPaw 二次开发成个人办公用的 AI Agent 客户端。后端与 Tauri 桌面壳保留，
> 前端**绿地重写**为一个 Codex Desktop 风格的极简客户端。Windows 为主要目标平台。
>
> 本文档同时是给执行者（Codex）的任务书：每个阶段附带明确的验收标准。

---

## 0. 现状结论（摸底结果）

| 层 | 现状 | 决定 |
|---|---|---|
| Python 后端 | FastAPI + uvicorn，`/api` 下 ~310 个路由；聊天走 **SSE**（无 WebSocket）；SPA 由后端静态托管（`src/qwenpaw/app/_app.py` 的 catch-all） | **原样保留，一行不改** |
| Tauri 桌面壳 | sidecar 启动 PyInstaller 后端 → stdout 报端口 → WebView 跳转 `http://127.0.0.1:<port>/console`；Windows NSIS 打包链最成熟 | **保留**，仅替换它加载的前端 dist |
| 前端 console | ~108,500 行（606 文件）。~55K 行是服务端管理后台（Settings/Agent/Control 24 个导航入口）；聊天核心气泡/composer 来自厂商包 `@agentscope-ai/chat`（含深路径 import 和 DOM hack）；4.2K 行插件注册机制承载了核心路由 | **不在原地精简，绿地重写** |

**为什么不在原地精简：**
1. 核心路由和菜单都注册在插件 registry 里，首屏渲染阻塞在插件网络请求上——砍插件系统等于动全身。
2. 聊天气泡、composer、会话状态都在 `@agentscope-ai/chat` 厂商包内，本地代码通过深路径 import 厂商内部模块（`HostBubbles.tsx`）和 DOM 查询点按钮（`clearSenderAttachments`）来定制——这是"繁杂感"的根源，精简绕不开它。
3. 前后端唯一的硬契约是 **HTTP API + 静态 dist**：后端按 `QWENPAW_CONSOLE_STATIC_DIR` → `src/qwenpaw/console/` → `console/dist/` 的顺序找 SPA。产出一个新 dist 放进去即可，成本远低于拆旧摊子。

---

## 1. 目标形态（对标 Codex Desktop）

```
┌──────────┬─────────────────────────────────────┐
│ 新建会话  │                                     │
│ 定时任务* │        欢迎语（空会话时居中）          │
│          │     "今天要处理什么工作？"             │
│ 最近会话  │                                     │
│  · 会话A  │   ┌─ 对话流（气泡 + 工具卡片 + 审批卡）│
│  · 会话B  │   │                                 │
│  · …     │   └─────────────────────────────────│
│          │  ┌───────────────────────────────┐  │
│          │  │ [模型▾] [审批模式▾]     composer │  │
│ 设置 ⚙   │  └───────────────────────────────┘  │
└──────────┴─────────────────────────────────────┘
```
（* 定时任务为 v2）

设计原则：
- **一屏一事**：只有 聊天 / 设置 两个顶级视图（v2 加"定时任务"）。没有第二套导航、没有并存的两个会话列表。
- **默认隐藏复杂度**：工具调用折叠为一行卡片，点开看详情；审批卡内联在对话流里。
- **克制的视觉**：无 antd。中性色 + 单一强调色，浅/深色主题，系统字体，动效只用于状态反馈。

## 2. 技术选型

| 项 | 选择 | 理由 |
|---|---|---|
| 框架 | React 18 + TypeScript + Vite | 与现有工具链一致，Codex 执行时参考资料最多 |
| 样式 | Tailwind CSS v4 | 摆脱 antd 的视觉语言；design token 集中在一处 |
| 无头组件 | Radix UI（dropdown/dialog/tooltip） | 只取交互行为，外观完全自控 |
| 图标 | lucide-react | 现有依赖，线条风格贴近 Codex |
| 状态 | zustand | 现有依赖，团队熟悉 |
| Markdown | react-markdown + remark-gfm + shiki | 代码高亮质量优于现用的 react-syntax-highlighter |
| 路由 | react-router（仅 `/login`、`/`、`/settings`） | 三条路由，不需要 registry |
| **不引入** | antd、@agentscope-ai/*、插件/PawApp 机制、Monaco、mermaid、图表库 | 繁杂感的来源 |

新代码放在仓库 `app/` 目录（与旧 `console/` 并存），产物输出 `app/dist`。
后端通过 `QWENPAW_CONSOLE_STATIC_DIR` 指向新 dist；旧 console 保留可构建，
需要用到冷门管理功能（渠道、备份、安全策略）时切回去应急。**上游同步不受影响**（新目录零冲突）。

## 3. 后端契约（新前端唯一需要实现的东西）

### 3.1 鉴权
- `GET /api/auth/status` → `{enabled, has_users}`；`enabled=false` 直接跳过登录。
- `POST /api/auth/login` / `register` `{username,password}` → `{token}`，存 `localStorage["qwenpaw_auth_token"]`。
- 所有请求带 `Authorization: Bearer <token>`；多 Agent 时带 `X-Agent-Id`（v1 可只用默认 Agent）。
- 401 → 清 token 回 `/login`。

### 3.2 聊天（核心）
- 发送：`POST /api/console/chat`，body：
  ```json
  {
    "input": [{"role": "user", "content": [{"type": "text", "text": "..."},
              {"type": "image", "image_url": "..."}, {"type": "file", "file_url": "...", "filename": "..."}]}],
    "session_id": "console:default", "user_id": "default", "channel": "console",
    "stream": true,
    "request_context": {"approval_level": "..."}
  }
  ```
  返回 `text/event-stream`，用 `fetch` + `response.body.getReader()` 读（POST-SSE，不能用 EventSource）。
- 流帧协议（`data: <json>\n\n`，参考 `src/qwenpaw/runtime/envelope.py` 与 `src/qwenpaw/app/channels/console/channel.py:413-522`）：
  - `object:"message"`：消息信封。`type` ∈ `message | reasoning | function_call | function_call_output | plugin_call | plugin_call_output | mcp_tool_call | mcp_tool_call_output | progress | result`；`status` ∈ `created | in_progress | completed | failed | cancelled`。
  - `object:"content"`：增量块 `{type:"text", text, delta:true, msg_id, index}`，按 `msg_id` 累积。
  - `object:"response"` 且 `status:"completed"` → 本轮结束。
  - 尾帧 `{type:"turn_usage", usage, context_usage}`（上下文用量表）；`{type:"rate_limited", ...}`；`{error}`。
  - 消息 `metadata.clear_history === true` → 清空本地历史（`/clear` 指令）。
- 断线重连：刷新后若 `GET /api/chats/{id}` 返回 `status:"running"`，用 `{reconnect:true, session_id, user_id, channel}` 重新 POST 同一端点接上流。
- 停止：`POST /api/console/chat/stop?chat_id=<uuid>`。
- 首条消息会自动建会话（后端 `get_or_create_chat` + LLM 起标题），无需先 `POST /api/chats`。

### 3.3 会话
- `GET /api/chats` 列表（`ChatSpec`：id、name、status、pinned、updated_at…）。
- `GET /api/chats/{id}` → `{messages, status}`。历史→UI 分组规则参考旧代码 `console/src/pages/Chat/sessionApi/index.ts:205-310`：连续非 user 消息合并为一个助手回合。
- `PUT /api/chats/{id}`（改名/置顶）、`DELETE /api/chats/{id}`。

### 3.4 附件
- `POST /api/console/upload`（multipart `file`）→ `{url}`（存储名）。
- 预览 `GET /api/files/preview/{name}?token=<token>`（img/embed 直连需 query token）。
- 限制 `GET /api/settings/upload-limit`。

### 3.5 审批（HITL）
- 轮询 `GET /api/console/push-messages` （2.5s）→ `{messages, pending_approvals}`；按 `root_session_id` 过滤到当前会话，渲染审批卡。
- `POST /api/approval/approve|deny` `{request_id, session_id: <root_session_id>, scope?: "exact"|"similar"}`。
- 批准后原 SSE 流自动继续（服务端 Future 解除阻塞）。
- 每轮审批级别放在聊天 body 的 `request_context.approval_level`，默认值取 `GET /api/workspace/running-config`。

### 3.6 模型设置
- `GET /api/models`（provider + 模型列表）、`GET/PUT /api/models/active`（未设置时禁止发送并引导去设置——旧版行为）、`POST /api/models/{provider_id}/config`（API key / base URL）。

### 3.7 Tauri 桌面集成（从旧 `console/src/tauri/` 迁移，UI 无关逻辑直接搬）
- 保留 stdout 端口协议与 `BackendReadyGate` 流程（`backendRuntime.ts`、`useBackendReadyPolling.ts`、`desktopUpdate.ts`、`closeWindowPreference.ts` 基本可原样复用，仅重写 3 个带 antd 的小组件）。
- 需在新 UI 里重实现：外链走 `open_external_link`、关闭窗口最小化到托盘的确认弹窗、更新流程 UI、右键菜单抑制。
- v1 沿用"WebView 跳转到后端托管页"模式（改动最小）；v2 可改为 Tauri 直接打包前端、把 `127.0.0.1:<port>` 当纯 API，从而去掉 `remote.urls` 能力和 cache-buster hack。

## 4. 功能范围

**v1（MVP，先跑起来天天用）**
1. 登录门 + 无鉴权直进
2. 聊天：流式渲染、markdown、思考块（reasoning 折叠）、停止、重连、`/clear`
3. 工具卡片：一个通用折叠卡（工具名 + 参数摘要 + 结果），外加 3 个特化卡：Shell（命令+输出）、文件读写（diff 视图）、进度（progress）。其余 20+ 种先落到通用卡
4. 会话侧栏：列表/置顶/改名/删除，运行中标记
5. 审批卡 + composer 上的审批级别切换
6. 附件上传/图片预览
7. 设置页（单页）：模型与 provider、主题、语言（zh/en）
8. 上下文用量指示（turn_usage 尾帧）

**v2（用起来后按需加）**
- 定时任务精简版（`/api/cron/jobs`，办公场景刚需）
- 多 Agent 切换（`X-Agent-Id`）
- 推送消息收件箱、语音输入（Whisper）、MCP 管理精简版
- Tauri 直连 API 模式

**明确不做**：Coding 三栏 IDE、IM 渠道管理、技能市场、插件/PawApp、备份 UI、安全策略 UI、用量图表。冷门需求切回旧 console。

## 5. 实施阶段与分工

> 分工原则：Claude（本人）负责架构、接口设计、视觉品味、验收；Codex 负责具体执行与 bug review。
> 每阶段 Codex 交付后，先自查验收标准，再由 Claude review 品味与架构一致性。

### Phase 0：契约与骨架（Claude 主做）
- 产出 `app/` 脚手架：Vite + TS + Tailwind + 目录结构 + design tokens（色板/间距/字号，浅深双主题）
- 产出 `app/docs/api-contract.md`（即本文 §3 的细化版，含真实 SSE 抓包样本作为 fixture）
- 布局骨架：侧栏 + 主区 + composer 的静态版
- **验收**：`npm run dev` 起来能看到空壳三栏布局；`QWENPAW_CONSOLE_STATIC_DIR=app/dist qwenpaw app` 能被后端托管。

### Phase 1：聊天核心（Codex 执行）
- SSE 解析器（纯函数模块 `app/src/lib/stream.ts`，用 Phase 0 的抓包 fixture 写单测——这是 Codex 的强项）
- 消息 store（zustand）+ 渲染器（气泡、markdown、reasoning 折叠、通用工具卡）
- composer：发送、停止、Enter/Shift+Enter、粘贴图片
- 会话列表 + 历史加载（含分组规则、`status:"running"` 重连）
- **验收**：与真实后端对话完整流畅；刷新页面后进行中的回复能接上；`/clear` 生效；fixture 单测全绿。

### Phase 2：审批 + 附件 + 设置（Codex 执行）
- 轮询服务 + 审批卡（approve/deny/scope）+ 审批级别切换
- 上传、图片预览、大小限制
- 设置页：模型 active 槽、provider key 配置、主题/语言
- **验收**：触发一次真实工具审批走通 approve 与 deny 两条路；未配置模型时发送被禁并有引导。

### Phase 3：Tauri 接入 + Windows 打包（Codex 执行，Claude 定方案）
- 迁移 `src/tauri/` 胶水层，重写 3 个 UI 组件（loading 页、关闭确认、更新提示）
- 打包脚本改指新 dist（`scripts/pack-tauri/qwenpaw.spec` 的 `CONSOLE_DIST` 指向）
- 跑通 `build_win_pyinstaller.ps1`，Windows 上安装验证
- **验收**：Windows NSIS 安装包可安装启动，托盘/关闭/更新流程正常。

### Phase 4：打磨（Claude 定清单，Codex 执行）
- 键盘快捷键（Cmd/Ctrl+N 新会话、Cmd/Ctrl+K 会话搜索）
- 空态欢迎语、错误态、rate_limited 提示
- 特化工具卡补充（按实际使用频率）

## 6. 风险与对策

| 风险 | 对策 |
|---|---|
| 自研渲染器要覆盖 SSE 协议边角（clear_history、rate_limited、重连、乱序） | Phase 0 先录制真实 SSE 转写作为 fixture；解析器做成纯函数 + 单测；旧代码 `sessionApi/index.ts`、`turnUsage.ts` 作为参考实现 |
| 丢掉厂商包后气泡/composer 细节工作量 | v1 只做文本+图片+文件三种内容块；audio/video 后置 |
| 上游 fork 同步冲突 | 新代码全部在 `app/`，后端零改动，旧 console 不删 |
| 旧 console 冷门功能偶尔要用 | 环境变量一键切回旧 dist |

## 7. 给 Codex 的工作方式约定

- 每个 Phase 是一个独立任务包：拿到本文档 + 对应阶段的验收标准 + 相关旧代码路径作为参考。
- Codex 不做的事：改后端、改视觉规范（token 文件只有 Claude 改）、引入新依赖需先报备。
- 每阶段交付物：可运行代码 + 单测 + 一段自查报告（对照验收标准逐条说明）。
- Review 顺序：Codex 自查 → Codex review bug（边界、竞态、泄漏）→ Claude review 架构与品味。
