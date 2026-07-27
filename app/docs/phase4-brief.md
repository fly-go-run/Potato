# Phase 4 任务包：打磨（执行者：Codex）

延续 Phase 1/2（报告见 `app/docs/phase1-report.md`、`phase2-report.md`）。约束不变：只动 `app/`、不动 `tokens.css`、不新增依赖、只用语义色类、新文案必须同时进 zh/en 词典。以下修正清单部分来自架构方对真实截图的视觉 review，逐条执行，不要自由发挥。

## 1. 键盘快捷键

- `Cmd/Ctrl+N`：新建会话（等价点击侧栏按钮；在输入框聚焦时也生效）。
- `Cmd/Ctrl+K`：会话搜索弹层（Radix Dialog）：输入即过滤会话名，↑↓ 选择、Enter 打开、Esc 关闭。列表复用侧栏的排序（置顶优先）。
- 修饰键按平台显示与响应（mac 用 ⌘，其余 Ctrl）。

## 2. 空态与错误态

- 空会话欢迎语下加一行 muted 副提示（如 zh:「输入任务开始，或用 ⌘K 检索历史会话」，en 对应），字号 sm、克制。
- 把 ChatView 顶部 error 横幅与 rate_limited 提示统一为同一个 Banner 组件（danger/warn 两种语气）。
- rate_limited：展示后端返回的 `error` 文本；若 `alternatives` 非空（结构读
  `src/qwenpaw/app/channels/console/channel.py` 的 `_get_free_model_alternatives`），
  提供「切换到 <名称>」按钮：`PUT /api/models/active` 后刷新 activeModel 并收起横幅。

## 3. 特化卡补充

- 文件类工具卡：先在后端确认工具名（`src/qwenpaw/governance/tool_registry.py` 及内置工具定义，
  找出读/写/编辑文件类工具的实际 name 与参数结构），为它们做特化卡：
  路径行 + 展开后内容区；参数含新旧内容（edit/replace 类）时渲染行级 diff
  （自实现朴素 diff：相同行灰、删除行 danger-soft、新增行 ok 色调；**不引依赖**）。
- `progress` 类型消息：渲染为轻量进度卡（工具名/标题 + 状态文本，完成后收敛为一行）。
- 未匹配特化卡的工具一律走通用卡（现状保持）。

## 4. 视觉修正清单（照做）

1. 对话流首条消息距顶部留白加大（约 pt-6，与底部呼吸感一致）。
2. Composer 外框圆角 `rounded-lg` → `rounded-xl`。
3. 设置页「外观」三个按钮与「语言」两个按钮宽度各自统一（等宽網格）。
4. 侧栏会话行确认有 hover 底色过渡；运行中脉冲点与置顶图标不与文字截断冲突（长名 truncate）。
5. 全局检查：所有新旧文案均走 i18n（不允许残留硬编码中英文）。

## 5. 自查报告 `app/docs/phase4-report.md`

逐条对照本文件说明完成情况；`npm test`（为 diff 算法、会话搜索过滤、banner 分支补测）
与 `npm run build` 结果；联调冒烟（快捷键、搜索、切换免费模型按钮如可触发）。
联调会话测完删除。dev server 在 5174（QWENPAW_DEV_BACKEND 已指向桌面后端）。
