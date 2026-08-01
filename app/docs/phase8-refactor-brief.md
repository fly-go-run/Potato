# Phase 8 任务包：审美重构回填（执行者：Codex）

架构方（Claude）已完成**地基层**：重写了 `app/src/styles/tokens.css`（深色 elevation 分层、阴影三档、中性色四级、hover/active 填充、ring、圆角、主按钮中性色）、`app/src/styles/global.css`（全局 focus-visible 兜底、滚动条、动效 keyframes），并新建了一整套共享控件原语 `app/src/components/ui/`。你的任务是**把这套地基回填进所有 view/组件**，消除逐页手写漂移、去掉原生控件、补微观打磨与动效。

**先做**：通读 `app/src/components/ui/index.ts` 及各原语源码（Button/IconButton/Input/Select/Switch/Badge/CountBadge/Card/EmptyState/PageHeader/PageContainer/SegmentedControl/Skeleton/SkeletonRows/ConfirmDialog），理解 API；通读新 `tokens.css` 了解可用语义类（新增：`text-ink-tertiary`、`bg-fill-hover`、`bg-fill-active`、`bg-btn-primary`/`text-btn-primary-ink`、`ring-ring`、`border-line-highlight`、`shadow-[var(--shadow-sm)]` / `shadow-[var(--shadow-md)]` / `shadow-[var(--shadow-lg)]`、`--dur-fast`/`--dur-panel`）。

**硬约束**：不改 `tokens.css` 和 `components/ui/` 下的原语（它们是架构方定稿；若某原语 API 不够用，在报告里提出，先用现有的）。不新增依赖。所有新增/改动文案走 zh/en i18n。禁止硬编码色值。每改完 1–2 个 view 就 `npm run build` 验证，避免堆积错误。

## A. 结构层（最高优先）

### A1. 统一布局：所有 view 套 PageContainer + PageHeader
把每个 view 的外层容器换成 `<PageContainer width=...>`，页头换成 `<PageHeader title subtitle actions>`。宽度分配（杜绝现在 3xl/4xl/5xl/6xl 四种混用）：
- 阅读型 `width="reading"`（max-w-3xl）：SettingsView、MemoryView、InboxView。
- 宽表型 `width="wide"`（max-w-5xl）：CronsView、SkillsView。
- ChatView 不套（它是特殊的 flex 满高布局），但其内部对话流/composer 的 `max-w-3xl` 保留。
涉及：`CronsView.tsx:132`、`InboxView.tsx:129`、`SkillsView.tsx:147`、`MemoryView.tsx:71`、`SettingsView.tsx:219` 附近。

### A2. 去原生控件（最刺眼的 demo tell）
- **原生 `<select>`（4 处）→ `<Select>`**：`SettingsView.tsx` 的 Provider / 模型 两处，以及其它任何 `<select`。
- **`window.prompt`/`window.confirm`（6 处）→ 组件**：
  - 删除确认（`InboxView.tsx:89`、`SkillsView.tsx:111,129`、`CronsView.tsx:112`、`Sidebar.tsx:237`）→ 用 `<ConfirmDialog tone="danger">`，受控 open 状态。
  - Sidebar 重命名（`Sidebar.tsx:222` 的 `window.prompt`）→ 用一个小 `Dialog`（Radix，已装）内联 `<Input>` + 确认/取消，或复用 ConfirmDialog 模式加输入框。

### A3. 控件回填为原语
把各 view 手写的按钮/输入/开关/卡片/badge 换成原语：
- 手写按钮 → `<Button variant size>`。**变体映射**：原来实心 accent 的主操作（"设为活动模型""新建任务"发送键等）→ `variant="primary"`（中性近黑，这是本次核心视觉变化，把 accent 从"到处蓝"解放）；描边次要按钮 → `variant="secondary"`；纯 hover 底的 → `variant="ghost"`；删除 → `variant="danger"`。
- 图标触发按钮（Sidebar 的 MoreHorizontal、抽屉关闭 X、行内删除等）→ `<IconButton>`。
- 手写开关（`SettingsView` 沙箱、`SkillsView` 技能启停、`CronsView` 启停）→ `<Switch>`。
- 分区卡/列表容器 → `<Card>`（SettingsView 的分区外框、各列表外框）。
- 状态/版本/来源标、Cron 状态标、success 标 → `<Badge tone>`；侧栏未读数 → `<CountBadge>`。
- 段控/tab（`SettingsView` ChoiceGroup 主题/语言、`SkillsView` 技能|插件 tab）→ `<SegmentedControl>`。**注意** SkillsView 现在有药丸 + 下划线两套 tab，统一成 SegmentedControl。
- 输入框 → `<Input>` 或 `inputClasses`。

### A4. accent 克制化
- SettingsView 每个分区图标现在是 `text-accent`（5 个小节全点蓝）→ 改 `text-ink-muted`。
- 主操作按钮走 `variant="primary"`（中性），不再 accent 实心。
- 段控/选中态用 SegmentedControl 的中性选中（已内置），不用 accent-soft 蓝底。
- accent 只保留给：侧栏当前项、链接、进行中状态点、Switch 开启态、真正的强调徽标。

## B. 微观打磨

- **B1 侧栏图标跟随 active**（`Sidebar.tsx`）：图标别写死 `text-ink-secondary`；active 项图标用 `text-accent`，非 active 用 `text-ink-muted`。pin 图标、各入口图标同理。
- **B2 hover 填充**：全局把 `hover:bg-line/50`（28 处，拿描边色当填充）→ `hover:bg-fill-hover`；选中/按下底 → `bg-fill-active`。
- **B3 工具卡成面**（`ShellToolCard.tsx`、`ToolCard.tsx`）：卡片加 `border border-line rounded-[var(--radius-md)]`，底用 `bg-bubble-tool`（已提一档）；Shell 卡"命令头 + 输出区"收进同一圆角容器，展开区用 `border-t border-line`，不要再切到 `bg-bg` 造成三灰相叠。
- **B4 Skills emoji 图标槽**（`SkillsView.tsx`）：skill 自带 emoji 放进统一容器 `grid h-8 w-8 place-items-center rounded-[var(--radius-md)] bg-bubble-tool border border-line`，降饱和 `opacity-90`，避免彩色 emoji 混进 lucide 线性图标系统显得随意。
- **B5 空态**（`CronsView.tsx:170`、`MemoryView.tsx:104`、`SkillsView` 空态）→ 用 `<EmptyState icon title description action>`，取代虚线边框。
- **B6 加载态**：裸文字"加载中…/正在读取…"（Sidebar、Memory、Skills、Crons）→ 用 `<SkeletonRows>` 或 `<Skeleton>`。
- **B7 裸 `shadow-sm`**（Composer、卡片、Switch 拨片等）→ `shadow-[var(--shadow-sm)]` 或 `shadow-[var(--shadow-md)]`（弹层/抽屉用 lg）。
- **B8 上下文用量标签**（`ChatView.tsx` "上下文已用 x%"）现在悬空无锚点 → 并入 composer 底栏或做成右下角极淡 caption（`text-ink-muted text-[11px]`）。
- **B9 排版**：页面 h1 已由 PageHeader 统一为 `text-2xl font-semibold tracking-tight`；正文次级信息善用新增的 `text-ink-tertiary`（时间戳、元信息），与 `text-ink-secondary`（正文补充）、`text-ink-muted`（占位）分层。

## C. 动效（克制，只用于反馈与进入/退出）

- **C1 Radix 弹层**：所有 `Dialog.Content`/`DropdownMenu.Content` 加进入动画 —— 居中弹层用 `className="qp-pop ..."`，抽屉（右侧滑入，如 MemoryView/SkillsView 详情）用 `qp-drawer`，Overlay 用 `qp-overlay`。这些类已在 global.css 定义。
- **C2 消息进入**（`MessageList.tsx`）：新气泡挂 `qp-msg-in` 类。
- **C3 过渡时长**：交互态统一 `duration-[var(--dur-fast)]`，弹层 `var(--dur-panel)`。

## D. 工程修复（codex 自己的 review findings，随手带上）

- **D1**（`app/src/lib/api.ts` `modelApi.setActive`，硬编码 `scope:"global"`）：当 agent 已有 `active_model` 覆盖时，只改 global 不生效。核实后端 `PUT /api/models/active` 的 scope 语义，改为更新当前 agent 生效作用域（或先读 `/api/models/active` 判断是否有 agent 覆盖再决定 scope）。
- **D2**（`CronsView.tsx:349-351` 编辑器）：后端 cron 有 `schedule.type==="once"`（无 cron）与 text 型任务（request===null）。旧 console/API 建的这类任务点编辑会解引用 null 崩溃。规范化这些变体（once 型填占位/禁用 cron 字段、text 型安全取 prompt），或对不支持的类型禁用编辑并提示。
- **D3**（`chat.ts` `consumeResponse` 的 `while` 循环，约 :701）：SSE 在终态 response 帧前被干净关闭（`done`）时，直接退出会让 `isStreaming` 被清但后端还在跑。EOF 后检查 response 是否终态，非终态则触发重连或显式断线提示。

## 验收

- `npm test`（为改动的逻辑补/改测；D1/D2/D3 尽量加回归测试）+ `npm run build` 通过。
- 真实联调冒烟：设置页 Select 下拉、主题/语言切换、沙箱开关；侧栏改名/删除走新 Dialog；技能启停；一次对话看工具卡/消息动效。联调数据清理。
- 自查报告 `app/docs/phase8-report.md`（≤80 行）：逐条对照 A/B/C/D 说明完成度与未尽项。
- dev server 在 5174。**不要动 `tokens.css`、`components/ui/`、`global.css`。**
