# Phase 1 任务包：聊天核心（执行者：Codex）

你在 QwenPaw 仓库中执行前端重构的 Phase 1。总体方案见仓库根 `REFACTOR_PLAN.md`（先读 §1–§4 与 Phase 1 一节），后端契约见 `app/docs/api-contract.md`（必读，字段已对照后端源码核实）。Phase 0 已完成：`app/` 下有可构建的脚手架（Vite + React 18 + TS + Tailwind v4）、design tokens、布局骨架、协议类型 `app/src/lib/protocol/types.ts`、真实抓包 fixtures（`app/fixtures/`）。

## 交付物

1. **SSE 解析器 `app/src/lib/stream.ts`**（纯函数，不碰 fetch/store）：
   - 增量字节流 → 帧解析器（处理 `data: ` 前缀、`\n\n` 分帧、半帧被 TCP 切断的情况）；
   - 帧 → 会话状态的 reducer（按契约 §2.2：文本增量累积、终值替换、工具参数拼接、
     工具结果全量替换、reasoning、message 开卡/收卡、response 生命周期、
     `clear_history`、`turn_usage`、`rate_limited`、error、sequence_number 乱序防御）。
   - **单测**（vitest，`npm test`）：以 `app/fixtures/sse/*.sse.txt` 为输入逐帧喂入断言最终状态；
     另覆盖半帧切断、任意字节边界切分（用 fixture 内容按随机边界重切）、failed response、
     `clear_history`、`rate_limited`。
2. **消息 store**（zustand，`app/src/stores/`）：当前会话消息、流式状态、会话列表。
3. **渲染器**（`app/src/components/chat/`）：
   - 文本气泡（用户/助手）、markdown（react-markdown + remark-gfm，代码高亮 shiki 懒加载）；
   - reasoning 折叠块（默认折叠，流式时显示"思考中"动效，可展开）；
   - 通用工具卡：一行折叠卡（工具名 + 参数摘要 + 状态），点开看完整参数与结果；
     `plugin_call`/`plugin_call_output` 按 `data.call_id` 配对成一张卡；
   - Shell 特化卡（`execute_shell_command`）：命令 + 输出的终端样式。
4. **Composer 接入**（改造 `app/src/components/chat/Composer.tsx`）：
   发送（POST-SSE，见契约 §2.1）、停止按钮（流式中变为停止）、Enter 发送 /
   Shift+Enter 换行、粘贴图片（先存本地待 Phase 2 上传，UI 上显示缩略图占位即可）。
   未配置活动模型（`GET /api/models/active` 的 `active_llm` 为空）时禁止发送并提示去设置。
5. **会话侧栏接入**（改造 `Sidebar.tsx`）：`GET /api/chats` 列表、置顶/改名/删除
   （Radix DropdownMenu 右键/悬浮菜单）、运行中标记；切换会话加载历史
   （分组规则见契约 §3）；`status:"running"` 时按契约 §2.4 重连接流。
6. **登录门**：`GET /api/auth/status`，`enabled=false` 直接进主界面；否则 `/login`
   表单登录，token 注入与 401 处理放 `app/src/lib/api.ts`。
7. **自查报告 `app/docs/phase1-report.md`**：对照 REFACTOR_PLAN.md Phase 1 验收标准逐条说明，
   附单测结果与已知限制。

## 硬约束

- 不改 `app/` 之外的任何文件（后端、旧 console、Tauri 一律不动）。
- 不改 `app/src/styles/tokens.css`（design token 只有架构方改；需要新 token 就在报告里提出）。
- 样式只用 token 暴露的语义类（`bg-surface`、`text-ink`、`border-line`、`bg-accent`…），禁止硬编码色值、禁止内联 style 写颜色。
- 不引入新依赖（package.json 里已备齐：zustand、react-markdown、remark-gfm、shiki、Radix、lucide-react）。确需新增时在报告中说明理由，先不装。
- 协议实现以 `app/src/lib/protocol/types.ts` + `app/docs/api-contract.md` 为准；发现与真实 fixture 不符时，以 fixture 为准并在报告中记录差异。
- 视觉对标 Codex Desktop / ChatGPT 桌面版的克制风格：留白、无边框噪音、动效只用于状态反馈。拿不准的布局细节，参考现有骨架的间距与层级，不要自由发挥。

## 验证方式

- `cd app && npm test` 全绿；`npm run build` 通过。
- 联调：`npm run dev` 后浏览器访问 http://localhost:5174 。本机桌面版 QwenPaw 后端在运行
  （端口随机，`lsof -nP -iTCP -sTCP:LISTEN | grep qwenpaw` 可查），
  启动 dev server 时用 `QWENPAW_DEV_BACKEND=http://127.0.0.1:<port> npm run dev` 指向它。
  联调产生的测试会话请用 `DELETE /api/chats/{id}` 清理。
- 发消息可能触发工具审批而挂起：联调消息请像这样在 body 里带
  `"request_context": {"approval_level": "OFF"}`，或只发不需要工具的消息。
