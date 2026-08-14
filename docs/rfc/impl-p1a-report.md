# Tool Runtime P1-a 实施报告

## Commit 1 — 契约基础

`e3a9ea8c` 新增 `runtime/tool_meta.py`：定义七种 kind、v1 `qp` 结构、
JSON 可序列化与 4KB 校验，以及“qp 只能在终态 chunk”断言帮助。2-chunk
和 3-chunk 聚合测试确认 `ToolResponse.append_chunk()` 的最终 `qp` 等于
唯一终态值。

## Commit 2 — 七个 kind 产出

`ef03fd59` 接通 file_write、file_edit、file_read、shell、file_sent、
web_search、batch。文件增删行以锁内执行前后文本快照和
`difflib.SequenceMatcher` 行级 diff 计算；`splitlines()` 统一处理有无尾换行。
写入字节取落盘物理字节，batch 先限 50 条，再为满足 4KB 删除尾部完整
step。所有失败分支只写可证明字段；send_file 仅写 `qp.ok=false`，未改变
legacy SUCCESS state。

## Commit 3 — 传输矩阵

`72a4cd60` 覆盖六条路径：主 SSE 仅 END 写 `FunctionCallOutput.meta`，
中间帧无 meta，历史 ToolResultBlock 回填，取消补写明确无 qp，`/output`
返回终态 meta，`/stream` 保持原样 chunk dump。另完成 RFC §1.4 的后端
icon 透传，仅发送 `ToolUISpec.icon`，未迁移名称或 i18n。

## Commit 4 — split 保护

`2d7548fb` 采用我方 `QwenPawAgent` 覆盖 split 的方案：调用 AgentScope
原 split 后，将原始 ToolResultBlock.metadata 深拷贝到 reserved 和
offloaded block。pruning 开启仍使用现有超大上限避免常规二次 split；
关闭或意外越界时则由复制兜底。测试对 pruning 开/关都强制超限并验证
两块 metadata 存活。未修改 vendored AgentScope 源码。

## Commit 5 — 后端消费迁移

本提交让 `run_tool_batch` 优先读取子工具 `metadata["qp"]["ok"]`；缺失或
非 bool 时保持原 ToolResultState、JSON 和文本嗅探链。B1 sandbox violation
文本分支只标为 legacy fallback，未改变行为。包含优先级与兼容回归测试。

## 存疑点与保守裁决

- `file_read.bytes_read` 表示工具选中、进入截断前的逻辑 UTF-8 文本字节，
  不把展示层 truncation notice 计入。
- batch 的 step 名称可能很长；若前 50 条仍超过 4KB，会从第 50 条起继续
  删除完整 step 并置 `truncated=true`，不会截断字符串制造假字段。
- hosted web search 没有结构化 sources，因此不提供 `source_count`；只有
  Tavily 成功结果提供该字段。
- 实施期间架构师并行提交了 P1-b（`7c7dfba7`），所以它出现在 P1-a 的
  commit 1 与 commit 2 之间；本实施未修改仓库根 `app/` 目录。

## Backlog 触碰记录

RFC §4 backlog 均未实施：未修 send_file state 误标，未解析 hosted 搜索
prose 生成 sources，未退役 GuardedFunctionTool，未改 audit、非 web
renderer 或渠道交付语义。无新依赖。

每个 P1-a commit 前均执行 `uv run pytest tests/unit -q`。除任务说明允许
忽略的 3 个 doubao ASR `.env` 污染失败外，其余单元测试通过。

## P1-c 修复批

- Commit 1（`30744a88`）在真实 coordinator chunk 聚合边界剥离非终态
  `qp` 并记录 warning；终态 `qp` 保持原样。回归覆盖“中间有/终态无”与
  “中间有/终态有”两种序列，并删除了无生产调用者的抛错 helper。
- Commit 2（`03ce2e65`）在后端 `tool_meta.py` 单一源头加入七个 kind 的
  always 与 `ok=true` 必填字段校验。未知字段仅 warning、不拒绝；七个 kind
  各有一个拼错必填键的失败用例。
- Commit 3 增加一条收敛整链路回归：stub 工具真实产出 `ToolChunk`，经
  `ToolCoordinatorMiddleware` 和 AgentScope `_execute_tool_call()` 生成 END
  事件，再通过 Envelope 写入 `FunctionCallOutput.meta`，最后从 AgentScope
  context 经 `chats/utils` 历史转换读回并核对同一份 `qp`。

本批未修改 `app/`，未引入依赖，也未触碰已由 `18ee8a72` 修复的 P1-1。
每个 commit 前均运行 `uv run pytest tests/unit -q`；除任务说明允许忽略的
3 个 doubao ASR `.env` 污染失败外，其余单元测试通过。
