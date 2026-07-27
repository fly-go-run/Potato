# Phase 2 自查报告：审批、附件与设置

日期：2026-07-27

## 结论

Phase 2 的审批轮询与内联卡片、附件上传/渲染、模型与 Provider 设置、主题、轻量 i18n、
Shiki 精简加载和 React Router URL 替换均已实现。

`npm test` 与 `npm run build` 通过，生产构建不再出现 500 kB chunk 警告。真实桌面后端上
已完整走通 STRICT 审批的 approve 和 deny 两条路径，并验证原 SSE 流继续、创建会话后的
URL replace 不打断流。主题、语言和活动模型保存已通过 UI 人工检查。

真实图片选择被当前浏览器自动化环境的文件传输安全策略阻止，策略阻止前未发生上传；
因此“真实上传并刷新历史”的自动化联调未完成。附件限额、上传失败不发送、content 组装
与历史渲染已完成代码审查和单测，具体见“已知限制”。

本阶段只修改 `app/`，未修改 `tokens.css`，未新增依赖。

## Phase 2 验收项

### 1. 审批（HITL）

- `ChatView` 仅在 `isStreaming` 且当前 `sessionId` 有效时启动轮询：
  - 进入流立即请求一次，此后每 2.5 秒请求
    `/api/console/push-messages?session_id=...`。
  - 用 in-flight 守卫避免慢请求重叠。
  - 切会话、流结束或组件卸载时 abort 请求并清除 interval。
  - 空闲会话不轮询。
- `filterApprovalsForSession` 只按
  `approval.root_session_id === currentSessionId` 过滤，未误用子 session id。
- 审批卡位于消息回合之后，包含：
  - display tool name、精简参数摘要、完整 JSON 参数折叠区。
  - severity 语义色 badge、findings、执行目标与规则来源。
  - approve exact、deny；仅在后端 `is_generalized:true` 时显示 approve similar，
    与 `approval_display_fields` 的约定一致。
- action body 使用 `request_id`、`root_session_id` 和小写 scope。
- 404 视为超时/已处理并静默移除；403 等其他错误进入全局错误提示。
- action 成功只移除审批卡，不重连 SSE。

真实联调：

- 后端：QwenPaw Desktop `127.0.0.1:53511`，前端 dev server `127.0.0.1:5175`。
- Composer 切换为 STRICT，发送强制执行 `pwd` 的请求：
  - 审批卡显示 `Bash`、`command: pwd`、STRICT rule。
  - 点击批准后原流继续，shell 返回
    `/Users/liuxu/.qwenpaw/workspaces/default`，agent 完成回复。
- 同一会话再次请求执行 `whoami`：
  - 点击拒绝后原流继续。
  - tool output state 为 `denied`，agent 明确回复命令因用户拒绝而未执行。
- 新会话从临时 session 创建真实 chat 后 URL 以 router `navigate(..., {replace:true})`
  更新；URL 更新期间 SSE 未中断，也没有触发 reconnect。
- 本次 STRICT 样本 `is_generalized:false`，所以按契约未展示“批准同类”；该 scope 的
  请求路径已实现，但本次没有可用的 generalized 审批做真实点击。

### 2. 附件

- Composer 文件选择器已放开类型限制；粘贴仍只接收剪贴板图片。
- pending attachment 同时支持：
  - 图片 object URL 缩略图。
  - 普通文件 filename chip。
- 发送前先读取 `/api/settings/upload-limit`；当前真实后端返回
  `{"upload_max_size_mb": null}`。
- 在创建乐观 user message 前逐个 multipart 上传到 `/api/console/upload`。
- 任一限额检查或上传失败时：
  - 不创建本地消息、不调用 chat SSE。
  - 保留 pending attachments。
  - Composer 恢复原输入，并显示包含文件名的错误。
- content 组装为 text + 按选择顺序排列的 image/file blocks；普通文件字段使用契约规定的
  `filename`。
- 本地乐观消息与历史消息共用 `MessageContent`：
  - 图片通过 `/api/files/preview/{path}` 渲染。
  - 有 auth token 时只在预览 URL query 中追加 token。
  - 文件渲染为可打开的 filename chip。
  - HTTP(S) 媒体 URL 保持原 URL。
- 切会话发生在上传过程中时，上传完成后会检查 submitting state，不会把旧选择发送到新会话。

自动化浏览器选择本地 `website/public/logo.png` 时被浏览器文件传输安全策略拒绝；
未绕过策略使用脚本或其他传输通道，因此本阶段没有真实上传/刷新证据。

### 3. 设置页

- 单页四分区设置已替换原骨架：
  - 模型：读取 Provider 列表，合并/去重 `models` 与 `extra_models`，选择并设置 global
    active model。
  - Provider 连接：API key 留空时保留现值，支持 base URL；`freeze_url` 时禁用地址输入。
  - 外观：浅色、深色、跟随系统，复用 `theme.ts`。
  - 语言：中文、English。
- 活动模型保存后调用 store `loadActiveModel()`；真实 UI 保存当前
  `sub2api / gpt-5.6-sol` 后显示成功提示，Composer 使用同一 store。
- 后端当前源码把 Provider config 实现为 `PUT /api/models/{id}/config`，任务契约写
  `POST`。前端先按契约 POST；收到当前后端的 405 时兼容回退 PUT。
- i18n：
  - `src/lib/i18n.ts` 提供 zh/en 两份 dict、`t(key)`、参数插值、localStorage 持久化和
    zustand 触发重渲染。
  - 现有 TSX 组件中的硬编码中文 UI 文案均已迁移。
  - dict 完整性单测验证两种语言 key 集合完全一致。
- 真实 UI 检查：
  - 深色主题即时加上根节点 `.dark`，随后恢复“跟随系统”。
  - 切到 English 后侧栏、设置页、字段、按钮与说明即时变为英文，随后恢复中文。
  - 设置页在 1536×898 视口下检查过布局；浏览器 console error 为 0。
- 当前后端已有活动模型，未破坏用户配置去制造“未配置”状态；未配置时 Composer 禁用与
  设置引导沿用 Phase 1 逻辑，本阶段错误文案已纳入 i18n。

### 4. Backlog 清理

- Shiki 改为 `shiki/core` + `shiki/engine/javascript`：
  - 代码块出现且语言在白名单内才初始化 highlighter。
  - ts/js/tsx/json/bash/python/html/css/md/yaml/sql/go/rust/java/c/diff 各自动态加载。
  - cpp 使用按需注册的精简 TextMate grammar，覆盖注释、字符串、关键字、基本类型、数字、
    预处理指令和函数名，避免上游 cpp grammar 连带嵌入多份大 grammar。
  - 未知语言直接渲染普通 `pre`。
- 最大生产 chunk 为 492.12 kB，构建无 chunk size warning；语言 chunk 最大约 209 kB。
- `sendMessage` 不再调用 `window.history.replaceState`，改为传入 React Router
  `navigate(path, {replace:true})`。

## 自动化验证

在 `app/` 执行：

```text
npm test
Test Files  4 passed (4)
Tests       17 passed (17)

npm run build
tsc -b      passed
vite build  passed
largest JS  492.12 kB
warnings    none
```

新增单测覆盖：

- 审批只按 `root_session_id` 过滤，空 session 返回空列表。
- text + image + file 的 outbound content 顺序与字段。
- per-file 上传限额和 unlimited。
- zh/en dict key 完整性与参数插值。

Phase 1 的 10 个 SSE parser/reducer 测试继续全绿。

## 联调清理

- 联调会话：
  `1b2d7f7c-5015-482f-a034-a040ee02413b`。
- 删除前读取权威历史确认 status 为 `idle`，approve 和 deny 的 tool output 均已落历史。
- `DELETE /api/chats/{id}` 返回 `{"deleted":true}`。
- 删除后 `/api/chats?archived=false` 仅剩联调前已有的 3 个 idle 会话。
- `/api/console/push-messages` 最终返回空 `pending_approvals`。
- 本阶段启动的 5175 Vite dev server 已停止。

## 约束检查

- 只修改 `app/`。
- 未修改 `app/src/styles/tokens.css`。
- 新增/修改组件未出现 hex/rgb 硬编码颜色或颜色 inline style。
- 未新增 npm 依赖。

## 已知限制

1. 当前浏览器自动化安全策略阻止本地文件传输，真实图片上传与刷新历史未能在本次会话执行；
   需要人工在 Composer 选择一张图片后补验。策略阻止前没有上传文件，也没有产生待清理的
   图片测试 chat。
2. 当前后端已有活动模型，本次未清空用户模型配置，因此“无活动模型”的禁用/引导只做了
   代码检查，没有在真实后端上制造该状态。
3. 当前后端 Provider config 的 HTTP method 与任务契约不一致；前端保留 POST → 405 时
   fallback PUT 的兼容逻辑，待契约或后端统一后可去掉 fallback。
4. cpp 为控制 bundle 使用精简 grammar，不覆盖完整 C++ TextMate 语法的所有边角。
