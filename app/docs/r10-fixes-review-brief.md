# 任务:对抗性 review r10 修复批次

你是严苛的审查者,挑战下列修复的正确性。只审查,**不修改任何文件**。背景:这批修复对应 `app/docs/design-audit-r10.md` 的工作包 W-R1~R5。

## 本批改动文件

- 新增 `app/src/lib/errorPresentation.ts`(presentError)、`app/src/lib/skillPresentation.ts`(中文技能文案)
- `app/src/lib/inbox.ts`:summarizeAutoDream/isRoutineEvent/presentTraceStep
- `app/src/views/InboxView.tsx`:正文 presentation、例行分组(展开即批量已读)、轨迹时间线+就地错误重试、来源跳转(cron/memory/chat)
- `app/src/views/CronsView.tsx`:投递目标不默认选中+会话名展示、无目标 CTA、状态未知、保存成功反馈
- `app/src/components/chat/ApprovalCard.tsx`:批准同类内联确认层(展示 similar_target)
- `app/src/components/chat/Composer.tsx`:审批档位常显+四档说明、技能加载失败重试(TriggerPopover errorText/onRetry)、@文件 url 去重、会话内上下文用量(≥80% 变警示色)
- `app/src/views/ChatView.tsx`:限流 banner「换用 X 并重发」(切换成功后自动重发最后一条用户消息)
- `app/src/components/chat/ProgressCard.tsx`:失败终态恒可见、内部标识符标题→「正在处理…」
- luna 已改(一并复核):SkillsView(描述人话化/emoji/三态/取消反馈/多工作区选择)、MemoryView(脏确认/重试/三态)、ChatSearchDialog(设置 background/hover 同步)、Sidebar(操作失败反馈)

## 重点挑战

1. 收件箱:例行分组的已读批量标记与 unread 计数一致性;summarizeAutoDream 正则的误匹配/漏匹配;presentTraceStep 误分类。
2. Crons:编辑已有任务时目标回填是否仍正确;targetLabel 在 chats 未加载时的表现;保存反馈与 load 失败并发时的 banner 覆盖。
3. 限流重发:sendMessage 重发时机(切换未生效就发?)、无 lastUserText 场景、重复点击。
4. ApprovalCard:确认层与 processing 状态竞态。
5. ProgressCard presentTitle 启发式的误伤(把该显示的标题吞了)。
6. luna 批次:脏确认覆盖所有关闭路径?多工作区选择的提交校验?

不要报:纯风格、i18n 复数、已在 r10 报告中标记为接受/暂缓的项。

## 输出

P0/P1/P2,每条:文件:行号 + 触发场景 + 一句话修法。最后总评。
