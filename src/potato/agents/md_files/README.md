# 应用工作区模板

这里的 `zh/en/id/ru/AGENTS.md` 是应用运行时的行为模板，不是给仓库开发代理的项目说明。它们仍被 Python 的 `agents/utils/setup_utils.py` 复制到用户工作区，由 `runtime/prompt_contributors.py` 加载；打包脚本也收集这些资源。因此不能仅凭文件名或创建时间删除。

2026-09-06 已核对并同步修订四种语言：移除默认具备外部渠道、工具或调度能力的暗示，明确既有授权可继续使用，编辑 heartbeat 文档不会创建定时任务。保留 `heartbeat:start/end` 标记，供运行时按能力过滤。

修改默认模板不代表现有用户工作区已经更新；不要覆盖用户自己的角色、偏好或记忆文件。Rust 迁移也必须保留这一约束。旧 QA Agent 的专用模板已经移除，`app/docs/qa-agent-removal-*` 仅是带历史标记的完成记录。

原 `.gitignore` 的全局 `AGENTS.md` 规则误忽略了这四份运行时资源；现已为准确路径添加例外。`build/`、`console/src-tauri/binaries/` 中的同名文件是构建/安装资源副本，不是新的模板源；已清理旧 `build/lib/qwenpaw` 中七份过时的 `AGENTS.md` 副本，安装资源应通过重建更新。`.claude/worktrees/` 属于其他工作副本，不在此次清理范围。

Rust 核心在编译时嵌入这四份 `AGENTS.md`，不需要安装 Python。新工作区首次使用时将模板保存到 SQLite 文档存储；此后读取保存的版本，语言切换或应用升级不覆盖用户修改。Rust 已实现 cron/单次任务调度；自主 heartbeat 尚未启用，因此发送给模型前仍移除 heartbeat 标记段。编辑文档本身不会创建任务。
