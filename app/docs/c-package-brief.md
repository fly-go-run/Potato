# C 包:r3 精确修复三件套(依据双审查报告)

仓库根 /Users/liuxu/lifeProjects/QwenPaw,前端 app/。先读 `app/docs/reference/r3-execution-plan.md` 了解全局分工——你负责 C1/C2/C3,**只许改:app/src/components/chat/FileToolCard.tsx、app/src/views/InboxView.tsx、app/src/lib/i18n.ts**。其他文件(尤其 MessageList.tsx/Composer.tsx/ChatView.tsx,架构方并行在改)绝对不碰。有并行工作流,vitest 出现你没动过的文件的失败可忽略并在返回中注明。

## C1 send_file_to_user 接入产物卡

依据:`app/docs/reference/gap-review-opus-r2.md` P1-7、`gap-review-codex-r2.md` P1-1(先读这两条)。
- `FileToolCard.tsx`:`FILE_TOOL_TITLES` 加 `send_file_to_user` → 新 i18n 键 `tool.file.deliver`(zh「已发送文件」/en "File delivered");加入 `ARTIFACT_TOOLS`。
- 大小解析:现有 fileSizeLabel 从 "Wrote N bytes" 解析;send_file 的 result 文案格式先读后端 `src/qwenpaw/agents/tools/send_file.py` 确认(只读,不改后端),按实际格式加解析分支;解析不到回落现有目录展示。
- 卡片视觉规格不动(ArtifactCard 已达标,见 opus 报告附录)。

## C2 收件箱本地化与相对时间

依据:`gap-review-codex-r2.md` P1-10、`gap-review-opus-r2.md` P2-4/P2-1。
- `InboxView.tsx`:状态 Badge 不再直出 `status` 原文,映射 i18n:`inbox.status.success/error/running`(成功/失败/运行中,未知值原样);来源同理 `inbox.sourceType.memory/cron`(记忆整理/定时任务,未知原样)。
- 标题人话化:`Auto-dream result`/`Auto-memory result` 这类已知后端标题映射为「自动记忆整理」(加 `inbox.title.autoMemory`);未知标题原样。做成小的纯函数 presenter(文件内即可),便于以后扩展。
- 列表时间改相对时间:用现有 `lib/relativeTime.ts`(返回 {key,params} 描述符,交给 t() 渲染,参考 Sidebar.tsx 用法);绝对时间移到展开详情里。
- 所有新文案 zh+en。

## C3 i18n 中文标点全量校对

依据:`gap-review-opus-r2.md` P1-10 列出的 13 处(半角 `,`→`，`、`?`→`？`),范围仅 zh 字典的**界面文案与模板 prompt**;注意:
- cron 表达式、`9:00` 这类数字时间、代码/路径/URL、`{count}` 占位符本身不动;
- `time.minutesAgo` 等四条时间键去掉数字与单位间空格(「{count} 分钟前」→「{count}分钟前」),与记忆页格式统一;
- `composer.placeholder` 改为「今天帮你做点什么？@ 引用文件，/ 调用技能」(服务方视角+全角标点);
- en 字典不动。
- 改完跑 `cd app && npx vitest run`,i18n 相关测试必须过(有 zh/en 键集一致性测试)。

## 完成标准

`npx tsc --noEmit -p app/tsconfig.app.json` 你改的文件无错;vitest 中 i18n/inbox 相关用例全过。不跑 build,不 commit。返回:三个子任务各自的改动摘要+新增键列表。
