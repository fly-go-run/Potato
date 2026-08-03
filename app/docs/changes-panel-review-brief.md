# 任务:对抗性 review 本轮「改动侧栏 + diff + 预览」实现

你是严苛的代码审查者,目标是**挑战并推翻**下列实现,找出真实缺陷。只审查,**不修改任何文件**。

## 审查范围(本轮改动/新增)

- `app/src/lib/fileChanges.ts`(新)会话消息流 → 文件改动聚合
- `app/src/lib/unifiedDiff.ts`(新)git unified diff 解析 + 绝对路径→仓库相对路径后缀匹配
- `app/src/lib/api.ts` 新增:`workspaceGitApi`(status/diff/discard)、`fetchFileText`
- `app/src/components/chat/ConversationSidePanel.tsx`:「改动」tab、ChangeDiffPanel(useGitDiff 三态:git 真行号视图/回落会话片段)、GitDiffView、撤销按钮(git/discard)、FilePreviewPanel 升级(md 富渲染/shiki 代码高亮/图片/iframe 回落)
- `app/src/components/chat/MessageList.tsx`:FileChangesCard 回合汇总卡
- `app/src/components/chat/FileToolCard.tsx`:LineDiff 重样式
- `app/src/components/chat/Markdown.tsx`:导出 tokenClass
- `app/src/views/ChatView.tsx`:侧栏三态接线(selectedFilePath/selectedChangePath)
- `app/src/components/chat/ModelPicker.tsx`:重写为嵌套菜单(模型/思考深度/恢复默认)

后端契约参考(勿改):`src/qwenpaw/app/routers/git.py`(status/diff/discard)、`src/qwenpaw/app/routers/files.py`(preview)。

## 重点挑战方向(按优先级)

1. **正确性**:聚合统计错算、diff 行号错位、状态机漏态(loading/git/fallback 切换时的竞态,path 快速切换时 setState 串台)、useEffect 清理、AbortController 泄漏。
2. **数据边界**:流式期间半截 arguments、历史消息无 output、git 输出畸形、超大文件、非 UTF8/CRLF。
3. **交互逻辑**:撤销按钮误触/双击、reload 后 undoState 残留、面板开关与 chatId 切换的状态清理、ModelPicker 挂载即拉取与打开刷新的并发。
4. **产品层**:与 Codex Desktop 的行为差距里有没有会让用户困惑的(如 git diff 是全工作树非本会话增量——这是已知接受项,不用报)。

已知接受、不要报的:后缀匹配跨根误命中(低概率接受)、watch SSE 未做、git 指向 coding_mode 目录的语义、i18n 复数形式。不要报纯风格/命名/可读性问题。

## 输出格式

按严重度分级列出:
- P0(会出错/崩溃/数据错):文件:行号 + 触发场景 + 一句话修法建议
- P1(边界下行为错但可恢复)
- P2(值得改进但可不改)

每条必须给出具体触发场景,推不出触发场景的不要列。最后给一行总评:这套实现能不能上。
