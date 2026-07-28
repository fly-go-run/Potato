# Phase 8 审美重构回填报告

## A. 结构层

- A1 完成：Settings / Memory / Inbox 使用 `PageContainer width="reading"`；Crons / Skills 使用 `width="wide"`；五页统一 `PageHeader`。Chat 保持满高布局及 3xl 对话流。
- A2 完成：Settings、Crons 的 4 个原生 select 全部回填 `Select`；Inbox、Skills、Crons、Sidebar 的删除确认改为受控 `ConfirmDialog`；Sidebar 改名改为 Radix Dialog + `Input`。
- A3 完成：页面操作按钮、图标按钮、输入、开关、列表卡片、状态徽标、未读计数、段控、空态和骨架回填共享原语。复合列表行仍保留语义化 button，避免把整行错误套成操作按钮。
- A4 完成：主操作统一中性 `Button primary`；Settings 分区图标降为 muted；段控使用中性选中；accent 仅保留选中、链接、运行中、开关和强调状态。
- 原语缺口：`ConfirmDialog` 不支持输入内容；Sidebar 改名按任务要求用现有 Radix + `Input` 内联实现，未改原语。

## B. 微观打磨

- B1 完成：Sidebar 入口、设置、pin 图标按 active 使用 accent / muted；未读数使用 `CountBadge`。
- B2 完成：业务代码中的 `hover:bg-line*` / `focus:bg-line*` 清零，统一为 fill-hover / fill-active。
- B3 完成：ShellToolCard / ToolCard 统一 bubble-tool 面、line 边框、md 圆角；Shell 输出区保留同一面并用 border-top 分区。
- B4 完成：Skills emoji 使用 8×8、md 圆角、bubble-tool、line 边框及 90% opacity 图标槽。
- B5/B6 完成：Crons、Memory、Skills、Inbox 使用 `EmptyState`；Sidebar 与各页/抽屉加载使用 Skeleton/SkeletonRows。
- B7 完成：业务裸 shadow-sm / shadow-raised 清零，按 elevation 使用 shadow token。
- B8 完成：上下文用量移入 composer 底栏右侧 caption。
- B9 完成：页头字阶统一；元信息、正文补充、占位分别使用 tertiary / secondary / muted。

## C. 动效

- C1 完成：所有 Radix Overlay 使用 `qp-overlay`；居中弹层/菜单使用 `qp-pop`；右侧详情/历史抽屉使用 `qp-drawer`。
- C2 完成：用户与 assistant turn 挂 `qp-msg-in`。
- C3 完成：新增交互过渡使用 `--dur-fast`；弹层动效沿用 `--dur-panel`。

## D. 工程修复

- D1 完成：`modelApi.setActive` 先解析当前 agent，再以 `scope:"agent"` 写入，避免 agent override 遮蔽 global 更新；新增 API 回归测试。
- D2 完成：cron 类型支持 once/text/request:null 的安全读取；紧凑编辑器仅允许 cron+agent+非空 request，其他变体禁用编辑并给出双语提示；新增回归测试。
- D3 完成：SSE 干净 EOF 后检查 response 终态；非终态且非主动 abort 时抛出双语断线错误并进入失败态；新增回归测试。

## 验收

- `npm test -- --run`：14 个测试文件、47 个测试全部通过。
- `npm run build`：通过；仅有任务说明 Markdown 中示例 class 被 Tailwind 扫描产生的非阻断 CSS warning，以及既有 chunk-size warning。
- 5174 真实冒烟通过：模型 Select、主题/语言、沙箱、Sidebar 改名/删除 Dialog、技能启停、真实对话、消息动效、shell 工具卡及 composer 用量锚点。
- 数据清理完成：临时会话已删除；改名、技能、主题、语言、沙箱均恢复冒烟前状态。
- 浏览器控制台无运行 error；仅有 React Router v7 future-flag 既有 warning。
- 未修改 `tokens.css`、`global.css` 或 `components/ui/` 原语。
