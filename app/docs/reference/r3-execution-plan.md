# r3 执行计划(基于双审查报告 gap-review-{codex,opus}-r2)

分工规则(用户定):复杂+审美=Claude;简单+审美=Opus;无审美=codex。

## A 包(Claude 亲自)
- A1 排版根因:global.css body line-height 1.6→1.5;Sidebar 行密度收敛(w-64→14.5rem,行 13px/leading-5/py-1.5,时间 11px)
- A2 tokens:全套中性色拉平 B 通道(去蓝紫偏);新增 --overlay(遮罩语义,修深色遮罩反向 bug)、--composer-tray(深色比 surface 亮)、--shadow-composer;ink-tertiary 压深
- A3 Composer:发送键永远实心(禁用只降 opacity);宽度 52rem→46rem;圆角 24/18→18/14;托盘 p-1、深色去黑阴影;placeholder 全角标点+服务方措辞;composer 下方 AI 免责行
- A4 MessageList:相邻工具行归并 ToolGroup(≥2 条收成"已完成 n 步"一行,产物卡除外);用户气泡 82%→70%、圆角 18→10;阅读列 46rem;动作行右侧加模型/用量;产物汇总行
- A5 ChatHeader:sticky 44px,标题+右侧动作,bg-canvas/85 backdrop-blur

## B 包(Opus)
- B1 页面骨架:PageHeader 降档(19px/mb-6,副标题仅空态)+PageContainer py-6;EmptyState 去描边盒(bare);Crons 单 CTA;SettingsView 面板 max-h 自适应+左导航 bg-bg;SettingRow/EmptyState 说明文字 ink-muted→ink-tertiary;Button 默认矩形(radius-sm),pill 仅 chips
- B2 技能页:图标统一线稿(去 ✦/emoji 混排)、名称 humanToolName、描述 2 行、启用态 Switch、行 padding 统一、可点击暗示 chevron
- B3 记忆页:满宽单列同构、标题去 .md/slug 转自然标题、元信息降级;删 memory.ts 私有 formatRelativeTime 统一走 lib/relativeTime

## C 包(codex)
- C1 FileToolCard:send_file_to_user 接入产物卡(titles/ARTIFACT_TOOLS/从 result 解析大小)+「已发送文件」i18n
- C2 InboxView:状态/来源枚举本地化映射、AutoDream 标题人话化、列表改相对时间(lib/relativeTime)
- C3 i18n 中文标点全量校对(13 处半角→全角;时间格式去空格「{count}小时前」;代码/cron 表达式/数字时间不动)

## 顺延到 r4(记录不丢)
- 技能/记忆完整 presentation model(中文名 registry、memory API 返回 frontmatter)
- 侧栏底部身份/工作区锚点;会话内搜索;右侧产物面板
- P0-2 的路径脱敏(本轮归并后默认不可见,展开态脱敏 r4 做)
