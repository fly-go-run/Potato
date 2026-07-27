# Phase 5 任务包：项目粒度 + 定时任务 + 收件箱（执行者：Codex）

对标 ChatGPT/Codex Desktop 的三个能力缺口，用户已确认范围。约束不变（只动 `app/`、不动 `tokens.css`、不新增依赖、文案全走 zh/en i18n、只用语义色类）。后端 API 均已存在，**后端零改动**。

## 1. 项目粒度（会话绑定工作目录，对标 Codex 的 project 选择）

后端机制（已核实）：
- 每次聊天请求可带 `request_context["qwenpaw.coding_project_dir"] = "<绝对路径>"`，
  该轮即在此目录启用 Coding Mode（文件/git/shell 作用于该目录；见
  `src/qwenpaw/runtime/builder.py:546` `_apply_request_coding_project`）。
  不带则用默认工作区。
- 项目管理 API（`src/qwenpaw/app/routers/coding_project.py`，prefix
  `/api/workspace/coding-project`）：`GET ""` 当前目录、`GET /list` 项目列表
  （`{path,name,is_git,is_active}`）、`POST /create` 新建、浏览本机目录接口
  （读该 router 确认路径与响应，含 Windows 盘符逻辑）。

UI 设计（**按此实现，不要自由发挥**）：
- Composer 左侧、模型按钮之前加「项目」chip：folder 图标 + 当前项目名
  （未绑定显示「默认工作区」）。点击弹 Radix Dialog 项目选择器：
  - 列表 = `GET /list` 的项目 + 最近手选目录（localStorage，最多 8 条）；
  - 「浏览目录…」走后端 browse 接口做简易目录浏览器（列子目录、可逐级进入/返回、
    选定当前目录）；「新建项目」调 `POST /create`；
  - 每行显示 name + is_git 时的 git 徽标 + 淡色完整路径。
- 绑定粒度 = 会话：选择后存 store 并持久化（localStorage 按 session_id 记，
  新会话继承上次选择）；该会话每次 `sendMessage` 的 `request_context` 都带上。
- 历史会话打开时恢复其绑定并显示在 chip 上。
- 联调验证：绑定一个 git 目录后发「用 shell 执行 pwd 和 git status」，
  确认输出的确在所选目录（approval_level OFF），测完删会话。

## 2. 定时任务页（对标 Scheduled）

API（`src/qwenpaw/app/crons/api.py`，prefix `/api/cron`；Spec 结构读
`src/qwenpaw/app/crons/models.py`）：jobs CRUD、`/jobs/{id}/pause|resume|run`、
`/jobs/{id}/state`、`/jobs/{id}/history`、`GET /dispatch-targets`。

UI：
- 侧栏「新建会话」下方加「定时任务」入口（Clock 图标），路由 `/crons`。
- 列表页：名称、可读化 schedule、启/停状态开关（pause/resume）、上次/下次运行、
  行操作（立即运行、删除带确认、查看历史）。
- 历史抽屉：最近执行记录（时间、状态、摘要）。
- 新建/编辑（Dialog 单表单）：名称、cron 表达式输入 + 常用预设下拉（每小时/每天 9:00/
  每周一 9:00…选预设自动填表达式）、任务 prompt（多行）、投递目标（dispatch-targets 下拉）。
  以 Spec 实际字段为准，精简版只暴露这些，其余字段用后端默认。

## 3. 推送消息收件箱（对标通知/后台任务结果查看）

API（`src/qwenpaw/app/routers/console.py:452-530`）：
`GET /api/console/inbox/events?unread_only=&limit=`、`POST /inbox/read`
（`{all}` 或 `{event_ids}`）、`DELETE /inbox/events/{id}`、`GET /inbox/traces/{run_id}`。

UI：
- 侧栏「定时任务」下方加「收件箱」入口（Inbox 图标），路由 `/inbox`；
  未读数徽标（进入应用时取一次 unread_only 计数，打开页后刷新）。
- 列表：来源/时间/摘要，未读加粗+圆点；点击展开详情（正文 + 如有 run_id 提供
  「查看运行轨迹」展开 trace 摘要）；「全部已读」、单条删除。
- 事件结构以真实返回为准（先 curl 看 events 字段再写渲染）。

## 4. 报告 `app/docs/phase5-report.md`

逐条对照；`npm test`（项目选择器状态、cron 表单→spec 组装、收件箱未读逻辑补测）
与 `npm run build`；真实联调各功能冒烟（含上文项目绑定验证）；联调数据清理
（测试 cron job 删除、测试会话删除）。dev server: 5174。
