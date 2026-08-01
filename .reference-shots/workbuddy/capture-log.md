# WorkBuddy 桌面客户端设计参照采集日志

- 日期：2026-07-28
- 应用：WorkBuddy v5.3.5（Electron，账号已登录）
- 默认窗口截图尺寸：1061 × 768；窄窗截图：904 × 768
- 方法：Codex Computer Use（可访问性树定位 + 真实 UI click/type/drag）；每次操作后重新读取界面状态，再保存当前窗口截图
- 安全边界：只浏览已有页面和历史任务；未发送消息、未创建或删除任务/项目/自动化、未授权连接器、未修改模型或安全设置

## 截图清单

1. [01-home-office-mode.png](01-home-office-mode.png) — 新建任务首页「日常办公」。三模式分段控件、能力 chips、吉祥物、活动卡和大尺寸 composer 同屏；页面主体居中，左侧栏保持固定宽度。
2. [02-home-code-mode.png](02-home-code-mode.png) — 首页「代码开发」。结构与办公模式不变，仅替换能力集合为日常开发、网站开发、Agent 应用、Skill 开发、CI/CD、文档。
3. [03-home-design-mode.png](03-home-design-mode.png) — 首页「设计创意」。能力集合扩展为网站设计、PPT、视觉海报、移动端 App、设计系统、Web App、品牌设计和插画。
4. [04-sidebar-assistants.png](04-sidebar-assistants.png) — 「助理」页面。本地助理标题和连接状态靠上，composer 固定在底部；内容很少时主动保留大面积留白。
5. [05-sidebar-projects.png](05-sidebar-projects.png) — 「项目」页面。页头插画与主操作并列，下方用单个项目卡和模板卡网格组织内容。
6. [06-sidebar-experts.png](06-sidebar-experts.png) — 「专家·技能·连接器」页面。顶部三级 tab、搜索和精选场景横向卡，下方是筛选 chips 与三列专家卡片；此图同时采到该侧栏项的二级浮层。
7. [07-sidebar-automation.png](07-sidebar-automation.png) — 「自动化」页面。空态 + 黑色主按钮位于上半区，下半区为 3 × 4 模板卡；定时任务/运行记录使用安静的分段控件。
8. [08-sidebar-more-menu.png](08-sidebar-more-menu.png) — 「更多」展开态。浮层按个人文件、文档知识库、灵感分组，使用图标 + 单行文字，没有额外说明。
9. [09-sidebar-collapsed.png](09-sidebar-collapsed.png) — 主侧栏收起态。内容区扩展到整窗，顶部只保留紧凑的导航图标和页面 tab。
10. [10-task-conversation.png](10-task-conversation.png) — 历史任务「Word转PDF文件转换」。同屏包含长回复排版、用户附件气泡、AI 产物文件卡、操作行和底部 composer，可作为会话页主要对标图。
11. [11-composer-more-actions.png](11-composer-more-actions.png) — composer「+」菜单。添加文件、模式、专家、技能、连接器五项，图标和文字纵向排列。
12. [12-model-selector.png](12-model-selector.png) — 模型菜单。顶部 Max 模式开关，中部为模型名称、折扣/倍率信息，底部单列自定义模型与配置入口。
13. [13-workspace-selector.png](13-workspace-selector.png) — 工作空间菜单。带搜索框、空态，并在底部提供「新建工作空间」「打开本地文件夹」两个动作；未执行这些动作。
14. [14-permission-selector.png](14-permission-selector.png) — 权限菜单。默认权限附带解释文字，仅提供「允许完全访问」升级选项；未切换权限。
15. [15-at-reference-trigger.png](15-at-reference-trigger.png) — composer 输入 `@` 的引用态。当前新对话没有文件，因此显示简洁空态；截图后已清空输入。
16. [16-slash-skill-trigger.png](16-slash-skill-trigger.png) — composer 输入 `/` 的技能/指令态。大面积列表浮层显示技能名称和一行说明，搜索入口与 composer 连成一个视觉整体；截图后已清空输入。
17. [17-global-search.png](17-global-search.png) — 左上全局搜索。独立居中弹层，默认展示最近任务。
18. [18-account-menu.png](18-account-menu.png) — 头像账户菜单。包含版本/积分运营卡、设置、外观、帮助、更新和退出；浅色/深色切换直接放在一级菜单。
19. [19-settings-system.png](19-settings-system.png) — 设置「系统设置」。约 11 个分区组成左侧导航；主区使用分组卡，包含语言、字号、自动更新、代理、存储、通知等。
20. [20-settings-account.png](20-settings-account.png) — 设置「账户管理」。账号信息、版本和 Credits 信息集中呈现。
21. [21-settings-agent.png](21-settings-agent.png) — 设置「智能体设置」。
22. [22-settings-personalization.png](22-settings-personalization.png) — 设置「个性化」。
23. [23-settings-memory.png](23-settings-memory.png) — 设置「记忆」。
24. [24-settings-models.png](24-settings-models.png) — 设置「模型」。本地配置文件入口、添加模型按钮和已保存模型列表使用很克制的单层列表布局。
25. [25-settings-assistant.png](25-settings-assistant.png) — 设置「助理设置」。
26. [26-settings-data.png](26-settings-data.png) — 设置「数据管理」。
27. [27-settings-shortcuts.png](27-settings-shortcuts.png) — 设置「快捷键」。搜索 + 表格结构，快捷键用小型 keycap 表达，右侧统一放清除操作。
28. [28-settings-security.png](28-settings-security.png) — 设置「安全中心」。
29. [29-settings-help.png](29-settings-help.png) — 设置「帮助与反馈」。
30. [30-dark-home.png](30-dark-home.png) — 深色首页。近黑主背景、略抬升的侧栏和 composer、低饱和边框；保留绿色活动状态作为少量强调色。
31. [31-dark-task.png](31-dark-task.png) — 深色历史任务。正文、文件卡、输入框和侧栏在深色层级中的实际表现；截图后主题已恢复浅色。
32. [32-narrow-home.png](32-narrow-home.png) — 904px 窄窗首页。侧栏宽度基本不变，主区收窄并保持居中；能力 chips 横向压缩，composer 仍保持完整操作区。截图后窗口已恢复 1061px 宽。

## P0 / P1 覆盖情况

- P0：首页三模式、助理、项目、专家/技能/连接器、自动化、更多菜单、侧栏收起态、历史任务、文件产物卡和会话 composer 均已采到。
- P1：更多操作、模型、工作空间、权限、`@`、`/`、全局搜索、账户菜单、11 个设置分区、深色首页/会话和 904px 窄窗均已采到。
- 未采到：纯 hover 态。当前 Computer Use 接口没有独立 mouse-move/hover 动作，click 会同时改变导航选中状态；为避免使用已导致崩溃的辅助功能注入方案，本项跳过。

## 三条突出设计观察

1. **层级来自大片留白和极轻表面差，而不是重阴影。** 浅色背景接近白色，侧栏约为浅灰，卡片通常只比背景略亮并配极细描边；弹层才使用更明确的阴影。主操作统一近黑，品牌紫主要留给桌面模式入口，绿色只用于连接/活动状态。
2. **相同骨架承载不同办公场景。** 三模式只替换能力 chips，不改变标题、composer 和侧栏；项目、自动化、专家页也持续复用页头、筛选、卡片网格和空态原语，因此功能很多但学习成本低。
3. **Composer 是产品视觉中心。** 首页使用约两行高的大输入面，附件/模型/语音/发送在内层，工作空间和权限位于独立底栏；`+`、模型、权限、引用和技能都以紧贴 composer 的浮层展开，用户很少需要离开当前任务上下文。

## 异常与恢复

- 本轮 Computer Use 点击、键入、拖拽均正常，WorkBuddy 未再次崩溃；全程未使用 AX `set value`。
- 输入的 `@` 和 `/` 已逐字删除，没有发送。
- 深色模式仅用于截图，完成后已恢复浅色。
- 窗口从 1061px 临时缩至 904px，完成后已恢复 1061px。
- 目录中此前失败采集遗留的 `*-test.png` 调试图和 `.mouse_event.applescript` 原样保留，正式材料以本日志列出的 01–32 为准。
