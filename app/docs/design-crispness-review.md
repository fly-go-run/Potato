# 清晰感重校准事实审查

审查对象：`app/docs/design-crispness-proposal.md` r1。本文只记录源码、历史和可计算结果，不作 C1/C2 审美裁决。

## 0. 范围与口径

- 源码口径：`app/src`，排除 `app/dist`、`node_modules` 和仅作说明的旧文档示例。
- `text-ink-muted` 共 155 处，`text-ink-tertiary` 共 53 处；合计 208 次 token 引用，分布于 30 个 TS/TSX 文件。`SkillsView.tsx:390` 同一行同时含两个 token，所以 grep 输出为 207 行。
- 除 Tailwind 映射外，还有一处直接消费：`global.css:108` 用 `var(--ink-muted)` 作为 scrollbar thumb hover 色。
- 分类规则来自当前 token 注释：`muted` 仅用于占位、禁用、装饰；`tertiary` 用于元信息、时间戳、设置说明。下表中的“升 tertiary”和“转 icon”只是按现有语义定义归类，不代表视觉方案选择。

## 1. `muted` / `tertiary` 全仓分类

### 1.1 真占位或禁用：A 项不应改语义归属

| 文件 | 行 | 实际用途 |
| --- | --- | --- |
| `components/ui/Input.tsx` | 8 | input placeholder |
| `components/ui/Input.tsx` | 10 | disabled input 文本 |
| `components/chat/Composer.tsx` | 711 | textarea placeholder；disabled 另由 opacity 表达 |
| `views/ChatView.tsx` | 755 | 会话内搜索 placeholder |
| `views/SettingsView.tsx` | 1490 | 后端健康状态尚无值时的占位破折号 |
| `views/SkillsView.tsx` | 390（muted 分支） | 未启用 skill 的图标状态 |

以上是 grep 结果中可以直接落入“占位/禁用”的全部位置。其余 `muted` 均是装饰、信息、控件或状态表达。

### 1.2 `text-ink-muted` 完整分类

表中：

- “保留 muted（装饰）”表示符合 r1 中“装饰可继续 muted”，但修改 token 值仍会增强它。
- “升 tertiary”表示内容是说明、元信息、状态、空态或技术信息，不是占位/禁用。
- “转 icon”表示是常态功能图标、可点击图标或 spinner，属于新增 `--icon` 的覆盖面。
- “转 secondary”表示是可点击文字控件或主标签，不是弱化内容。

| 文件 | 保留 muted（装饰） | 升 tertiary | 转 icon | 转 secondary |
| --- | --- | --- | --- | --- |
| `App.tsx` | — | 119, 128（加载状态） | — | — |
| `components/chat/ApprovalCard.tsx` | — | 50, 69, 77（摘要、字段标签） | — | — |
| `components/chat/Composer.tsx` | — | 325（选项说明）, 746, 856（上下文用量） | — | — |
| `components/chat/ConversationSidePanel.tsx` | 234, 611（空态图形）；574, 934, 949, 952, 961, 1055（行号、hunk/空白符号） | 160, 166, 173, 238, 266, 331, 507, 590, 615, 638, 801, 865, 922, 984, 1029, 1072 | 279, 321, 342, 351, 363, 789, 830, 845 | — |
| `components/chat/FileToolCard.tsx` | 166（完成态行图标）, 483（未改 diff 符号） | 96, 113, 430 | 263 | 134（可点击/展示 pathNode，讨论稿补充明确要求内容档） |
| `components/chat/Markdown.tsx` | 215（语法高亮 comment） | — | — | — |
| `components/chat/MessageList.tsx` | — | — | 859 | 648, 661（密度切换文字） |
| `components/chat/ModelPicker.tsx` | — | 219, 236, 252, 257, 269, 317, 328 | 223, 293, 347, 376, 379 | — |
| `components/chat/ProgressCard.tsx` | 21, 57（完成/压缩轨道的 quiet chrome） | 75（运行状态说明） | — | — |
| `components/chat/ProjectPicker.tsx` | — | 301, 420, 534 | 411, 415 | — |
| `components/chat/ReasoningBlock.tsx` | — | 51（等待状态） | — | — |
| `components/chat/ShellToolCard.tsx` | 31（shell prompt `$`） | 51 | 68, 83 | — |
| `components/chat/ToolCard.tsx` | — | 95, 101, 107, 132 | 170 | — |
| `components/chat/ToolDisclosure.tsx` | — | — | 61 | — |
| `components/chat/TriggerPopover.tsx` | — | 79（空结果） | — | — |
| `components/layout/ChatSearchDialog.tsx` | — | 203, 211, 249 | 186, 239, 243 | — |
| `components/layout/ShortcutsDialog.tsx` | — | 46（分组标题） | — | — |
| `components/layout/Sidebar.tsx` | — | 266（计数）, 324（空态） | 141, 158, 178, 198, 209, 261, 271, 364, 537 | — |
| `components/ui/IconButton.tsx` | — | — | 8（原语默认色，影响所有 IconButton） | — |
| `components/ui/SegmentedControl.tsx` | — | 101（计数） | — | — |
| `components/ui/Select.tsx` | — | — | 30 | — |
| `views/ChatView.tsx` | — | 758, 774, 778, 793 | 714, 730, 747, 767 | — |
| `views/CronsView.tsx` | — | 353, 679, 879, 959, 979, 1075 | 992 | — |
| `views/LoginView.tsx` | — | 58 | — | — |
| `views/MemoryView.tsx` | — | 147, 352, 448, 525, 531 | 164, 194, 345 | — |
| `views/SettingsView.tsx` | — | 1235, 1565, 1721, 1856, 1881, 1901, 1912 | 934, 1569 | — |
| `views/SkillsView.tsx` | — | 463, 512, 582, 867, 899, 1024, 1150, 1223, 1274 | 423, 531, 1253 | — |

真占位/禁用位置已在 1.1 单列，未重复放入本表。

### 1.3 `text-ink-tertiary` 完整分类

| 文件 | 保留 tertiary（元信息/说明/弱化代码） | 转 icon | 转 secondary（交互文字或主标签） |
| --- | --- | --- | --- |
| `components/chat/Composer.tsx` | — | 305 | — |
| `components/chat/ConversationSidePanel.tsx` | 972, 1062（未变更 diff 正文） | — | 194, 215, 287, 1125 |
| `components/chat/FileToolCard.tsx` | 250（文件元信息）, 469（未变更 diff 正文） | — | — |
| `components/chat/MessageList.tsx` | 436（目录）, 634（静态摘要） | — | 453, 629（可点击 disclosure） |
| `components/chat/ProgressCard.tsx` | — | 72（spinner） | — |
| `components/chat/ProjectPicker.tsx` | 340（路径） | 274, 327, 356, 361 | — |
| `components/chat/ReasoningBlock.tsx` | — | — | 28（可点击 disclosure） |
| `components/chat/ShellToolCard.tsx` | — | — | 73（完成态命令是行主标签） |
| `components/chat/ToolCard.tsx` | — | — | 126（完成态工具名是行主标签） |
| `components/chat/TriggerPopover.tsx` | 58, 121 | 114 | — |
| `components/layout/Sidebar.tsx` | 549（时间戳） | — | 227, 306（讨论稿补充明确：折叠组入口不再常态用 tertiary） |
| `components/ui/EmptyState.tsx` | 23（装饰图形）, 28（说明） | — | — |
| `components/ui/PageHeader.tsx` | 33 | — | — |
| `components/ui/SegmentedControl.tsx` | — | — | 95（未选中 tab 标签） |
| `views/ChatView.tsx` | 690（工作区元信息） | 231 | — |
| `views/CronsView.tsx` | 381, 508, 1001 | 502 | — |
| `views/MemoryView.tsx` | 171, 533 | — | — |
| `views/SettingsView.tsx` | 1057, 1078, 1560, 1861, 2129 | — | — |
| `views/SkillsView.tsx` | 403, 468, 591, 1176 | 390（启用分支）, 444, 505, 1159 | — |

讨论稿当前工作区版本已经明确写出“折叠组入口不再用 tertiary 当常态”，所以 `Sidebar.tsx:227,306` 不再是待定边界项。另有 `Sidebar.tsx:534` 的非 active 会话标题使用 `text-ink-secondary`，它不在本次两个 grep token 内，但讨论稿补充已明确要求会话标题升到 ink。

## 2. token 值直接修改的波及面

### 2.1 数量与非文本依赖

- 若先不迁移语义类而直接改 token，155 个 `muted` 引用和 53 个 `tertiary` 引用会同步变化。
- `global.css:108` 的 scrollbar thumb hover 直接使用 `--ink-muted`；它不在 `text-ink-muted` grep 清单内。浅色 `#9b9b9b → #8f8f8f` 会使 scrollbar hover 变深；深色效果取决于尚未给出的新 dark 值。
- `components/ui/IconButton.tsx:8` 把 muted 写在共享原语 base 上。只改 muted 值会让所有 IconButton 一起增强；引入 `--icon` 并改原语则会一次覆盖其全部消费者。

### 2.2 明确依赖“装饰性弱化”的场景

以下场景当前不是误用，而是在用 muted/tertiary 主动压低视觉权重；升档后会比现状更显眼：

1. 代码与 diff chrome：`Markdown.tsx:215` 的注释语法色；`ConversationSidePanel.tsx:574,934,949,952,961,972,1055,1062` 的行号、hunk、符号和上下文行；`FileToolCard.tsx:469,483` 的未变更行和空符号。
2. 已完成工具的 quiet 展示：`ProgressCard.tsx:21,57`、`FileToolCard.tsx:166`、`ShellToolCard.tsx:68`、`ToolCard.tsx:126,132`。这是现存代码的弱化依赖；讨论稿补充已明确禁止对内容本体降档，所以 shell 命令、通用工具标签和 FileTool pathNode 已在 1.2/1.3 归入内容档，装饰图标仍单列。
3. 层级标签与技术信息：菜单分组、计数、内部 id、路径、空态说明、Dialog description、设置说明。它们多数应从 muted 归 tertiary，但新的 tertiary 值也会使这整层同步变强。
4. 常态图标与 chevron：当前广泛依赖 muted 让图标退后；新增 `--icon` 会把这些功能图标升到 `ink-secondary` 档，而不是只改变侧栏五个入口。
5. scrollbar hover：这是唯一直接用 `var(--ink-muted)` 的非文本装饰依赖。

## 3. 对比度核算

计算口径：WCAG 相对亮度公式，结果保留两位。普通小字号文本参考线为 4.5:1；大文本和必要 UI 图形参考线为 3:1。这里不把“通过/不通过”扩展为审美结论。

### 3.1 浅色主题：现值与 r1 明确给出的新值

前景：tertiary `#747474 → #6d6d6d`，muted `#9b9b9b → #8f8f8f`；新增 icon 按提案取 `#505050`，icon-strong 取 `#202020`。

| 背景 | tertiary 现 | tertiary 新 | muted 现 | muted 新 | icon | icon-strong |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| canvas `#fbfbfb` | 4.52 | 5.00 | 2.69 | 3.13 | 7.79 | 15.75 |
| bg/侧栏 `#f5f5f4` | 4.28 | 4.74 | 2.55 | 2.96 | 7.39 | 14.94 |
| surface `#ffffff` | 4.67 | 5.17 | 2.78 | 3.23 | 8.06 | 16.29 |
| bubble-user `#ececec` | 3.96 | 4.38 | 2.35 | 2.74 | 6.83 | 13.79 |
| bubble-tool `#f3f3f3` | 4.21 | 4.66 | 2.50 | 2.91 | 7.27 | 14.68 |

可直接读出的边界：

- 新 muted 在 canvas/surface 上超过 3:1，在 bg、bubble-user、bubble-tool 上分别为 2.96、2.74、2.91，仍低于 3:1。
- 新 tertiary 在 canvas/bg/surface/bubble-tool 上超过 4.5:1，在 bubble-user 上为 4.38:1。
- r1 所说侧栏现状“约 2.7:1”按实际 `#9b9b9b` / `#f5f5f4` 精算为 2.55:1；2.69:1 对应的是 canvas `#fbfbfb`。
- 实际源码还有 `bg-bg/70`、`bg-fill-hover/60` 等透明混合背景；其最终对比度取决于下层像素，不等同于本表的实色 token 结果。

### 3.2 深色主题：只能核算现值

r1 只写“深色对应微调”，没有给出新的 dark tertiary/muted/icon hex，因此不能形成“现值 vs 新值”的可复算表。当前值如下，供后续新值确定后对表：

| 背景 | tertiary `#8f8f8f` | muted `#6f6f6f` | icon=`#b0b0b0` | icon-strong=`#ececec` |
| --- | ---: | ---: | ---: | ---: |
| canvas `#141414` | 5.70 | 3.67 | 8.49 | 15.59 |
| bg `#1c1c1c` | 5.27 | 3.39 | 7.86 | 14.43 |
| surface `#202020` | 5.04 | 3.24 | 7.51 | 13.79 |
| bubble-user `#262626` | 4.68 | 3.01 | 6.98 | 12.81 |
| bubble-tool `#222222` | 4.92 | 3.17 | 7.34 | 13.47 |

## 4. B 项“三件套”改动范围

### 4.1 共享 Button 原语

`components/ui/Button.tsx:28-29` 的 secondary 已有：

- `bg-surface`
- `shadow-[var(--shadow-control)]`
- `border border-transparent`

因此三件套缺口只有可见的 `border-line`；不是从零增加 surface 和 shadow。若同时落实 r1 的深色规则，secondary 还需要在 dark 下从 `shadow-control` 切换为“表面差 + line-highlight”。当前 dark `--shadow-control` 仍是黑影 `0 1px 2px rgba(0,0,0,.28)`，并未使用 line-highlight。

共享 secondary 变体有 19 个显式调用，加上 3 个依赖默认 variant 的调用，共 22 个静态调用点，消费者如下：

| 消费组件 | 范围 |
| --- | --- |
| `components/chat/ApprovalCard.tsx` | 审批次级操作 |
| `components/desktop/DesktopHostBridge.tsx` | 最小化、稍后下载、更新重试（3 个默认 secondary） |
| `views/ChatView.tsx` | 限流后的备选模型按钮 |
| `views/CronsView.tsx` | 重试、无目标 CTA |
| `views/MemoryView.tsx` | 刷新、编辑、重试 |
| `views/SettingsView.tsx` | 主题/模型/Provider 的导入、发现、测试、次级保存等 |
| `views/SkillsView.tsx` | 重试、搜索/导入、能力导入 |

`Button` 的 primary/ghost/danger 不会因只改 secondary 变体而自动获得三件套。`ConfirmDialog.tsx:70` 使用动态 primary/danger，也不属于默认 secondary。

### 4.2 chip / segmented controls

- 正式 chip 原语只有 `components/ui/SegmentedControl.tsx`。共 6 个调用：`CronsView.tsx:289`，`SettingsView.tsx:1114,1261,1594`，`SkillsView.tsx:201,880`。
- track 变体的选中项已是 `bg-surface + shadow-sm`，没有 border；tabs 变体的选中项是 `bg-btn-primary + shadow-control`，不是“白底次级 chip”。两种变体不能通过同一条 class 替换得到相同三件套。
- `ChatView.tsx:178-235` 有两套未走原语的自定义 chip：场景 tab 和建议 chip。建议 chip 已有 `bg-surface + shadow-control + border-transparent`，与 Button secondary 的缺口相同；场景 tab 的选中项是 `bg-btn-primary`。
- `ConversationSidePanel.tsx:1104-1127` 的 `PanelTab` 是另一套自定义 track，选中项已有 `bg-surface + shadow-sm`、无 border。
- `MessageList.tsx:641-664` 的密度切换是小型描边分段控件；它已有外层 `border-line`，没有独立 surface/shadow。
- `ModelPicker.tsx:211` 与 `ProjectPicker.tsx:244` 是当前状态触发器，分别为自定义透明 pill 和 ghost Button；如果“可点 chip”包含它们，需要单独改，改 Button secondary 不会覆盖。
- `Badge` / `CountBadge` 是非交互状态标记，不在“可点 chip”范围；`Button shape="pill"` 当前没有调用点。

### 4.3 侧栏选中 pill

侧栏没有选中态原语。主入口选中底分别写在 `Sidebar.tsx:138,149,169,189`，会话行在 `Sidebar.tsx:526`；均只使用 `bg-fill-active`，没有 border 或 shadow。选择“加深 fill-active”会波及全仓所有 active fill；选择“白底+边”则只需要改这些局部 class，但两者不是同一改动范围。

### 4.4 B 中图标统一的独立范围

这不属于三件套，但其规模可由源码确定：Lucide 数值 size 目前包含 11、12、13、14、15、16、17、18、20、22、24、28 共 12 档；13px 有 37 处，15px 有 30 处，17px 有 4 处，均不在偶数网格。显式 Lucide `strokeWidth` 只有 `ChatView.tsx:198` 的 1.8、`Composer.tsx:730,805` 的 1.9 和 `Composer.tsx:829` 的 2.4；其余使用 Lucide 默认值。统一到 1.75/2 与偶数尺寸不是 token 单点修改。

## 5. C2 历史与仍存约束

### 5.1 可复核历史

1. 初始代码中的浅色 accent 是 `#3d6df2`。
2. `9c39a9ef`（`refactor(app): phase 8 aesthetic overhaul — tokens, primitives, neutral chrome`）把它改为 `#3b6ef0`，并明确：主按钮使用独立的中性 `--btn-primary`；accent 只用于选中、链接、进行中等真状态；侧栏选中被中性化以对齐 Codex Desktop。
3. `45c81d09`（`feat(app): refresh office frontend interactions`）把浅色 accent 从 `#3b6ef0` 改为 `#4a4a4a`，dark 从 `#6b8fe6` 改为 `#c9c9c9`，ring 同步改为中性。该提交中的 token 注释写明“所有界面 chrome 都使用中性灰；彩色只留给成功/警告/错误等真正有语义的状态”。
4. 同提交的 `reference/workbuddy-review-loop-r4.md` 最终回归记录明确写有“首页保持中性灰阶，无蓝色焦点框”；`reference/codex-app-inventory.md:21,35` 记录 Codex 侧栏选中是中性灰，除彩色技能图标外零 accent。
5. `#2563e0/#2563E0` 没有作为 `tokens.css` 的实际值出现在可达 git 历史。它只在 `reference/gap-review-opus-r2.md:97` 被文字描述为“QwenPaw 的 accent”；该提交父版本的真实 token 是 `#3b6ef0`。因此“历史上用过 #2563e0”有文档叙述依据，但没有 token 代码依据。

### 5.2 原因是否仍成立：文档状态

- “中性 chrome、主按钮与 accent 解耦、无蓝色焦点框”仍是当前 `tokens.css` 注释和 r4 最终验收记录；仓库内没有比 r1 更新的已通过设计决议废止这些约束。
- 当前 r1 提案本身把 C2 标为“需用户拍板”，所以它是对既有约束的显式重开，不是已经覆盖旧决议的新结论。
- C2 的四个落点中，选中图标、focus ring、主 CTA 会直接反转上述旧约束；switch on 在代码中同样仍走中性主按钮 token。

### 5.3 C2 的实际代码波及风险

只把 `--accent` 改蓝不会得到 r1 声明的“仅四处”：

- 当前 `text/bg/border/ring/decoration-accent` 还用于 `MemoryView` hover 图标、Chat 拖拽上传层、Skills 上传/导入提示、桌面更新图标、Approval 图标、Model/Project picker 对勾与 git 标记、消息定位 ring、Markdown 链接和语法高亮、Badge/CountBadge。
- `global.css:53` 的文本选区背景使用 `--accent-soft`；`global.css:76` 的全局 focus fallback 直接使用 `--accent`。
- primary CTA 实际使用 `--btn-primary`（`Button.tsx:25-27`），Switch on 也使用 `bg-btn-primary`（`Switch.tsx:37`）。要让这两处变蓝，必须改组件映射或 `--btn-primary`，仅改 `--accent` 无效。
- 组件自己的 focus ring 多使用 `--ring`，全局 fallback 使用 `--accent`。要统一成蓝色 focus，需要同时处理两条路径；只改其中一个会产生两种 focus 色。

因此，“仅四处”是一次语义重映射任务，不是单一 accent token 换值任务。
