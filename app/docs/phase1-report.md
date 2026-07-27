# Phase 1 自查报告：聊天核心

日期：2026-07-27

## 结论

Phase 1 任务包已完成。SSE 解析与会话 reducer、zustand 消息 store、消息与工具渲染、
Composer、会话侧栏、登录门均已接入真实 API。fixture 单测全绿，TypeScript 与 Vite
生产构建通过，并已使用本机桌面版后端完成真实 POST-SSE、停止、刷新重连和历史恢复联调。

本阶段只修改了 `app/`，未修改 `tokens.css`，未新增依赖。

## 验收项

### 1. SSE 解析器

- `src/lib/stream.ts` 保持纯函数：
  - `parseSseChunk` 处理 `data:`、空行分帧、半帧缓存和 CRLF。
  - `parseSseBytes` 处理任意 `Uint8Array` 边界，并保留被切断的 UTF-8 尾字节。
  - parser 不访问 fetch、store 或浏览器状态。
- reducer 覆盖：
  - response 的 created / in_progress / completed / failed 生命周期。
  - message 开卡、completed 终值校正以及 response.output 兜底校正。
  - 文本 delta 累积、`delta:false` 终值替换。
  - reasoning 文本流。
  - 工具 arguments 增量拼接；工具 output 全量替换。
  - `clear_history`、`turn_usage`、`rate_limited`、顶层 error。
  - `sequence_number` 重复与乱序丢弃。
- 真实 fixture 差异已按样本处理：
  - `tool-call.sse.txt` 中工具调用/结果的 `delta:false` 终值帧可能
    `msg_id:null`。实时内容以带 `msg_id` 的帧更新，最终以 completed message
    校正归属；没有修改协议类型。
  - response.output 会再次携带已经处理过的 clear message，reducer 对空历史清理保持幂等。

### 2. 消息 store

- `src/stores/chat.ts` 统一持有：
  - 当前 ChatSpec / session identity。
  - 当前会话消息和 `ConversationStreamState`。
  - 流式、历史加载、模型加载和错误状态。
  - 会话列表、活动模型、审批级别和本地待上传图片。
- SSE reader 只把字节交给纯 parser/reducer。
- 新会话使用乐观 user message；后端创建 ChatSpec 后切换到真实 UUID URL。
- 发送中的临时 session id 存入 `sessionStorage`，覆盖“点击发送后立刻刷新、真实 UUID
  尚未返回”的恢复窗口。
- 重连流结束后再次读取权威历史，覆盖“history 返回 running 后、attach 前恰好完成”
  导致空重连流的竞态。
- 历史消息 metadata 中的 `qwenpaw_turn_usage` 会恢复到上下文用量状态。

### 3. 渲染器

- 用户消息为右侧中性气泡；助手消息为无边框正文流。
- Markdown 使用 `react-markdown + remark-gfm`，覆盖标题、列表、引用、表格、链接和代码。
- 代码块出现时才动态 import Shiki。Shiki 负责语法分词，词法类别映射到既有语义色类，
  没有采用 Shiki 的内联硬编码颜色。
- reasoning 默认折叠，流式时显示“思考中”状态动效。
- `plugin/function/mcp` call 与 output 均按 `data.call_id` 配对；默认压成单行工具卡。
- `execute_shell_command` 使用命令行与输出分区的终端样式特化卡。
- 历史按 user 开新回合，连续非 user 消息归入一个助手回合。

### 4. Composer

- `fetch` POST-SSE 发送；每轮携带 `request_context.approval_level`。
- 审批默认值来自 `/api/workspace/running-config`，可在 Composer 菜单切换。
- 生成中发送按钮替换为停止按钮，调用 `/api/console/chat/stop?chat_id=...`。
- Enter 发送，Shift+Enter 换行，输入法 composing 时不误发。
- 粘贴/选择图片后使用 object URL 暂存在内存并显示缩略图；可单张移除。
- `/api/models/active` 的 `active_llm` 为空时发送按钮禁用，并显示设置入口。

### 5. 会话侧栏

- `/api/chats?archived=false` 加载并按 pinned、updated_at 排序。
- 支持切换历史、置顶/取消置顶、改名、删除。
- Radix DropdownMenu 支持悬浮操作按钮；会话行右键也可打开同一菜单。
- running 会话显示脉冲状态点。
- 历史接口返回 `status:"running"`，或瞬时刷新恢复标记仍存在时，自动 POST reconnect。

### 6. 登录门

- 启动先请求 `/api/auth/status`。
- `enabled:false` 直接进入主界面。
- `enabled:true` 且无 token 时进入 `/login`；`has_users:false` 使用注册接口，否则登录。
- token 写入 `localStorage["qwenpaw_auth_token"]`。
- `api.ts` 统一注入 Bearer token；401 清 token 并跳转登录页。

## 自动化验证

在 `app/` 执行：

```text
npm test -- --reporter=verbose
Test Files  1 passed (1)
Tests       10 passed (10)

npm run build
tsc -b      passed
vite build  passed
```

单测覆盖：

- `simple-text.sse.txt` 最终文本和 turn_usage。
- `tool-call.sse.txt` 参数拼接、结果替换和总结文本。
- JSON 半帧。
- fixture 按确定性随机的 1–17 字节边界重新切分（包含中文 UTF-8 边界）。
- failed response、顶层 error、clear_history、rate_limited、reasoning、乱序/重复 seq。
- response.output 最终校正时 clear_history 幂等。

## 真实后端联调

后端：本机 QwenPaw Desktop，`http://127.0.0.1:53511`。

- 鉴权状态：`enabled:false`，无鉴权直进通过。鉴权启用时的登录/注册页本次没有可用的
  第二后端实例做真实提交测试。
- 活动模型：`gpt-5.6-sol`。
- 纯文本 POST-SSE：通过；文本增量、completed、turn_usage（UI 显示 0.8%）均到达。
- 自动建会话和 LLM 标题更新：通过。
- 刷新后的完整历史恢复：通过。
- 运行中刷新重连：通过；刷新后重新出现停止按钮并继续接收文本。
- 点击发送后立即刷新：通过；临时 session 能恢复到真实 `/chat/<uuid>`。
- 停止按钮：通过；长回复在部分输出后停止，侧栏回到 idle。
- 长历史视觉检查：Markdown、普通工具卡、Shell 卡均正常；工具卡默认折叠。
- 浏览器控制台：无应用运行错误；只有 React Router v7 future flag 预告警告。
- 所有联调创建的会话均已 DELETE；最终 `/api/chats` 中无遗留 running 会话。

## 约束检查

- 未修改 `app/` 之外文件。
- 未修改 `app/src/styles/tokens.css`。
- 组件源码未出现硬编码 hex/rgb 颜色或颜色 inline style。
- 未新增 npm 依赖。

## 已知限制 / Phase 2 衔接

1. 图片目前仅本地暂存和预览，不调用上传 API、不随消息发送；刷新会丢失，符合 Phase 1
   “待 Phase 2 上传”的范围。
2. 设置页仍是 Phase 0 骨架。未配置活动模型时会正确禁止发送并引导至设置，但真正的
   provider/model 配置表单属于 Phase 2。
3. 本阶段未实现审批卡、附件上传、文件/音视频内容渲染和文件 diff 特化卡，按计划留给
   Phase 2 或后续阶段。
4. Shiki 已懒加载，不影响首屏主 bundle；但 Vite 会为完整 Shiki 语言集合产出较多懒加载
   chunk，并给出个别 chunk 超过 500 kB 的构建警告。后续可按常用语言改为精简 highlighter
   bundle，无需新增依赖。
5. 本机后端鉴权关闭，因此已验证无鉴权直进和代码级 401/token 流程，未实际提交一次
   auth-enabled 登录。
