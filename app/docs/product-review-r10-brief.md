# 任务:产品层深度 review —— 设置以外的全部界面

你是资深产品设计审查者(你此前抓过"会话页像调试控制台/技能收件箱记忆透传后端 schema"这类问题,继续用这个视角)。只审查,**不修改任何文件**。设置窗口刚重做过,不在本轮范围。

## 审查范围(读代码推断实际渲染效果)

- `app/src/views/SkillsView.tsx` 技能与插件页
- `app/src/views/MemoryView.tsx` 记忆页
- `app/src/views/CronsView.tsx` 定时任务页
- `app/src/views/InboxView.tsx` 收件箱页
- `app/src/components/layout/Sidebar.tsx` 左侧栏(含收起态)
- `app/src/components/layout/ChatSearchDialog.tsx` ⌘K 命令面板
- `app/src/views/ChatView.tsx` 聊天页 chrome(header/搜索/banner)
- `app/src/components/chat/Composer.tsx` 输入区(含 / 技能、@ 文件弹层、项目 chip、审批选择器、上下文用量)
- `app/src/components/chat/` 消息渲染族(MessageList/ToolCard/ShellToolCard/ProgressCard/ReasoningBlock/ApprovalCard/JsonView)
- 空态与错误态(EmptyState 用法、chatBanner)

## 审查维度(按优先级)

1. **后端 schema 透传**:哪些地方直接把接口字段/枚举/英文原文丢给用户(名称、状态、错误信息)而没有 presentation model。
2. **流程断点**:完成一个任务要跳几个地方、有没有死胡同(做完一件事回不去/不知道下一步)、危险操作有没有确认、操作后有没有反馈。
3. **信息层级**:每页第一眼该看到什么 vs 实际第一眼是什么;重复入口;不该并列的并列了。
4. **一致性**:同类操作在不同页面的交互/文案/位置是否一致(如删除、刷新、空态、时间显示)。
5. **中英文案质量**:面向用户的措辞是否说人话。

不要报:纯代码质量/性能、设置窗口、已知的 codex/ChatGPT 对标差距文档里已有项(可读 `app/docs/reference/` 下历史报告避免重复)。

## 输出

按页面分组,每条:文件:行号 + 现状一句话 + 为什么伤体验 + 修法一句话 + P0/P1/P2。最后给一个「最伤的前 5」排序。
