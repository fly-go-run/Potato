# 任务:r10 修复 —— 机械包(四个文件)

你负责以下四个文件的修复。**只准修改这四个文件**:`app/src/views/SkillsView.tsx`、`app/src/views/MemoryView.tsx`、`app/src/components/layout/ChatSearchDialog.tsx`、`app/src/components/layout/Sidebar.tsx`。**严禁修改 `app/src/lib/i18n.ts`**(所需 key 已全部预置,列在下方)和其他任何文件——其他文件正被并行修改,动了会冲突。

可用的新工具(已存在,直接 import):
- `app/src/lib/errorPresentation.ts`:`presentError(error)` → `{summaryKey, detail}`。用法:`const p = presentError(err); setError(t(p.summaryKey))`,detail 放 title 属性或折叠的技术详情。
- `app/src/lib/skillPresentation.ts`:`skillDescription(name, fallback)` → 中文一句话描述。

预置 i18n key(直接用):common.retry、common.technicalDetail、error.*(network/auth/notFound/rateLimited/server/generic)、skills.loadFailedTitle、skills.installCancelled({name})、skills.import.workspace、skills.import.workspaceHint、memory.discardTitle、memory.discardDescription、memory.discardConfirm、sidebar.actionFailed({message})。

## SkillsView.tsx

1. 【描述人话化】技能卡与抽屉里展示的 description 一律经 `skillDescription(skill.name, skill.description)`。
2. 【图标一致】技能卡图标:`skill.emoji` 存在时显示 emoji(和设置页内联列表一致),否则保留现有图标。
3. 【三态互斥】loading / 加载失败 / 就绪三态互斥:首次加载失败时**只**显示错误块(skills.loadFailedTitle + presentError 概括 + common.retry 按钮重新拉取),不得同时渲染"还没有技能"空态或空网格。技能与插件两个 tab 都要。
4. 【取消安装反馈】Hub 安装取消后给出明确反馈:顶部 notice 显示 skills.installCancelled({name}),被取消的条目允许重新安装。
5. 【导入目标工作区】从技能池导入时:workspaces 长度为 1 保持现状;大于 1 时在导入确认 UI 中列出工作区供选择(显示可读名称,不显示裸 agent_id;默认不选中,未选不可提交),提示用 skills.import.workspaceHint。

## MemoryView.tsx

6. 【脏确认】编辑抽屉中 `draft !== content` 时,Esc/遮罩/关闭按钮/取消 都必须先弹确认(用 `components/ui` 的 ConfirmDialog:memory.discardTitle/memory.discardDescription/memory.discardConfirm,tone danger),确认后才关闭丢弃。
7. 【详情失败可重试】详情读取失败时,错误信息旁提供 common.retry 按钮重新加载该条,并保留关闭出口。
8. 【三态互斥】列表首次加载失败只显示错误 + 重试,不渲染空态。

## ChatSearchDialog.tsx

9. 【设置入口统一】面板里"打开设置"改为 `navigate("/settings", { state: { background: location } })`(useLocation 取当前 location),与侧栏入口一致。
10. 【hover 同步】结果项 `onMouseEnter` 同步 `selectedIndex`,保证键盘选中与鼠标 hover 永远只有一个高亮,Enter 执行当前高亮项。

## Sidebar.tsx

11. 【操作反馈】会话改名/删除:await 结果,失败时**保留弹层**并就地显示 sidebar.actionFailed({message: presentError 概括的 t 文案});成功才关闭。置顶/取消置顶失败时用同样文案给出可见反馈(可用简单的临时提示行)。若 store 方法不抛错,检查调用后 store 的 error 状态。
12. 【命名】侧栏"搜索会话"入口的文案 key `sidebar.searchChats` 不改 key,但你**不要动** i18n——该项由他人处理,跳过文案本身,只确保无回归。

## 验收

- `cd app && npx tsc --noEmit` 干净;`npx vitest run` 全过(133+)。
- 不新增依赖。完成后输出:逐项完成状态(1-12)+ 涉及行号 + 你自查发现的风险。
