# Phase 4 自查报告：打磨

日期：2026-07-27

## 结论

Phase 4 清单已完成：全局新建/搜索快捷键、会话搜索 Dialog、空态副提示、统一 Banner、
限流免费模型切换、文件工具特化卡、行级 diff、progress 卡以及指定视觉修正均已落地。

本阶段只修改 `app/`，未修改 `src/styles/tokens.css`，未新增依赖。`npm test` 25 项全绿，
TypeScript 与 Vite 生产构建通过，最大入口 chunk 为 492.93 kB，无 chunk size warning。

## 1. 键盘快捷键

- `Cmd/Ctrl+N`
  - 在 `AppShell` 注册全局键盘监听，输入框聚焦时同样生效。
  - 调用与侧栏按钮相同的 `newChat()`，随后导航至 `/`。
  - macOS 只响应 `Meta`，其他平台只响应 `Ctrl`，不拦截额外带 Shift/Alt 的组合键。
- `Cmd/Ctrl+K`
  - 打开基于 `@radix-ui/react-dialog` 的会话搜索弹层。
  - 输入按会话名称即时、大小写不敏感过滤；空查询显示完整列表。
  - 过滤保持 store 中的侧栏顺序，即 pinned 优先、其余按 `updated_at` 倒序。
  - ↑/↓ 循环选择、Enter 打开、Esc 由 Dialog 关闭；选中项自动滚入视口。
- 平台显示
  - macOS 显示 `⌘N / ⌘K`，其他平台显示 `Ctrl+N / Ctrl+K`。
  - 侧栏操作入口与空态提示共用同一平台判断。
- 搜索弹层使用路由级按需 chunk，避免抬高主入口体积。

## 2. 空态与错误态

- 空会话欢迎语下新增 muted、`text-sm` 副提示：
  - zh：`输入任务开始，或用 {shortcut} 检索历史会话`
  - en：`Enter a task to begin, or use {shortcut} to search past chats`
- 新增统一 `Banner` 组件：
  - 普通顶部错误使用 danger 语气。
  - `rate_limited` 使用 warn 语气，并直接显示后端 `error` 文本。
  - 两种语气共用图标、布局、关闭行为与 action 区。
- 按后端 `_get_free_model_alternatives` 的真实结构补全类型：
  - `provider_id`
  - `provider_name`
  - `model_id`
  - `model_name`
- 每个免费候选显示“切换到 `<model_name>`”按钮。点击后：
  1. `PUT /api/models/active`
  2. `GET /api/models/active` 刷新 store 中的 `activeModel`
  3. 成功后清除 rate-limit Banner
- 切换失败时收起旧限流提示并以 danger Banner 显示真实请求错误。
- `clearError()` 同时清理 store error、stream error 和 rate-limit 状态，避免残留横幅。

## 3. 特化卡

### 文件工具

已核对：

- `src/qwenpaw/governance/tool_registry.py`
- `src/qwenpaw/agents/tools/file_io.py`

真实工具名与参数：

| 工具名 | 参数 |
|---|---|
| `read_file` | `file_path`, `start_line?`, `end_line?` |
| `write_file` | `file_path`, `content` |
| `edit_file` | `file_path`, `old_text`, `new_text` |
| `append_file` | `file_path`, `content`（后端默认关闭，但沿用同类卡） |

实现结果：

- 文件工具统一显示文件图标、操作名、路径和状态。
- 展开后固定显示路径行与内容区：
  - read 展示工具输出；
  - write/append 展示待写内容；
  - edit 展示 `old_text → new_text` 行级 diff。
- diff 使用自实现 LCS 行比较，不引入依赖：
  - 相同行：中性灰语义色；
  - 删除行：`danger-soft / danger`；
  - 新增行：`ok` 语义色调。
- 未匹配的工具仍进入原通用 `ToolCard`，Shell 特化卡保持不变。

### progress 消息

- `type:"progress"` 在普通消息分支前识别并交给 `ProgressCard`。
- 运行中显示轻量卡片：标题/工具名、状态文本和 pulse 状态图标。
- completed/failed/cancelled 后收敛为一行，分别使用完成或失败语义状态。
- 标题兼容 `message.name` 与 metadata 的 `title/tool_name/name`；状态兼容 text content
  与 data content 的 `status/message/detail`。

## 4. 视觉修正

1. 对话列表使用 `pt-6 pb-8`，首条消息顶部与底部均保留呼吸空间。
2. Composer 外框由 `rounded-lg` 改为 `rounded-xl`。
3. 设置页 ChoiceGroup 改为等宽 grid：
   - 外观三项实测宽度 `142 / 142 / 142 px`。
   - 语言两项实测宽度 `217 / 217 px`。
4. 侧栏会话行：
   - 保留 `transition-colors` 和 hover 底色。
   - running pulse 与 pinned 图标可同时显示且均 `shrink-0`。
   - 名称区域为 `min-w-0 flex-1 truncate`，操作按钮不会挤坏长名称。
5. i18n 与颜色检查：
   - 新增文案均同时加入 zh/en 字典。
   - 字典 key 完整性测试继续通过。
   - 既有 stream fallback 的硬编码中文已迁移到 `stream.requestFailed`。
   - 组件新增颜色全部使用 token 暴露的语义类，无新增 hex/rgb/inline color。

## 5. 自动化验证

在 `app/` 执行：

```text
npm test
Test Files  8 passed (8)
Tests       25 passed (25)

npm run build
tsc -b      passed
vite build  passed
largest JS  492.93 kB
Vite warnings  none
```

Phase 4 新增测试：

- `lineDiff.test.ts`
  - 替换拆分为 remove/add；
  - 公共行保留；
  - 纯新增、纯删除与空输入。
- `chats.test.ts`
  - pinned 优先、更新时间倒序；
  - 搜索 trim、大小写不敏感；
  - 过滤后保持侧栏排序。
- `chatBanner.test.ts`
  - rate-limit warn 分支及后端原文；
  - ordinary error danger 分支；
  - clean null 分支。
- `shortcuts.test.ts`
  - macOS Command 显示与响应；
  - 非 macOS Control 显示与响应。

Phase 1/2 的 SSE、审批、附件和 i18n 测试继续全绿。

## 6. 真实联调冒烟

环境：

- dev server：`http://127.0.0.1:5174`
- 本地桌面后端：dev proxy
- 鉴权：关闭
- 活动模型：`gpt-5.6-sol`
- 平台：macOS

通过项：

- Composer 聚焦时 `⌘K` 打开搜索弹层。
- 输入“模型”即时只保留匹配会话；ArrowDown + Enter 打开目标会话。
- Esc 关闭搜索弹层。
- Composer 聚焦时 `⌘N` 返回空会话，欢迎副提示显示 `⌘K`。
- 创建真实联调会话，要求模型调用 `read_file(".bootstrap_completed")`：
  - SSE 中实际收到 `name:"read_file"` 与真实参数；
  - UI 显示“读取文件”特化卡；
  - 展开后显示文件路径和内容区；
  - 流完成后卡片状态为“完成”。
- 设置页外观/语言按钮实测等宽。
- 浏览器控制台 error 为 0。

未强行触发：

- 当前后端没有自然出现 `rate_limited`，未通过耗尽额度或修改用户模型配置制造限流。
  该分支已按真实后端结构接线并由 reducer/Banner 单测覆盖。
- 本次真实流没有 `progress` 样本，因此 progress 卡完成了协议兼容实现和构建检查，
  未记录真实后端截图。

## 7. 联调清理

- 联调会话：`e15afd84-94ce-45c3-a269-1cce8e5ee912`
- 删除前确认历史 `status:"idle"`，包含完整 `read_file` call/output 与助手终值。
- `DELETE /api/chats/{id}` 返回 `{"deleted":true}`。
- 删除后会话列表中已无该 id。
