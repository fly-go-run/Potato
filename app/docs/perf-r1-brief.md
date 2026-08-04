# 性能优化第一轮：流式渲染链路（perf-r1）

## 背景

性能评估确认：SSE 每帧单独写 Zustand + 多个核心组件整店订阅，导致流式回复期间每个 token 都触发侧栏、输入框、消息列表全量重渲染；代码块每次文本变化重跑完整 Shiki 高亮。本轮只做流式链路，**不做**拆包、不做已完成回合 memo（那是第二轮）。

仅改 `app/` 前端，后端零改动。不改任何视觉样式、design tokens、文案、i18n。

## 任务

### T1 SSE 合帧提交（app/src/stores/chat.ts 的 `consumeResponse`，约 721 行）

现状：`while` 循环里每个解析出的 frame 都调用一次 `set({ stream: next, error: next.error })`。

改为本地累积 + 节流提交：

- 维护局部 `pending: ConversationStreamState`，初始为 `get().stream`；每个 frame 做 `pending = reduceStreamFrame(pending, frame)`（可复用 `stream.ts` 已导出的 `reduceStreamFrames`）。
- 提交策略：
  - **立即 flush** 的情形：frame 使 `responseStatus` 进入终态（completed/failed/cancelled）、产生 `error`、产生 `rateLimited`。判断可以简化为「reduce 前后 responseStatus / error / rateLimited 任一发生变化就立即 flush」。
  - 其余 frame 节流提交，间隔约 **40ms**（33～50ms 均可）。
  - 注意尾帧不能滞留：一批网络 chunk 处理完后若有未提交的 pending，需用 setTimeout 安排一次 trailing flush（下一批到来或立即 flush 发生时取消/合并），保证任何 frame 的效果延迟不超过约 50ms。
  - 读取循环结束后（`done` 或 parser error 抛出前）做最终 flush。
- flush 即 `set({ stream: pending, error: pending.error })`，与现状语义一致。
- **关键约束**：`stop()` 会 abort controller 并把 `stream.responseStatus` 写成 `cancelled`。abort 之后（`controller.signal.aborted` 为 true）**不得再 flush**，否则会覆盖 cancelled 状态。现有循环里的 `if (controller.signal.aborted) break` 语义保留。
- 清理好 timer，避免组件卸载/请求结束后仍触发 set。

### T2 立即消费 SSE，不等会话列表（app/src/stores/chat.ts sendMessage，约 402 行）

现状：拿到 SSE `response` 后先 `await get().refreshChats()` 查新会话 ID 并导航，然后才 `consumeResponse`。首字被一次 `/api/chats` 挡住。

改为并发：

```ts
const navigationDone = get()
  .refreshChats()
  .then((chats) => {
    const created = chats.find((chat) => chat.session_id === sessionId);
    if (created && get().sessionId === sessionId) {
      set({ activeChatId: created.id });
      navigate(`/chat/${created.id}`, { replace: true });
    }
  })
  .catch(() => {}); // 导航失败不影响流；catch 分支的兜底逻辑已存在
await consumeResponse(response, controller, set, get);
await navigationDone;
```

- catch 分支里现有的 `knownChat` 查找逻辑保持不变。
- `finally` 里最后一次 `refreshChats()` 保留（它负责把会话状态刷成终态）。

### T3 字段级订阅（三个组件）

把无 selector 的 `useChatStore()` 全部改成字段级 selector。Zustand v5 写法：每个字段一行 `useChatStore((s) => s.xxx)`；store 里的 action 引用是稳定的，单独 select 不会引起额外渲染。

- `app/src/views/ChatView.tsx` 约 258 行：解构的十余个字段逐个改 selector。
- `app/src/components/chat/Composer.tsx` 约 71 行：同上。注意 Composer 当前解构了整个 `stream`——检查它实际用到 `stream` 的哪些子字段（如 `stream.turnUsage`、`stream.responseStatus` 等），只订阅用到的子字段，避免每个 token 重渲染输入框。
- `app/src/components/layout/Sidebar.tsx` 约 51 行：只订阅 `chats`、`chatsLoading`、`activeChatId`、`newChat`。**Sidebar 不得订阅 `stream` 或 `pendingApprovals`。**

不要用 `useShallow` 包对象整取，逐字段订阅即可。

### T4 审批轮询无变化不写 store（app/src/stores/chat.ts pollApprovals，约 647 行）

`set({ pendingApprovals: ... })` 之前先与当前 `get().pendingApprovals` 比较：长度相同且逐项 `request_id` 相同（顺序一致）则跳过 set。每 2.5s 轮询一次、多数时候无审批，没必要每次生成新数组触发订阅者。

### T5 自动滚动合并到 rAF（app/src/views/ChatView.tsx 约 363 行）

现状 effect 依赖 `[pendingApprovals, stream.messages]`，每帧同步读写 `scrollHeight/scrollTop`。改为：effect 里用 `requestAnimationFrame` 调度滚动写入，调度前取消上一个未执行的 rAF（用 ref 存 id），卸载时清理。判断 `atBottomRef` 的逻辑不变。

### T6 流式期间延迟代码高亮（app/src/components/chat/Markdown.tsx `HighlightedCode`，约 108 行）

现状：effect 依赖 `[code, language]`，流式中代码每变一个字符就重跑一次完整 Shiki 高亮。

改为防抖：

- `code` 变化时先把 `lines` 置回 `null`（走现有纯文本渲染分支，避免显示与新文本不符的旧高亮），然后 **200ms 防抖**后再调用 `highlightCode`。
- 消息流式结束后代码不再变化，200ms 后自然完成高亮，无需感知 streaming 状态。
- 首次挂载（历史消息、非流式场景）也走同一路径即可，200ms 延迟可接受；如果想优化，可以「首次挂载立即高亮、后续变化才防抖」，任选一种，保持实现简单。
- 清理 timer 与现有 `active` flag 语义合并，防止卸载后 setState。

## 验收标准

在 `app/` 目录下全部通过：

```
npx tsc --noEmit
npm test          # 现有 133 个用例全绿，不得删改既有断言来凑通过
npm run build
```

- 如果 T1/T4 好写单测（纯函数层面），补少量用例；组件层不强求。
- 不引入新依赖。
- 不做本 brief 之外的重构、格式化、重命名。
- 完成后输出：改动文件清单、每个任务的实现要点、测试结果。
