# W5:定时任务页预置模板卡

参照 `.reference-shots/workbuddy/07-sidebar-automation.png`(WorkBuddy 自动化页):空态下方有一个"自动化任务模版"网格,每张卡=图标+标题+一行描述,点击即预填新建表单。给 QwenPaw 的定时任务页(`app/src/views/CronsView.tsx`)做同样的模板区。

## 要求

1. 先读 `app/src/views/CronsView.tsx`,弄清现有"新建任务"表单的字段与打开方式(名称/调度/提示词等),模板点击后**打开新建表单并预填**这些字段,不直接创建任务。
2. 模板区渲染在页面主体下半部:标题行「任务模板」(i18n),网格 3 列(<lg 2 列,<sm 1 列)。**无论列表是否为空都显示**(有任务时排在列表之后)。
3. 卡片样式完全照现有工程惯例:语义类(`border-line bg-surface rounded-[var(--radius-md)] shadow-[var(--shadow-sm)]`,hover 用 `hover:border-line-strong`),布局参考 `SkillsView.tsx` 的网格卡。图标用 lucide-react,`size={16}`,色用 `text-ink-tertiary`。**禁止改 `tokens.css`、禁止硬编码色值、禁止新增依赖。**
4. 八个模板(图标/名称/调度/提示词全部按下表,不要自行发挥):

| lucide 图标 | 名称(zh) | cron | 提示词(zh) |
|---|---|---|---|
| FileText | 每周工作周报 | `0 17 * * 5` | 汇总我本周的工作内容,按项目分组整理成周报草稿,列出进展、风险与下周计划。 |
| CalendarClock | 会议前准备 | `30 9 * * 1-5` | 检查我今天的会议安排,为每个会议整理议题、相关背景材料和需要确认的问题。 |
| Newspaper | 每日资讯摘要 | `0 9 * * 1-5` | 收集当天与我工作领域相关的重要资讯,精选 5 条,每条一句话摘要加链接。 |
| ListChecks | 周五待办清点 | `0 16 * * 5` | 盘点我本周未完成的待办事项,标出已过期的,给出下周优先级建议。 |
| Mail | 邮件整理提醒 | `0 18 * * 1-5` | 提醒我处理今天未回复的重要邮件,并整理一份待回复清单。 |
| BarChart3 | 月度数据报告 | `0 10 1 * *` | 生成上个月的工作数据汇总报告,包含关键指标变化和趋势分析。 |
| BookOpen | 每日学习卡片 | `0 8 * * *` | 挑一个与我工作相关的知识点,用 200 字讲清楚,附一个实际应用例子。 |
| Archive | 文件归档整理 | `0 17 * * 5` | 检查我本周产生的文档和文件,按项目归类,列出建议归档或清理的清单。 |

5. 每个模板的英文名称/提示词也要给(i18n 双语,键加在 `app/src/lib/i18n.ts` 定时任务相关键位附近;英文由你翻译,直译即可)。
6. 描述行 = 提示词截断一行(`line-clamp-1` 或 truncate,照工程现有做法)。
7. 只许改 `CronsView.tsx` 和 `i18n.ts` 两个文件。其他文件(尤其 Composer/ChatView/Sidebar/MessageList/tokens.css)一律不碰。
8. 完成标准:`cd app && npx vitest run` 全过。**不要跑 `npm run build`**(有并行工作流,构建产物会互相踩,最终构建由架构方统一做)。不要 commit。
9. 输出:改动摘要 + 你自测过的交互路径说明。
