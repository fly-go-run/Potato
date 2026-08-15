# Potato 图标漏网审计（design/crispness-final）

只读。范围：`app/src` 25 处 lucide 导入。形状体检规范：新建 `SquarePen`、技能页 `LayoutGrid`、记忆页 `Notebook`、定时 `Clock3`、设置页 `Settings`。`MessageCirclePlus` 已清；仓内无 `AlarmClock` / `Timer`。快捷键弹窗无图标。website / console 不属 Potato 壳，不计入。

## 必改（同语义旧形）

| 文件:行 | 现图标 | 建议 | 面 |
|---|---|---|---|
| `Sidebar.tsx:158` | `PenSquare` | `SquarePen` | 侧栏「新建」行。同文件顶钮 `:140` 已是 `SquarePen`，本页两形 |
| `ChatSearchDialog.tsx:71` | `PenSquare` | `SquarePen` | ⌘K「新建会话」 |
| `ChatSearchDialog.tsx:87` | `Blocks` | `LayoutGrid` | ⌘K「技能」 |
| `ChatSearchDialog.tsx:93` | `NotebookPen` | `Notebook` | ⌘K「记忆」 |
| `SkillsView.tsx:397` | `Blocks` | `LayoutGrid` | 列表无 emoji 回退 |
| `SkillsView.tsx:510` | `Blocks` | `LayoutGrid` | 详情抽屉头 |
| `SkillsView.tsx:972` | `Blocks` | `LayoutGrid` | 技能池安装列表 |
| `SkillsView.tsx:1034` | `Blocks` | `LayoutGrid` | Hub 安装列表 |
| `MemoryView.tsx:134` | `NotebookPen` | `Notebook` | 空态 |
| `MemoryView.tsx:346` | `NotebookPen` | `Notebook` | 文件抽屉头 |

## 可改（近义、非页身份）

| 文件:行 | 现图标 | 建议 | 说明 |
|---|---|---|---|
| `Composer.tsx:783` | `Sparkles` | `LayoutGrid` 或不动 | 「+」菜单「技能」是调用，不是进 `/skills` |
| `TriggerPopover.tsx:94` | `Zap` | `LayoutGrid` 或不动 | `/` 候选无 emoji 回退；`Zap` 偏闪电 |
| `ModelPicker.tsx:399` | `Settings2` | `Settings` | 全仓唯一 `Settings2`，跳「管理模型」。设置页本身不用齿轮 |

## 不动

| 文件:行 | 现图标 | 原因 |
|---|---|---|
| `AppShell.tsx:156` | `SquarePen` | 收起态浮条，已修 |
| `Sidebar.tsx:140 / 171 / 186 / 201 / 377` | `SquarePen` / `Clock3` / `LayoutGrid` / `Notebook` / `Settings` | 规范形 |
| `ChatSearchDialog.tsx:99 / 112` | `Settings` / `MessageSquare` | 设置页、会话结果，语义对 |
| `Sidebar.tsx:591` | `PenLine` | 重命名，不是新建 |
| `CronsView.tsx:340 / 858 / 990` | `CalendarClock` / `History` / `Clock3` | 空态/历史/记录行，不是导航混用 |
| `SkillsView.tsx:277 / 302 / 449 / 1065` | `PackageOpen` / `Puzzle` | 空技能、插件身份 |
| `SettingsView.tsx:866–871` | `Bot` 等分区标 | 设置内分区，不是 `Settings`/`Settings2` |
| `Banner.tsx:17` / `ApprovalCard.tsx:37` | `AlertCircle` / `TriangleAlert` / `AlertTriangle` | 告警分档；`AlertTriangle`≡`TriangleAlert` |
| `ShortcutsDialog.tsx` | 无图标 | 文案 + kbd |

## 字重 / 尺寸（同栏）

| 栏位 | 现状 | 分级 |
|---|---|---|
| 侧栏顶钮 `PanelLeft`/`SquarePen` `:132/:140` 默认 stroke 2，主导航 `:158–215` 是 16 / **1.75** | 同壳两档 | 可改 |
| 侧栏底 `Sun`/`Moon` `:419` 默认 2，邻钮 `Settings` `:377` 是 1.75 | 同行差一档 | 可改 |
| 项目夹 `FolderClosed` `:265` 14 / **1.8** | 与导航 1.75 差 0.05 | 可改 |
| Composer `Plus` 20 / 1.9，`Mic` 18 / 1.9，发送 `ArrowUp` 18 / **2.4** | 同工具条 | 可改；发送加粗可留 |
| Chat 顶栏 `Search` `:528` 15，浮层 `Search` `:558` 14 | 差 1px | 可改 |
| ⌘K 行图标 `:239` 15 | 对侧栏 16 偏细 | 可改 |

散布值：`1.75`（侧栏导航）/ `1.8`（项目夹）/ `1.9`（Composer +/Mic）/ `2.4`（发送）/ 默认 `2`。下一轮机械归格：导航与工具栏 16+1.75，行内 chrome 14+1.8，CTA 单独加粗。

**结论：** 形状体检漏在低频面（⌘K）和页内回退（Skills `Blocks`×4、Memory `NotebookPen`×2），外加侧栏「新建」行仍是 `PenSquare`。定时任务已统一 `Clock3`。`Settings2` 仅一处，非必须。
