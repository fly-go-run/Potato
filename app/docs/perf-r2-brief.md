# 性能优化第二轮：拆包 + 消息列表（perf-r2）

## 背景

perf-r1（SSE 合帧、字段级订阅、高亮防抖）已落地。本轮做两件事：右侧面板真正懒加载（把 Markdown/Shiki 从主包剥离），以及 MessageList 的重复计算消除。仅改 `app/` 前端，不改视觉样式、design tokens、文案、i18n，不引入新依赖。

## 任务

### T1 拆出轻量共享代码，面板改 React.lazy

现状：`ConversationSidePanel.tsx`（约 1200 行，静态导入 Markdown）被两处静态引用——`ChatView.tsx:38-41` 导入 `collectConversationArtifacts` + 组件本体，`MessageList.tsx:25` 导入 `ChangeStat`。导致面板、Markdown、Shiki 全进主包（当前主入口 530.8 kB），`MessageContent.tsx`/`ReasoningBlock.tsx` 里的 Markdown 动态导入形同虚设（构建时有明确警告）。

改法：

1. 新建 `app/src/lib/conversationArtifacts.ts`：把 `ConversationArtifact` 接口、`collectConversationArtifacts`、`presentRunStatus` 原样移过去（它们是纯函数/类型，不依赖面板 UI）。注意移动后不得引入对面板文件的反向依赖。
2. 新建 `app/src/components/chat/ChangeStat.tsx`：把 `ChangeStat` 组件原样移过去。
3. 更新引用方：
   - `MessageList.tsx` 改从 `./ChangeStat` 导入。
   - `ChatView.tsx` 改从 `../lib/conversationArtifacts` 导入函数，面板组件改为 `React.lazy(() => import(...).then(m => ({ default: m.ConversationSidePanel })))`，渲染处用 `<Suspense fallback={null}>` 包住（面板本身已经是 `sidePanelOpen &&` 条件渲染，fallback 用 null 即可）。
   - `ConversationSidePanel.tsx` 自身改从新位置导入被移走的符号。**不得在面板文件里 re-export 这些符号**，否则静态链路依旧存在、拆包失效。
   - `ConversationSidePanel.test.ts` 的导入路径同步更新（`collectConversationArtifacts`、`presentRunStatus` 从新 lib 导入），测试断言不变。
4. `MemoryView.tsx:12` 也静态导入 Markdown，但 MemoryView 本身已是 lazy 路由 chunk，**不需要改**。

验收要点：`npm run build` 后主入口 `index-*.js` 应明显小于当前 530.8 kB（Markdown/Shiki 引擎、面板应各自成为独立 chunk）。在报告里对比前后主入口体积。

### T2 MessageList 消除重复计算

文件：`app/src/components/chat/MessageList.tsx`。

1. **turns 分组 memo**：`MessageList` 目前每次渲染裸调 `groupIntoTurns(messages)`（约 57 行），改为 `useMemo(..., [messages])`。
2. **callId → output 预建 Map**：约 203 行处，工具调用配对目前在循环里 `messages.find(...)` 逐条扫全量消息，工具多时 O(n²)。改为循环开始前一次遍历构建 `Map<callId, StreamMessage>`（key 为 `stringValue(toolData(candidate).call_id)`，只收 `isToolOutput` 的消息；同一 callId 出现多条时保留首条，与现有 `find` 语义一致），配对处查 Map。
3. **已完成回合 memo**：`UserTurn` 和 `AssistantTurn` 用 `React.memo` 包裹，并提供自定义比较函数：
   - `messages` prop 用逐元素引用相等比较（长度相同且每个位置 `Object.is` 相同）。流式 reducer 只为有更新的消息创建新对象，历史消息引用稳定，所以流式期间只有最后一个回合会重渲染。
   - 其余 props（`activeMessageId`、`showActions`、`regeneratePrompt`、`onOpenFile`、`onOpenChange`）用 `Object.is` 比较。注意 `onOpenFile`/`onOpenChange` 来自 ChatView，若当前是不稳定的内联函数/每渲染新建的闭包，请在 ChatView 里用 `useCallback` 稳定化，否则 memo 白做。
   - 分组产生的 `turn.messages` 子数组每次都是新数组，这正是要靠自定义比较函数吸收的——不要因此改动 `groupIntoTurns` 的返回结构。

## 验收标准

在 `app/` 目录下全部通过：

```
npx tsc --noEmit
npm test          # 133 个用例全绿；T1 移动导致的 import 路径更新允许，断言不得删改
npm run build
```

- 不做本 brief 之外的重构、格式化、重命名。
- 完成后输出：改动文件清单、主入口 chunk 前后体积对比、测试结果。
