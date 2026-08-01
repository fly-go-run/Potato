# WorkBuddy vs QwenPaw 深度差距审查 r2

- 审查日期：2026-07-28
- QwenPaw 证据：`.reference-shots/qwenpaw-r7/` 10 张 1280×860 截图，以及 `app/src/`
- WorkBuddy 证据：`.reference-shots/workbuddy/` 正式编号截图
- 基线：`gap-analysis-workbuddy-r1.md` 中 W1–W8 均视为已落地。本报告不再把“双层 composer、能力 chips、消息动作行、模板卡、侧栏分组/折叠、深色抬升”等作为缺失项重复上报，只审查其落地不足及更深层差距。
- 结论：r7 已经不再是“功能骨架缺失”，但仍明显像一套把开发者数据直接铺进通用 UI 原语的前端。最严重的不是少一张卡，而是核心会话暴露执行管线、多个产品页暴露后端命名、深色弹层的遮罩方向写反。视觉层面又有过宽、过圆、过重阴影和过大的通用页头，导致它比 WorkBuddy 更像 Web 管理台。

## 分级口径

- **P0**：首屏或核心路径一眼暴露原型/开发工具感，或视觉层级方向错误。
- **P1**：普通用户无需细看就能察觉的成熟度差距。
- **P2**：不阻断使用，但持续削弱一致性、可读性和桌面产品感。

## P0：硬伤

### P0-1 会话页没有持久页头，用户进入历史任务后失去任务身份

**现状**：`qwenpaw-r7/chat-light.png`、`chat-dark.png` 顶部直接从一截被裁断的用户气泡/执行记录开始，没有会话标题、任务状态或页级操作；滚到任意位置都无法确认自己在哪个任务里。代码证据是 `app/src/views/ChatView.tsx` 根节点内直接进入 banner、滚动区和 composer，没有 header 原语。  
**WB 做法**：`workbuddy/10-task-conversation.png`、`31-dark-task.png` 顶部固定约 42px 的任务栏，左侧始终显示“Word转PDF文件转换”，右侧集中搜索、分享、历史等动作；正文滚动不会带走任务身份。  
**建议修法**：在 `ChatView.tsx` 增加独立 `ChatHeader`（建议新文件 `app/src/components/chat/ChatHeader.tsx`），固定 `h-11`（44px）、`px-5`、`border-b border-line`；标题用 `14px/20px, font-medium`，右侧图标按钮使用 32×32px、图标 15px。滚动容器只包 `MessageList`，header 不进入滚动。无标题时显示“新会话”，不要留空。

### P0-2 “安静工具行”修得远远不够：执行管线和本机路径仍然占据正文

**现状**：`qwenpaw-r7/chat-light.png` 中一个正常的 PDF 转换回合连续铺出 7 条工具行，包括 `Skill skill: pdf`、多条 shell 命令、`Glob Search`、`Send File To User`，并展示 `/Users/liuxu/.qwenpaw/...`；顶部用户气泡还直接出现“已经下载到 /Users/liuxu/...”。这些行从约 y=116 延续到 y=342，执行过程比最终答案更抢眼。`MessageList.tsx` 的 `AssistantTurn` 对每条 tool message 单独渲染 `ToolCard`；`UserTurn` 则把所有 user-role 文本无差别原样渲染。`ToolCard.tsx`、`ShellToolCard.tsx` 的完成态虽降为 28px 行高，但没有跨工具聚合，也没有路径脱敏。  
**WB 做法**：`workbuddy/10-task-conversation.png`、`31-dark-task.png` 把整段执行过程收成单行“已完成 1m14s >”，原始命令和路径默认不进入阅读流；正文和产物才是视觉主角。  
**建议修法**：

1. 在 `MessageList.tsx` 的 turn 级别先把连续 reasoning/progress/tool call-output 配对聚成一个 `ExecutionGroup`，完成态只显示一行：`min-h-7`、`12px/18px`、“已完成 · 7 个步骤 · 1分14秒”；仅展开后再渲染现有 `ToolCard` 明细。
2. 在展示层统一把工作区绝对路径折叠成 `…/media/文件名`，不得在默认摘要行出现 `/Users/<用户名>`。
3. 对“附件已被后端下载到本地”的 transport receipt 做消息类型过滤，用户侧改为附件 chip + 原始用户文字；不要把 transport 文案塞进 `UserTurn`。
4. 单个失败/等待审批步骤可越过聚合单独强调，普通成功步骤不得逐条打勾占版面。

### P0-3 技能页直接暴露内部 ID、英文系统说明和任意 emoji，像调试面板

**现状**：`qwenpaw-r7/skills-light.png` 首屏出现 `QA_source_index`、`browser_cdp`、`chat_with_agent`、`multi_agent_collaboration` 等内部标识，说明全是英文且被单行截断；图标混用 🗂️、🔌、🖥️、⏰、✍️ 和“✦”占位，视觉重量完全不同。`SkillsView.tsx` 的 `SkillRow` 直接输出 `skill.name`、`skill.description`、`skill.emoji || "✦"`，没有产品展示层。中文壳子包英文 schema，是最直接的“半成品”信号。  
**WB 做法**：`workbuddy/06-sidebar-experts.png` 使用面向用户的中文名称、完整的一句能力说明、统一头像/图标容器和标签；内部调用名不进入列表主标题。  
**建议修法**：在 `app/src/lib/capabilities.ts` 之上增加纯展示映射（例如 `capabilityPresentation.ts`），为 skill 提供 `displayName`、本地化 `summary`、`category`、统一 Lucide 图标；`SkillRow` 主标题只用展示名，内部 ID 移到详情抽屉的“技术信息”。未知 skill 的兜底应把 snake_case 转为空格标题并隐藏低质量英文说明，不能直接透传。图标统一 18px、`strokeWidth=1.75`，放入 36×36px、`radius-sm` 的中性托盘；emoji 仅在元数据明确声明为品牌图标时使用。

### P0-4 深色设置遮罩的颜色方向写反，背景不是压暗而是被漂白

**现状**：`qwenpaw-r7/settings-dark.png` 中弹层外背景变成大面积灰白雾层；浅色 `settings-light.png` 的背景压暗又太弱，弹层和原页面仍互相争夺。代码证据：`SettingsView.tsx` 使用 `bg-ink/25`；深色 token 中 `--ink: #ececef`，因此实际是在深色背景上叠 25% 近白色。相同写法还散布在多个 Dialog overlay。  
**WB 做法**：`workbuddy/19-settings-system.png`、`24-settings-models.png` 的原页面统一被黑色遮罩压到约中深灰，设置面板成为唯一亮面；遮罩在任何主题下都表达“后退一层”，不会随文字色反转。  
**建议修法**：**需架构方改 `tokens.css`**。新增独立 `--overlay`，建议浅色 `rgba(0,0,0,.44)`、深色 `rgba(0,0,0,.58)`，映射为 `bg-overlay`；将 `SettingsView.tsx`、`ConfirmDialog.tsx`、Memory/Skills/Crons drawer 等所有 `bg-ink/20`、`bg-ink/25` 统一替换。遮罩是表面语义，绝不能复用文本 token。

## P1：明显差距

### P1-1 产物卡只覆盖 `write_file/append_file`，实际“发送给用户”的文件仍然丢在正文里

**现状**：`qwenpaw-r7/chat-light.png` 明明出现 `Send File To User ...京东~xLLM-images.zip`，最终答案却只用一块 inline code 显示文件名，没有大小、文件类型、打开动作或产物汇总入口。`FileToolCard.tsx` 的 `FILE_TOOL_TITLES` 只登记 `read_file/write_file/edit_file/append_file`，`ARTIFACT_TOOLS` 只含 `write_file/append_file`；`ToolCard.tsx` 因而把 `send_file_to_user` 当普通工具行。W3 的“产物卡”存在，但没有覆盖真实交付路径。  
**WB 做法**：`workbuddy/10-task-conversation.png` 对最终 PDF 使用满宽文件卡，明确展示类型图标、文件名、218.8KB 和右上打开按钮，并在下方提供“查看所有产物 (1)”。  
**建议修法**：在 `FileToolCard.tsx` 把 `send_file_to_user`（及协议中的等价名称）纳入 artifact 路由，从 `file_path` 取文件名并读取后端 size/mime；卡片建议 `min-h-12`、`px-3 py-2.5`、无描边 `bg-bubble-tool`、14px 文件名、12px 元信息、32×32px 打开按钮。`AssistantTurn` 收集本轮 artifact，至少 1 个时显示“查看所有产物 (n)”的 12px 汇总行。

### P1-2 首页和会话共用 832px 标尺，标题也膨胀到 32px，视觉像网页 hero

**现状**：`qwenpaw-r7/home-light.png`、`home-dark.png` 的 composer/能力区宽 832px（源码 `max-w-[52rem]`），占去 1024px 主区约 81%；空态标题在 640px 以上直接使用 32px。`chat-light.png` 的正文也沿用同一个 832px measure，长工具路径和正文横向拉得很开。  
**WB 做法**：`workbuddy/01-home-office-mode.png`、`30-dark-home.png` 中主区约 854px，composer 截图量取约 628px（约 74%）；品牌标题约 24–26px。`10-task-conversation.png` 的正文列约 630px，阅读行长明显短于 QwenPaw。  
**建议修法**：不要让首页输入宽度等于会话阅读宽度。`ChatView.tsx` 的空态标题改为 `26px/34px`（窄屏 24px），Capability/Composer 改 `max-w-[46rem]`（736px）；`MessageList.tsx` 阅读列改 `max-w-[44rem]`（704px），composer 在会话页可维持 `max-w-[46rem]`。标题到说明 `8px`，说明到 chips `28px`，不要用当前 `mt-3 + mt-9` 形成 48px 的松散断层。

### P1-3 Composer 的圆角和阴影过量，双层结构被画成了“三圈发光边”

**现状**：`qwenpaw-r7/home-light.png` 的 composer 四周有明显灰色光晕，`home-dark.png`、`chat-dark.png` 中则形成黑色“护城河”；外壳 24px 圆角、内卡 18px 圆角，再叠 border 与 `shadow-md`。源码为 `Composer.tsx` 的 `rounded-[24px] bg-bg p-1.5` + `rounded-[18px] ... shadow-[var(--shadow-md)]`；当前 `--shadow-md` 是两层 `0 2px 4px` + `0 4px 12px`，深色更重。  
**WB 做法**：`workbuddy/01-home-office-mode.png`、`30-dark-home.png` 的 composer 主要靠白卡/底栏色差和约 12–14px 圆角分层，外沿几乎没有持续发光；明显阴影只留给弹层。  
**建议修法**：内卡收至 `radius-lg`（14px），外壳 16px，基座 padding 从 6px 收到 4px；常态只保留 1px border。**需架构方改 `tokens.css`**：新增 `--shadow-composer`，浅色建议 `0 1px 2px rgba(17,17,26,.06)`，深色建议 `inset 0 1px 0 rgba(255,255,255,.05)`，不要复用弹层级 `shadow-md`。聚焦时仅加 2px ring，不再同时加重 shadow。

### P1-4 通用页头 26px/36px + 32px 下边距，把所有工具页都做成了 SaaS 控制台

**现状**：`crons-light.png`、`skills-light.png`、`inbox-light.png`、`memory-light.png` 都用同一个超大标题；`PageHeader.tsx` 固定 `26px/36px, font-semibold` 和 `mb-8`。技能页随后又出现“技能/插件”tab，记忆页又出现分组标题，形成重复层级；正文要到 y≈126 后才开始。  
**WB 做法**：`workbuddy/07-sidebar-automation.png` 直接以紧凑分段控件开场；`06-sidebar-experts.png` 顶部 tab/搜索约 32px 高，真正的内容标题“精选场景”约 20px；`23-settings-memory.png` 的页标题约 16px。WB 根据页面类型分层，不把每一页都当营销 landing page。  
**建议修法**：给 `PageHeader` 增加 `variant="utility" | "catalog"`。utility（定时任务/收件箱/记忆）建议 `22px/30px`、副标题 `13px/20px`、`mb-6`；catalog（技能）改为 tab + 搜索做第一行，页面名用 18–20px 或只保留在侧栏/窗口标题。首页 display 样式单独维护，不能复用工具页标题。

### P1-5 侧栏行高和宽度偏大，5 条会话就形成一整块稀疏表格

**现状**：`home-light.png`、`chat-light.png` 的侧栏固定 256px（`Sidebar.tsx: w-64`）；主导航和会话按钮均 `text-sm + py-2`，Tailwind 的 14px/20px 行盒加上下各 8px 后单行高 36px，再叠 `mt-0.5/space-y-0.5`，实际节拍约 38px，时间还是 12px。5 条会话从 y≈309 延续到 y≈499。底部的版本信息又占用一整条约 49px 锚点。  
**WB 做法**：`workbuddy/01-home-office-mode.png`、`10-task-conversation.png` 的侧栏约 207px，任务行截图量取约 25–28px；同样 5 条任务只占约 125px，分组和项目仍有空间。  
**建议修法**：`Sidebar.tsx` 改为 232px（`w-[14.5rem]`）；主导航/会话行统一 32px，使用 `text-[13px] leading-5 px-2.5`，会话时间降为 11px；一级功能之间用 2px 间隔、分组前留 8px，不用 38px 行高制造“舒展”。图标保持 16px，但 `strokeWidth` 统一 1.75，避免在更紧行高中显粗。

### P1-6 设置面板过高且左右表面未分层，模型页下半屏是一整块无意义白板

**现状**：`settings-light.png`、`settings-dark.png` 的面板高 731px（源码 `sm:h-[85vh]`），模型内容约 263px，剩余 400px 以上纯空；左侧 nav 和右侧内容都继承 `bg-surface`，只靠 1px 竖线区分。空白不是围绕信息组织的留白，而是固定高度留下的未填区域。  
**WB 做法**：`workbuddy/19-settings-system.png`、`24-settings-models.png` 面板约 565/768px（约 74vh），左侧导航为明确的浅灰面，右侧为白色内容面；即使模型项很少，也不会占到 85vh。  
**建议修法**：`SettingsView.tsx` 桌面尺寸改为 `sm:h-[min(40rem,calc(100vh-3rem))]`（上限 640px），模型/外观等短页允许 `h-auto min-h-[32rem]`；nav 加 `bg-bg`，右区保持 `bg-surface`。nav 宽 176–184px 即可，当前 192px 可再收 8–16px。内容组与 header 间距维持 12px，不要靠扩大整个 modal 获得留白。

### P1-7 说明文本被当成 placeholder，浅深主题都低于功能文字应有的可读性

**现状**：`settings-light.png` 的“从当前 Provider 拉取…”、`crons-light.png` 空态说明、设置深色说明都接近消失。`SettingRow`、`EmptyState` 大量使用 `text-ink-muted`；token 为浅色 `#9a9aa2`、深色 `#6d6d76`。按实际背景计算：`#9a9aa2` 对 `#f2f2f4` 对比度约 **2.50:1**，`#6d6d76` 对 `#1f1f25` 约 **3.20:1**，却用于 12–14px 的有意义说明。  
**WB 做法**：`workbuddy/19-settings-system.png` 中每项说明虽次于标题，但仍可在正常缩放下一眼读出；placeholder/禁用文字才退到最弱色阶。  
**建议修法**：`SettingRow`、`EmptyState`、抽屉说明改用 `text-ink-tertiary` 或专门的 `text-description`，`ink-muted` 只留给 placeholder/disabled。**需架构方改 `tokens.css`**：浅色 `--ink-tertiary` 建议从 `#74747d` 压到 `#686872`（对 `#f2f2f4` 约 4.93:1）；深色现有 `#8b8b94` 可保留。不要靠加粗补偿低对比。

### P1-8 所有 Button 一律药丸化，形状语言与 WB 的“功能按钮 / chip”分工相反

**现状**：`crons-light.png` 的两个“新建任务”、`settings-light.png` 的“发现模型/管理模型”、技能页“添加”全部是 full pill。根因是 `Button.tsx` base 无条件 `rounded-full`；只有列表卡片在用 10/14px token，操作按钮反而没有层级差异。  
**WB 做法**：`workbuddy/07-sidebar-automation.png` 的黑色主 CTA、`19-settings-system.png` 的设置控件、`24-settings-models.png` 的“添加模型”均是约 6–8px 圆角的功能矩形；只有模式切换、筛选和能力 chip 使用胶囊。  
**建议修法**：`Button.tsx` 默认改 `rounded-[var(--radius-sm)]`（8px）；增加明确 `shape="pill"` 仅供能力 chips、筛选 chip、状态选择使用。IconButton 保持圆形。这样 8/10/14/18px 的现有圆角标尺才有角色，而不是“按钮全圆、容器半圆”的两套语言。

### P1-9 定时任务空态同时出现两个相同主按钮，并被一张 286px 高的大卡框住

**现状**：`qwenpaw-r7/crons-light.png` 右上和空态中心各有一个“+ 新建任务”，同屏形成两个同权黑色锚点；空态外层还是约 944×286px 的浅灰描边卡。代码中 `PageHeader.actions` 和 `EmptyState.action` 无条件同时渲染，`EmptyState.tsx` 又固定 `border + bg-bubble-tool/60 + py-16`。W5 补了模板，但空态容器仍像后台 placeholder。  
**WB 做法**：`workbuddy/07-sidebar-automation.png` 空数据时只保留中心“+ 添加自动化”一个主操作，图标/文案直接落在画布上；模板区自然从下半屏开始，没有巨型外框。  
**建议修法**：`CronsView.tsx` 在 `jobs.length===0` 时隐藏 PageHeader action，非空时才显示右上按钮；给 `EmptyState` 增加 `variant="bare"`，本页使用无 border/无填充、`min-h-[15rem] py-12`。模板标题到网格 12px、空态到模板区 28–32px 即可。

### P1-10 收件箱把后台事件原文直接当成产品文案，中文 UI 中出现半屏英文日志

**现状**：`qwenpaw-r7/inbox-light.png` 出现 `Auto-dream result`、`Auto-memory result`、英文 `success` badge，以及 `Files: 2 scanned...`、`Date: 2026-07-27`，下方又混入中文“来源：memory · 2026年7月…”。`InboxView.tsx` 直接输出 `event.title`、`event.body`、`event.source_type`；`EventStatus` 更是把原始 `status` 原样塞进 Badge。  
**WB 做法**：`workbuddy/07-sidebar-automation.png` 的任务模板和 `10-task-conversation.png` 的执行结果均使用面向人的标题与自然语言，不把 source/status/schema 直接暴露在主要扫描路径。  
**建议修法**：新增 `inboxEventPresenter.ts`，按 event type 映射本地化标题、状态和摘要；`success/error/running` 映射“成功/失败/运行中”，source `memory/cron` 映射“记忆整理/定时任务”。已知 AutoDream 结果应格式化为“自动记忆整理完成 · 扫描 2 个文件，新增 3 条记忆”；原始 body 放在展开区“技术详情”，默认摘要限制 2 行、13px/20px。

### P1-11 记忆页仍是 Markdown 文件管理器，不是用户可理解的“记忆”

**现状**：`qwenpaw-r7/memory-light.png` 的主要信息是 `2026-07-27.md`、`qwenpaw-shell-sandbox-and-pdf-conversion.md`、`782 B`、`3.6 KB`；`memory.ts` 的 `memoryDisplayName()` 只是裁掉目录前缀，保留 `.md` 和 slug。用户必须从文件名猜语义，字节数反而比记忆来源/内容摘要更显眼。  
**WB 做法**：`workbuddy/23-settings-memory.png` 直接展示一段可读的“工作背景”记忆、来源“来自对话的记忆”和更新时间；文件格式完全退到系统内部。  
**建议修法**：`MemoryView.tsx` 列表主行改为“标题 + 1 行摘要 + 来源/更新时间”，文件名、路径、字节数移到详情抽屉的技术信息区。短期可在 `memoryDisplayName()` 去 `.md`、把 slug 转为自然标题；完整修法应让 memory list API 返回 frontmatter/title/summary/source。行高建议 60–68px，标题 14px/20px medium，摘要 12–13px/18px，元信息 11px；左侧分组仍可保留。

## P2：打磨项

### P2-1 中文标点和术语没有经过 UI 文案校对

**现状**：`home-light.png` placeholder 为“今天要做些什么? @ 引用文件, / 调用技能”，问号和逗号都是半角；`crons-light.png` 模板说明也直接显示 i18n 中的半角逗号。源码 `i18n.ts` 的多条中文 prompt 使用 `,`，同时混用 `agent`、`Provider`、`Prompt`、`tokens`。这不是用户生成内容，而是产品自带中文。  
**WB 做法**：`workbuddy/01-home-office-mode.png` 使用“今天帮你做些什么？@ 引用对话文件，/ 调用技能与指令”这类全角中文标点；`19-settings-system.png` 的术语也保持中文界面语气一致。  
**建议修法**：集中校对 `app/src/lib/i18n.ts` 中文字典：`?/,/:` 分别改 `？/，/：`；展示层用“智能体、服务商、任务指令、上下文窗口 272,000 tokens”（若保留行业词则全局统一）。placeholder 建议“今天想完成什么？@ 引用文件，/ 调用技能”，去掉 `@`、`/` 前后不必要的半角空格，长度控制在一行。

### P2-2 技能行能点开抽屉，但外观没有稳定的可点击暗示

**现状**：`skills-light.png` 的技能条目看起来像静态信息行：无 chevron、无卡片边界、无操作文字，只有悬停后才出现的背景。`SkillRow` 还是 `div role="button"`，只有 `hover:bg-fill-hover` / `focus-visible:bg-fill-active`；相比 `MemoryView`、`InboxView` 同类可展开行都有 Chevron，交互语法不统一。  
**WB 做法**：`workbuddy/06-sidebar-experts.png` 的每个专家/技能是有明确边界、内边距和标签的完整卡片，点击范围与视觉范围一致。  
**建议修法**：`SkillRow` 改原生 `<button>`，末尾加入 14px ChevronRight；hover 同时 `bg-fill-hover` + chevron 从 `ink-muted` 提至 `ink-secondary`，active 使用 `bg-fill-active`，120ms；键盘 focus 使用 2px ring，而不是只换底色。若不加 chevron，则至少用 1px 透明边框，hover 时转 `border-line`，让可点击范围可见。

### P2-3 侧栏底部 W4 落成了“版本角标”，没有形成 WB 的身份/工作区锚点

**现状**：`home-light.png`、`chat-dark.png` 左下只显示极小的 `QwenPaw v0.1.0`，旁边是主题与设置；源码是 12px 应用名 + 11px 版本号。版本信息不是用户每天需要确认的上下文，也无法回答“当前以谁/在哪个工作区工作”。  
**WB 做法**：`workbuddy/01-home-office-mode.png`、`10-task-conversation.png` 左下以 28px 头像 + 用户名 `pal_xu` 作为稳定锚点，通知/帮助位于右侧；版本号放在侧栏上方品牌弱信息中。  
**建议修法**：`Sidebar.tsx` 底栏主信息改为当前工作区/本地身份（32px 头像或工作区图标 + 13px 名称），版本移到设置“关于”或 hover tooltip。若产品明确没有账号，至少显示“默认工作区”及路径摘要；底栏高度控制 44px，避免为版本号占 49px。

### P2-4 会话、技能、记忆三套图标语法没有完全收敛

**现状**：侧栏和页面操作使用 Lucide 16px；技能页使用 `text-lg` emoji；记忆列表是 16px Lucide + 32px 无描边底托；收件箱未读使用 8px 实心圆；工具行同时出现 12px chevron、12px terminal、13px check。单个组件内尚可，但跨屏看是“线性图标、彩色 emoji、状态点、重复 check”四套重量。`skills-light.png` 和 `chat-light.png` 最明显。  
**WB 做法**：`workbuddy/06-sidebar-experts.png`、`10-task-conversation.png` 中同层图标保持近似尺寸与线重；彩色头像只用于人物/品牌，工具与操作保持单色线性。  
**建议修法**：建立三档：导航/操作 16px、行内状态 12–13px、内容对象 18px；统一 Lucide `strokeWidth=1.75`。emoji 只用于真实品牌/人物；普通技能用图标 registry。P0-2 聚合后移除每条成功工具末尾的 check，仅在聚合行保留一次完成状态。

## 逐屏覆盖与审查维度核对

| 屏幕 | 主要问题 |
|---|---|
| Home | P1-2 过宽/标题过大；P1-3 composer 过圆、阴影过重；P2-1 文案标点 |
| 会话页 | P0-1 无页头；P0-2 执行管线泄露；P1-1 产物卡覆盖失败 |
| 侧栏 | P1-5 密度与宽度；P2-3 身份锚点错误 |
| 设置 | P0-4 深色遮罩反向；P1-6 面板尺寸/表面；P1-7 说明文字对比度；P1-8 按钮形状 |
| 定时任务 | P1-4 页头层级；P1-9 双 CTA/巨型空态 |
| 技能 | P0-3 内部 ID/英文/emoji；P2-2 点击暗示 |
| 收件箱 | P1-10 原始后台事件文案 |
| 记忆 | P1-11 文件系统心智而非记忆心智 |

| 审查维度 | 已覆盖条目 |
|---|---|
| 排版标尺 | P1-2、P1-4、P1-7、P2-1 |
| 间距节奏 | P1-2、P1-5、P1-6、P1-9 |
| 色彩层次 | P0-4、P1-3、P1-6、P1-7 |
| 形状语言 | P1-3、P1-8、P1-9 |
| 图标 | P0-3、P2-4 |
| 信息密度与布局 | P0-1、P0-2、P1-4、P1-5、P1-9、P1-11 |
| 微交互 | P2-2；另见 P1-3 的 focus ring 建议 |
| 文案 | P0-3、P1-10、P1-11、P2-1 |

## 最影响“第一眼观感”的 Top 3

1. **会话页仍然展示执行管线和本机绝对路径（P0-2）**  
   会话是核心产品面。用户第一眼看到的不是答案，而是 shell、Glob Search、skill ID 和 `/Users/liuxu`，这会立即把“办公助理”降级成“开发者调试控制台”。它对成熟度的伤害远大于某个圆角差 2px。

2. **产品展示层缺失，技能/收件箱/记忆直接透传后端 schema（P0-3、P1-10、P1-11）**  
   `browser_cdp`、英文 `success`、`Auto-memory result`、`.md + KB` 分别来自三个页面，说明这不是单页漏翻译，而是架构上缺少 presentation model。只修字面翻译不够，必须把内部标识、自然语言摘要和技术详情分层。

3. **深色遮罩反向 + composer 过圆过重，表面层级在最显眼组件上失真（P0-4、P1-3）**  
   WB 的高级感主要靠轻微表面差和克制投影；QwenPaw 却在深色弹层把背景漂白，在 composer 周围制造黑色光晕。两处都位于首屏视觉中心，即使功能齐全，也会一眼显得“假”和“重”。
