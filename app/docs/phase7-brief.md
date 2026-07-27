# Phase 7 任务包：记忆管理 UI（执行者：Codex）

QwenPaw 后端有活跃的长期记忆（remelight 后端），但新前端一直没有查看/管理入口。
本期补上记忆的浏览与编辑。约束不变（只动 `app/`、不动 `tokens.css`、不新增依赖、
文案全走 zh/en、语义色类）。执行纪律同 Phase 6：每条联调路径验一遍即止，不空转。

## 后端 API（已核实，prefix `/api/workspace`）

- `GET /api/workspace/memory` → `MdFileInfo[]`：`{filename, path, size, created_time, modified_time}`。
  `filename` 是相对记忆根的路径，形态举例：
  - `2026-07-27.md`（按天的日记）
  - `2026-07-27/qwenpaw-shell-xxx.md`（当天沉淀的具体条目）
  - `digest/procedure/xxx.md`（沉淀的流程）
  - `digest/wiki/xxx.md`（沉淀的知识）
- `GET /api/workspace/memory/{md_path:path}` → `{content}`（markdown 原文）。
- `PUT /api/workspace/memory/{md_path:path}` body `{content}` → `{written: true}`。
- **没有 DELETE 端点**：本期不做删除。UI 上不要放删除按钮，也不要用"清空内容"伪造删除。

## 交付

1. **侧栏入口**：在「技能与插件」下方加「记忆」入口（lucide `NotebookPen` 或 `BrainCircuit`，
   二选一更克制的），路由 `/memory`，页面懒加载独立 chunk。i18n key `sidebar.memory`。
2. **`/memory` 列表页**：
   - 页头：标题 + 副标题（说明这是 agent 的长期记忆）。
   - 按 `filename` 顶层前缀智能分组，分组标题用 i18n：
     - 日期形态（`YYYY-MM-DD.md` 或 `YYYY-MM-DD/...`）→「日记」
     - `digest/procedure/...` →「流程」
     - `digest/wiki/...` →「知识」
     - 其余 →「其它」
     组内按 modified_time 倒序。空记忆时给克制的空态（不是光秃秃一句话）。
   - 列表复用技能页的无边框分隔列表风格：文件名（去掉分组前缀更易读）+ 大小 +
     相对时间（如"2 小时前"，复用现有 formatTimestamp 或 crons 的时间格式化）。
3. **详情/编辑**：点击行进入详情（右侧抽屉或页内展开，选更顺手的）：
   - 默认 markdown 渲染查看（复用现有 `Markdown` 组件）。
   - 「编辑」按钮切换为 textarea 编辑；「保存」调 PUT，成功后回到查看态并刷新列表时间；
     「取消」丢弃改动。保存中禁用按钮，失败用 Banner/内联提示且不丢改动。
4. **i18n**：新增 `memory.*` 与 `sidebar.memory` 键，zh/en 双份。
5. **报告** `app/docs/phase7-report.md`（≤60 行）。

## 已知限制（写进报告）

- 无删除端点，本期不支持删除记忆。
- 编辑保存只改文件；记忆的语义检索索引（remelight）由后端后台重建，本期不主动触发
  reindex（`POST /api/agents/{id}/memory/reindex` 需要 agentId，且属于索引管理范畴，
  留待后续）。在编辑保存成功提示里可温和说明"内容已保存，检索索引稍后自动更新"。

## 验证

- `npm test`（分组逻辑、编辑保存状态机补测）+ `npm run build` 通过。
- 真实联调：本机后端有 5 个记忆文件，`/memory` 能正确分组展示；打开一个查看；
  编辑保存一次并确认 `GET` 回读到新内容（测完把内容改回原样，不要污染用户记忆）。
- dev server 在 5174（QWENPAW_DEV_BACKEND 指向桌面后端）。
