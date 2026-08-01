# WorkBuddy vs QwenPaw 深度差距审查 r2(独立审查员)

- 日期:2026-07-28
- 参照:`.reference-shots/workbuddy/`(32 张,1061×768)
- 被审:`.reference-shots/qwenpaw-r7/`(10 张,1280×860)+ `app/src/` 当前代码
- 前一轮:`gap-analysis-workbuddy-r1.md`(W1–W8 已落地,本文不复述已修项,只报**仍在的 / 新引入的 / 修得不到位的**)
- 方法:截图像素采样(PIL)+ 源码取值。所有数字均为实测,不是目测。

## 0. 总判断(与 r1 的分歧)

r1 说"差距集中在首页产品感、composer 结构、会话动作行、侧栏层次",这四处结构上确实补齐了。但 r1 **漏掉了两个比它们更致命的层面**:

1. **密度**。QwenPaw 所有 UI 行比 WorkBuddy 高 **40%**(实测:侧栏导航 38.2px vs 26.8px、会话列表 37.8px vs 25.0px、设置导航 40.8px vs 27.0px),字号只差 1px(14 vs 13)。根因是 `global.css` 给 `body` 设了 `line-height: 1.6`,这条正文行高被**每一个 chrome 控件继承**。这是"看起来像网页而不像桌面 app"的单一最大来源,r1 一个字没提。
2. **会话页的首屏**。QwenPaw 会话页**没有顶栏**,内容直抵窗口 y=0(`chat-light.png` 顶部用户气泡被切掉一半),而首屏可见区被 8 行未归并的工具日志(截断的绝对路径)占满。WorkBuddy 同一场景:44px 顶栏 + 一行"已完成 1m14s ›"。r1 说"工具卡完成态已改安静行,方向对"——方向对,但**没做归并**,8 条安静行的总噪声比 1 张卡片更大。

色板方向 r1 定得对(实测 QP 浅色 `#FAFAFA/#F2F2F4/#FFFFFF` 与 WB `#FAFAFA/#F2F2F2/#FFFFFF` 几乎重合),但深色的 elevation **抄反了一半**,且整套中性色带蓝紫偏色(见 P1-3)。

---

## P0 — 硬伤,一眼假(4 条)

### P0-1 会话页没有顶栏,内容顶到窗口边缘,且全程看不到会话标题

**现状**:`qwenpaw-r7/chat-light.png` / `chat-dark.png` —— 最上面那条用户气泡在 y=0 处被横切,上方没有任何 chrome。整个会话页从头到尾没有会话名、没有会话级操作入口。代码:`views/ChatView.tsx:191-295` 直接是 `<div className="relative flex h-full flex-col">` + 滚动容器,`components/layout/AppShell.tsx:111` 的 `<main>` 也不含 header;`MessageList` 只有 `pt-6`(`MessageList.tsx:30`),24px 之后就是消息。

**WB 做法**:`10-task-conversation.png` / `31-dark-task.png` —— 主区顶部固定 44px 栏,左侧任务标题「Word转PDF文件转换」(y=22,15px 半粗),右侧 4 个 16px 图标(搜索/分享/历史/右侧产物面板),下方一条 `--line` 分隔,正文从 y=45 起,滚动时正文从栏下穿过而不是从窗口边缘穿过。

**建议修法**:在 `ChatView` 顶部加 sticky header(不要放进 `AppShell`,首页空态不需要它):
```
<header className="sticky top-0 z-10 flex h-11 shrink-0 items-center gap-2 border-b border-line bg-canvas/85 px-4 backdrop-blur-sm">
```
左:`text-[13px] font-medium text-ink truncate` 显示 `chats.find(id).name`;右:`IconButton size="sm"` ×2(会话内搜索、产物汇总,后者见 P1-6)。macOS 壳下这条栏同时替代 `AppShell.tsx:82` 那条 28px 隐形拖拽带(给它加 `data-tauri-drag-region`)。`MessageList` 的 `pt-6` 保留即可。

### P0-2 连续完成的工具调用不归并,首屏被 8 行截断路径占满

**现状**:`chat-light.png` y=110–345,8 条等权"安静行"(Skill / 4×shell / Glob Search / shell / Send File To User),每条 `min-h-7`(28px)+ `my-0.5`,合计 **235px**,全部是 `font-mono text-[12px]` 的截断绝对路径(`/Users/liuxu/.qwenpaw/workspaces/default/media/5836793c5a6b46fca5bba5c22e9ab8f4_%E4%BA%AC…`)。8 行**没有任何视觉分组**,和后面的正文只隔 33px。代码:`MessageList.tsx:91-114` 逐条 message 渲染 `ToolCard`,`ShellToolCard.tsx:56-84` / `ToolCard.tsx:77-106` 各自输出独立 `<details>`,不存在归并逻辑。

**WB 做法**:`10-task-conversation.png` y=440 —— 整轮执行过程只有一行 `已完成 1m14s ›`(13px、`ink-tertiary`、可展开),占 20px。执行细节默认完全不占版面。

**建议修法**:在 `MessageList.tsx` 的 `AssistantTurn` 里把**相邻的** tool call/output 收集成一个 `ToolGroup`,组内 ≥2 条时渲染成单行摘要:`<ChevronRight/> 已完成 {n} 步 · {duration}`(`text-xs text-ink-tertiary`,`px-1.5 py-1`),展开后才逐条列出现有的安静行;组内只有 1 条时保持现状。耗时取组内首个 call 与末个 output 的时间差(`StreamMessage` 已带时间戳则直接用,没有就先只显示步数)。产物类工具(见 P1-7)**不入组**,始终独立成卡。

### P0-3 全局 `line-height: 1.6` 灌进所有 UI,行高整体虚胖 40%

**现状**(实测行距,像素扫描):

| 元素 | QwenPaw | WorkBuddy | 比值 |
|---|---|---|---|
| 侧栏导航行 | 38.2px | 26.8px | 1.43 |
| 侧栏会话行 | 37.8px | 25.0px | 1.51 |
| 设置左导航行 | 40.8px | 27.0px | 1.51 |
| 设置内容行 | ~68px | ~44px | 1.55 |
| 中文字形墨高 | 12–13px | 10–11px | 1.15 |

字号只差 1.15 倍,行高差 1.5 倍 → 差的是内边距和行高,不是字号。根因:`styles/global.css:20` 的 `line-height: 1.6` 作用在 `body`,于是 `Sidebar.tsx:79` 的 `px-3 py-2 text-sm` 算出来是 14×1.6 + 16 = **38.4px**;WB 同类行是 13×1.35 + 9 ≈ 27px。同一条 token 同时服务正文和控件,是这套系统里最贵的一个决定。

**WB 做法**:控件行高 1.3–1.4,正文行高 1.7+,两者分开(`10-task-conversation.png` 的正文行距 24px@13px = 1.85,而左栏列表行 25px 含 padding)。

**建议修法**(需架构方改 `global.css`,`tokens.css` 可不动):
1. `body { line-height: 1.5 }`;
2. `Markdown.tsx:12` 已经显式写了 `leading-[1.75]`,不受影响;`MessageList.tsx:57` 用户气泡的 `leading-[1.7]` 也显式;所以正文零回归;
3. 给密集行显式压到 `leading-5`(20px)并把 `py-2` → `py-1.5`:`Sidebar.tsx:79/87/104/122/139/156`(导航)、`Sidebar.tsx:321`(会话行)、`SettingsView.tsx:395`(设置导航)。目标:导航行 32px、会话行 30px、设置导航行 30px——不必做到 WB 的 27px,但 38px 必须下来。

### P0-4 首页整屏没有任何视觉锚点:发送键在空态被灰成"坏掉了"

**现状**:`home-light.png` / `home-dark.png` —— 主区从上到下:32px 黑标题、13px 灰副行、6 个白底描边胶囊、白色 composer、灰色底座。**没有一个深色/彩色元素**。右下发送键因为 `canSend=false` 走 `disabled:bg-fill-active disabled:text-ink-muted`(`Composer.tsx:43`),浅色下是 `#E7E7EC` 底 + `#9A9AA2` 箭头,深色下是 `rgba(255,255,255,0.09)` 底 + `#6D6D76` 箭头——两个主题下都读作"控件坏了"。产品最强的签名控件在**默认状态下不可见**。

**WB 做法**:`01-home-office-mode.png` / `30-dark-home.png` —— 输入框空着时,右下圆形发送键依然是**实心近黑 + 白箭头**(深色下近白 + 深箭头),并且左侧「日常办公」段控是黑色实心药丸。整屏恰好两个黑块,视觉重心立刻成立。r1 的 §1 自己写了"主按钮统一近黑做唯一视觉锚点",但落地时被 disabled 态吃掉了。

**建议修法**:改 `Composer.tsx:39-45` 的 `sendButtonClass`,去掉 `disabled:bg-fill-active disabled:text-ink-muted`,改为始终 `bg-btn-primary text-btn-primary-ink`,禁用时只加 `disabled:opacity-45 disabled:pointer-events-none`(保持是"暗但在",不是"灰但没了")。若担心"可点错觉",可保留 `cursor-default`。这是本轮成本最低、观感收益最高的一处。

---

## P1 — 明显差距(10 条)

### P1-1 阅读列 832px 过宽,composer 同宽 → 首页显空、会话显散

**现状**:`MessageList.tsx:30`、`Composer.tsx:264`、`ChatView.tsx:53` 全部 `max-w-[52rem]` = **832px**。实测 `home-light.png` composer 托盘 x=356→1180(824px);`chat-light.png` 正文列 x=383→1160(777px),15px 中文 → **约 52 字/行**。

**WB 做法**:`01`/`10` 图 composer 与正文列均为 x=322→948 = **626px**,13–14px 中文 → 约 43 字/行。占主区宽度 73%(QP 是 81%)。

**建议修法**:三处统一改 `max-w-[46rem]`(736px,约 46 字/行);同时把 `PageHeader.tsx:44` 的 `PageContainer` 的 `reading` = `max-w-3xl`(768px)对齐到同一值,消掉"会话 832 / 阅读页 768 / 宽页 1024"这套非等比的三档。宽度是**唯一**能同时改善"首页空旷"和"会话难读"的参数。

### P1-2 深色 composer 的层级方向与 WB 相反,底座像漏光而不是托盘

**现状**(像素采样 `home-dark.png`):画布 `#131316` → 底座 `#1A1A1F` → 内卡 `#1F1F25`。底座比内卡**暗**,比画布只亮 7 阶,三层糊成一坨,`shadow-md`(纯黑阴影)在深色画布上完全不产生高度。

**WB 做法**(采样 `30-dark-home.png`):画布 `#141414` → composer 面板 `#1F1F1F` → **底栏 `#292929`(比面板更亮)**。浅色下则相反(面板 `#FFFFFF` → 底栏 `#F4F4F4`)。即:底栏永远是"离用户更近的一层 chrome",浅色靠压暗、深色靠提亮来表达。

**建议修法**:需架构方在 `tokens.css` 增一个 `--composer-tray` 语义值(浅 `#f2f2f4`,深 `#292930`),`Composer.tsx:277` 的 `bg-bg` 换成它;深色下同时把内卡的 `shadow-[var(--shadow-md)]` 换成 `shadow-none` + `border-line-highlight`(黑阴影在近黑背景上是无效像素)。

### P1-3 中性色整体带蓝紫偏色,WB 是纯灰

**现状**(采样):`#131316`、`#1B1B20`、`#1F1F25`、`#292930`、`#F2F2F4`、`#17171A` —— 每个色的 B 通道比 R 高 3–6。深色下四层叠起来是**可见的冷紫调**。

**WB 做法**:`#141414`、`#1F1F1F`、`#292929`、`#F2F2F2` —— R=G=B,严格中性;只留绿色状态点和紫色品牌胶囊两处彩色。

**建议修法**:需架构方改 `tokens.css`。要么彻底中性化(把六个中性值的 B 通道拉平),要么把偏色**做足**成有意的品牌冷调(现在 3–6 的偏移量既不像纯灰也不像有色,是最尴尬的中间值)。我的建议是中性化,理由:QwenPaw 的 accent 已经是蓝(`#2563E0`),背景再偏蓝会让 accent 失去"这是状态色"的辨识度。

### P1-4 空态被塞进大描边盒,而且同一个主按钮在一屏里出现两次

**现状**:`crons-light.png` —— 页头右上「+ 新建任务」(深色实心),下方一个 **286px 高、带 `border-line` 描边、`bubble-tool/60` 底**的盒子,盒子中间又是一个**一模一样**的「+ 新建任务」深色实心按钮。代码:`CronsView.tsx:215-223`(页头 action)与 `CronsView.tsx:244-256`(EmptyState action)用的是同一个 `variant="primary"` + 同一个 `t("crons.new")`;盒子来自 `EmptyState.tsx:24`(`border border-line bg-bubble-tool/60 px-6 py-16`)。

**WB 做法**:`07-sidebar-automation.png` —— 空态**没有容器**,直接在画布上:线稿闹钟插画 → 一行「开启你的第一个自动化任务吧」→ 一个黑色「+ 添加自动化」。整页只有这一个主按钮;页头位置放的是「定时任务 / 运行记录」段控,不是重复 CTA。

**建议修法**:(a) `EmptyState.tsx:24` 去掉 `border border-line bg-bubble-tool/60`,只留 `px-6 py-16 text-center`,让空态浮在画布上;(b) 有空态时隐藏页头的 primary(`CronsView.tsx:215` 包一层 `jobs.length > 0 &&`),或反过来去掉空态里的按钮——**一屏一个主操作**;(c) 图标托盘 `h-11 w-11` + `size={20}` 视觉重量太轻(WB 插画约 56px),放大到 `h-14 w-14` + `size={26}`,或去掉圆托盘直接放 32px 线稿图标。此改动同时影响 crons/inbox/skills/memory 四个页面。

### P1-5 页头是营销页排版:26px 标题 + 副标题 + 32px 下边距,吃掉 100px

**现状**:`PageHeader.tsx:18-21` —— `text-[26px] leading-9 font-semibold tracking-tight` + `mt-1.5 text-sm` 副标题 + `mb-8`,配合 `PageContainer` 的 `py-8`,页面前 **~100px** 全是标题区。`crons-light.png` 里正文(空态盒)从 y=126 才开始;`inbox-light.png` 更极端——标题区 100px,内容只有 240px,剩下 500px 全空。

**WB 做法**:`07`/`19`/`24` —— **没有页面大标题**。页面身份由左上 13px 段控(自动化页)或模态标题(设置)承担,内容从 y≈45 就开始。桌面 app 的页面身份来自导航选中态,不需要在内容区再喊一遍。

**建议修法**:`PageHeader` 降一档 —— `text-[19px] font-semibold leading-7` + `mb-6`;副标题只在**首次/空态**显示(`subtitle` 传参时加 `showSubtitle={isEmpty}`),有数据时删掉(「查看定时任务与后台运行推送的结果」这类句子对第二次访问的用户是零信息)。`PageContainer` 的 `py-8` → `py-6`。

### P1-6 消息动作行只有 2 个图标,没有用量/模型元信息,也没有产物汇总入口

**现状**:`chat-light.png` y=594 —— 一轮回答结束后只有复制 + 重新生成两个 14px 灰图标,左对齐,右侧一片空。代码 `MessageList.tsx:158-176`。

**WB 做法**:`10-task-conversation.png` y=308/588 —— 动作行是 `复制 赞 踩 朗读 重新生成 分享 ⋯` 七个 14px 图标,**右侧同一行**跟 `共消耗 ◇16.98` 和 `⊕ Auto`(模型档),动作行**上方**还有一行 `查看所有产物 (1) ›`。这一行同时承担"操作"和"这次花了多少/用了谁"的交代。

**建议修法**:(a) 在 `MessageActions` 的 `flex` 里加 `<div className="flex-1"/>` + 右侧 `text-[11px] text-ink-tertiary` 显示本轮 `stream.turnUsage` 的 token/费用与 `activeModel.active_llm`(数据 store 里已有,`Composer.tsx:427` 已经在用 `context_usage_ratio`);(b) 动作行上方加 `查看所有产物 (n) ›`,n>0 时才出现,点击打开右侧产物抽屉或滚动定位——这是 P0-1 顶栏右侧那个图标的落点。

### P1-7 真正的"交付文件"不走产物卡,掉回安静行

**现状**:`chat-light.png` y=334 —— `Send File To User  file_path: /Users/liuxu/.qwenpaw/workspaces/default/output/京东-xLLM-images.zip`,和前面 7 条 shell 一样是 12px 灰安静行。而 `FileToolCard.tsx:27` 的 `ARTIFACT_TOOLS` 只有 `write_file` / `append_file`,`FILE_TOOL_TITLES`(:19)也不含它。后端确有这个工具:`src/qwenpaw/agents/tools/send_file.py:28 async def send_file_to_user(...)`,参数名 `file_path`(见 `security/tool_guard/guardians/file_guardian.py:27`)。**产物卡组件本身做得不错(`ArtifactCard`,:124-175),只是没接到真正的交付路径上。**

**WB 做法**:`10-task-conversation.png` y=493–538 —— 满宽浅灰卡、24px 彩色文件类型图标、13px 半粗文件名、12px 大小(218.8 KB)、右侧打开箭头。

**建议修法**:`FileToolCard.tsx` 里给 `FILE_TOOL_TITLES` 加 `send_file_to_user: "tool.file.deliver"`(新增 i18n 文案「已发送文件」),并加入 `ARTIFACT_TOOLS`;`parseArguments` 已经读 `file_path`,`fileSizeLabel` 匹配不到 bytes 时回落到 `directoryOf`,可再加一条从 `pair.result` 里取大小的规则。同时把产物卡从 P0-2 的归并组里排除。

### P1-8 技能页图标语言分裂,内容全是英文 id

**现状**:`skills-light.png` —— 17 行里,`📇🔌🖥️📩💬⏰🍵📄🧭📧✍️🗺️🥟` 是彩色 emoji,`docx/pdf/pptx/xlsx` 四行是黑色 `✦` 字形(`SkillsView.tsx:341`: `{skill.emoji || "✦"}`)。同一列里彩色 emoji 和单色字形并排,是最典型的"图标没成体系"。名称是 `QA_source_index`、`browser_cdp`、`chat_with_agent` 这类 snake_case 英文 id,描述是英文原句被 `line-clamp-1` 截断("Use this skill when the user explicitly wants to connect t…")。启用态用一个**裸 `Check` 图标**(:351)表示,没有任何控件语义。

**WB 做法**:`06-sidebar-experts.png` / `07` —— 卡片图标统一是单色线稿(同一套 stroke),标题中文,描述中文一句话。

**建议修法**:(a) 无 emoji 时不要用 `✦`,改用 `<Blocks size={18} className="text-ink-tertiary"/>`,与 `PluginRow` 的 `<Puzzle size={18}/>`(:376)成一套;更彻底的做法是**全部**用单色线稿,emoji 只在详情页显示;(b) 名称走 `ToolCard.tsx:206` 已有的 `humanToolName()`(`_`→空格 + 首字母大写),至少让 `QA_source_index` 变成 `QA Source Index`;(c) 描述 `line-clamp-1` → `line-clamp-2`,一行截断在英文长句上等于没信息;(d) 启用态换成 `Switch`(`components/ui/Switch.tsx` 已存在),并去掉整行 `opacity-55`(:338)——整行降透明会把 13px 说明文字压到对比度下限以下。

### P1-9 用户气泡是 682px 宽的灰板

**现状**:`MessageList.tsx:57` `max-w-[82%]`,在 832px 列里 = **682px**;`chat-light.png` 里那条气泡 x=517→1160(643px)、三行、`rounded-bubble`(18px)、`px-4.5 py-3`。因为路径类内容不换行友好,气泡几乎横贯整列,右对齐的意义被抵消。

**WB 做法**:`10-task-conversation.png` y=361 —— 用户气泡 x=605→948 = **343px**,单行,内含文件 chip + 文字,圆角约 10px,`#ECECEC` 底。

**建议修法**:`max-w-[82%]` → `max-w-[70%]`,`rounded-bubble`(18px)→ `rounded-[var(--radius-md)]`(10px);长路径类内容加 `break-all` 已有,但配合窄气泡效果才对。

### P1-10 中文标点半角混用(13 处,其中 8 处在首屏可见文案上)

**现状**:`src/lib/i18n.ts` 中文块里 13 条字符串在中文语境用了半角 `,` / `?`,与同文件其它字符串的全角 `,。、` 混用:
- `composer.placeholder` = `今天要做些什么? @ 引用文件, / 调用技能` —— **首页和会话页都在显示**(`home-light.png` y=404)
- `crons.templates.*.prompt` 共 8 条,如 `汇总我本周的工作内容,按项目分组整理成周报草稿,列出进展、风险与下周计划。` —— 半角逗号和全角顿号在**同一句**里(`crons-light.png` 模板卡描述,肉眼可见字距不齐)
- `composer.trigger.noFiles` = `本会话还没有文件,发送附件后可在这里引用`

**WB 做法**:`01-home-office-mode.png` placeholder = `今天帮你做些什么？ @ 引用对话文件，/ 调用技能与指令` —— 全角问号、全角逗号,标点自带的半个字宽让 `@` 和 `/` 天然分组。

**建议修法**:批量替换这 13 条里的 `,`→`，`、`?`→`？`(数字/时间里的 `9:00` 冒号保持半角,代码/路径不动)。另外 `composer.placeholder` 的措辞也弱一档:WB 的「今天帮你做些什么」是**服务方视角**,QP 的「今天要做些什么」是**命令用户**,建议对齐成「今天帮你做点什么？@ 引用文件，/ 调用技能」。

---

## P2 — 打磨项(9 条)

### P2-1 同一个"相对时间"有两套实现,渲染结果不一致
`home-light.png` 侧栏是 `16 小时前`(数字与单位间**有空格**,来自 `lib/relativeTime.ts` + i18n `"time.hoursAgo": "{count} 小时前"`),`memory-light.png` 是 `17小时前`(**无空格**,来自 `lib/memory.ts:81 formatRelativeTime` 用 `Intl.RelativeTimeFormat`),`inbox-light.png` 又是绝对时间 `2026年7月27日 23:02`(`InboxView.tsx:348`)。三个列表页三种时间语言。WB 全站相对时间且格式统一(`24分钟前`/`3小时前`/`35天前`,无空格)。建议:删掉 `memory.ts` 里的那套,全部走 `lib/relativeTime.ts`;i18n 去掉 `{count}` 后的空格(中文数字与单位间不加空格);Inbox 列表改相对时间,绝对时间放展开态。

### P2-2 `SegmentedControl` 的 track 变体内圆角大于外圆角
`SegmentedControl.tsx:66` 外层 `rounded-[10px] p-0.5`(2px 内边距),`:96` 选中项 `rounded-lg`。因为 `tokens.css` 把 `--radius-lg` 重定义成 14px,`rounded-lg` = 14px > 外层 10px − 2px = 8px,选中药丸的圆角会从轨道里"鼓"出来。用在设置-外观的主题三选一上。改成 `rounded-[8px]`。

### P2-3 Composer 用了两个脱离圆角标尺的硬编码值
`Composer.tsx:277` `rounded-[24px]`、`:278` `rounded-[18px]`,而标尺是 8/10/14/18(`tokens.css:61-64`)。`tokens.css` 开头写着"组件代码一律使用语义类,禁止硬编码色值",圆角同理。建议加 `--radius-xl: 22px` 到 token 层(需架构方),内卡用 `rounded-bubble`(18px,与用户气泡同族)。

### P2-4 收件箱状态徽标直出后端英文枚举
`inbox-light.png` 两条记录都挂着绿色 `success` 徽标。`InboxView.tsx:342` `return <Badge tone={tone}>{status}</Badge>` —— 未过 i18n。中文界面里出现小写英文枚举是最典型的"没做完"信号。建议加 `inbox.status.success/error/running` 三条文案。同页 `来源：memory` 里的 `memory`(`t("inbox.source", {source: event.source_type})`)同理。

### P2-5 技能页两种列表行内边距并存
`SkillsView.tsx:339` SkillRow 是 `px-3 py-3.5`,`:373` PluginRow 是 `px-5 py-4`,两者在同一页的两个 tab 下、同一个容器里切换显示 —— 切 tab 时左边距会跳 8px。统一成 `px-4 py-3`。

### P2-6 记忆页是偏心两栏,和全站任何一页都不同构
`memory-light.png` —— 左侧 135px 一列极小的分组标签(`日记 / 2 项`,12px),右侧 554px 的卡片列表,右边留 169px 空白。整页视觉重心偏左上,扫描路径要横跳。全站其它页都是"满宽单列"。建议:分组标题改成常规的**行上标题**(`text-[13px] font-medium text-ink-secondary mb-2`,与 crons 的「任务模板」一致),列表卡满宽 —— 一次改动就让四个列表页同构。

### P2-7 空态/胶囊/卡片的"描边 vs 阴影"用法不统一
首页胶囊(`ChatView.tsx:60`)同时有 `border-line` + `shadow-[var(--shadow-sm)]`;WB 胶囊(`01` 图采样:白底 `#FFF` 直接压在 `#FAFAFA` 上)只有极细描边、无阴影。`Card.tsx:14` 是 `border + inset 高光`(无外阴影),`EmptyState` 是 `border + 填充底`,`ArtifactCard`(`FileToolCard.tsx:140`)是**无描边纯填充**。四种卡片语汇。建议定一条规则并写进 `tokens.css` 注释:**画布上的静态容器只用描边;需要"浮起"的只有 composer 和弹层,才给阴影;填充底只用于工具/产物这类"嵌入正文"的块**。按此,首页胶囊去掉 `shadow-sm`。

### P2-8 设置面板固定 `85vh`,短分区留出 460px 空腔
`SettingsView.tsx:368` `sm:h-[85vh]`,在 860px 视口是 731px;而「模型」分区内容只有 ~270px(`settings-light.png` 内容止于 y=395,面板到 y=793)。WB 设置模态是 565/768 = 73%,且内容基本填满(`19-settings-system.png`)。建议 `h-[85vh]` → `max-h-[85vh] min-h-[28rem]`,让面板随内容收缩。

### P2-9 会话底部缺"AI 生成内容"免责脚注,且 composer 贴底只剩 20px
`chat-light.png` composer 底座下沿 y=838,窗口 860,只剩 22px(`Composer.tsx:255` `pb-5`)。WB(`10` 图)composer 下沿 y=740,下方 y=754 有一行 11px 极浅的「内容由 AI 生成，请核实重要信息」,把底部留白变成了有意的信息带。QP 把同等位置给了「上下文已用 5.6%」(放在底座**内部**右侧)。建议:底座下方加一行 `text-[11px] text-ink-muted text-center pt-2` 的免责/提示位;或者把「上下文已用」移到这一行,让底座内只留会话环境 chip。

---

## 逐屏结论

| 屏 | 最关键的一条 |
|---|---|
| 首页 | P0-4 无视觉锚点(发送键灰死)+ P1-1 composer 824px 过宽 |
| 会话页 | P0-1 无顶栏 + P0-2 八行工具日志未归并 |
| 侧栏 | P0-3 行高 38px(WB 25px);**其余达标** —— 分组计数/相对时间/底部锚点/收起态 r1 已补齐,结构与 WB 同级 |
| 设置 | P1-5 不适用(已是模态,做得对);实际问题是 P2-8 固定 85vh 的空腔 + 导航行 41px |
| 定时任务 | P1-4 空态描边盒 + 一屏两个相同主按钮 |
| 技能 | P1-8 彩色 emoji 与黑色 `✦` 混排、英文 id 直出 |
| 收件箱 | P2-4 `success` 英文枚举 + P1-5 页头吃掉 100px 而内容只有 240px |
| 记忆 | P2-6 偏心两栏与全站不同构 + P2-1 时间格式与侧栏不一致 |

---

## Top 3:最影响"第一眼观感"

**1. P0-3 行高虚胖 40%(根因:`global.css` 的 `body { line-height: 1.6 }`)**
排第一是因为它**同时**影响八个屏、且是唯一一处"改 1 行 + 6 处补 `leading-5`"就能让整个 app 换气质的改动。用户说"差距还是很大"但说不清哪里,通常就是这个:同样的组件、同样的色板,QwenPaw 一屏装 5 条会话,WorkBuddy 装 8 条;一个像后台管理系统,一个像桌面工具。字号差 1px 无所谓,行高差 13px 是结构性的。

**2. P0-1 + P0-2 会话页首屏(无顶栏 + 八行截断路径)**
排第二是因为**会话页是这个产品被看的时间最长的一屏**,而它的首屏当前是:窗口顶边切开一条灰气泡,下面八行等宽的 `/Users/liuxu/.qwenpaw/workspaces/default/media/5836793c…`。WorkBuddy 的同一屏是:标题栏、一行「已完成 1m14s」、正文、文件卡。这不是审美差距,是"谁是主角"的判断差距——r1 已经写对了结论("正文当主角"),但只做了降噪没做归并,八条安静行加起来一点也不安静。

**3. P0-4 + P1-1 首页的锚点与宽度**
排第三是因为首页是**第一眼字面意义上的那一眼**。当前状态:824px 宽的空白输入框、右下角一个灰掉的圆按钮、整屏零深色零彩色。WorkBuddy 同尺寸下是 626px 的输入面 + 一枚黑色发送键 + 一枚黑色段控,视觉重心一秒建立。这两条的修复成本几乎为零(一个 `max-w` 值、一段 `disabled:` 类名),但它们是"打开 app 的前 500 毫秒"里唯一起作用的东西。

---

## 附:本轮明确判定"已达标,不必再动"

- `TriggerPopover`(`/` 技能、`@` 引用)与 WB `16-slash-skill-trigger.png` 结构一致:标题带计数、单行 `名称 + 灰描述`、贴 composer 8px、键盘驱动。**做得比 WB 干净**(WB 那份没有加载态)。
- 浅色表面色阶:`#FAFAFA / #F2F2F4 / #FFFFFF` 对 WB `#FAFAFA / #F2F2F2 / #FFFFFF`,实测几乎重合,r1 的 §1 落地到位。
- 侧栏信息结构(分组计数 + 折叠、行尾相对时间 hover 让位给「⋯」、底部品牌 + 主题切换 + 设置)—— 交互密度已超过 WB,只差行高。
- `ArtifactCard` 的规格(20px 类型图标 / 13px 名 / 12px 大小 / 右侧打开)与 WB 文件卡对得上,问题只在**没接到 `send_file_to_user` 上**(P1-7)。
- 正文排版:15px / `leading-[1.75]`,与 WB 的 13px / 1.85 属同一档,不需要动。
