# Tool Runtime RFC 对抗审查 R1

结论：**无 P0，存在 10 项 P1、3 项 P2；当前 Draft r1 不宜直接进入 P1-a/P1-b 实施。** 常规 `AgentState` 持久化和现有 `ToolResultPruningMiddleware` 会保留 `ToolResultBlock.metadata`，但下列例外与未定义语义会造成实现返工或同一会话内结构化结果不一致。

## P0

无。

## P1

### P1-1：迁移清单不是仓库内可执行输入

- 证据：`docs/rfc/tool-runtime.md:126-130` 只把 F1-F11/B1-B8 指向“对话记录 2026-08-14”，没有列出各点的文件、行号和提取字段。
- 问题：P1-a/P1-b 无法从 RFC 确认“覆盖现有全部反推点”和“未触发任何 legacy 路径”，遗漏一个消费点也无法由审查清单发现。
- 修正建议：把 F1-F11/B1-B8 的 `file:line`、当前提取值、目标 `kind.data` 字段和迁移/保留结论完整内嵌到 RFC 或提交为仓库内附录。

### P1-2：实时、取消保存和独立 tool-call API 没有统一的 meta 完整性规则

- 证据：`src/qwenpaw/runtime/envelope.py:613-629` 仅在 `TOOL_RESULT_END` 收口，`src/qwenpaw/runtime/envelope.py:969-980` 的 `collect_tool_output()` 只返回文本，`src/qwenpaw/runtime/runtime.py:423-448` 的取消补写 `ToolResultBlock` 不带 metadata；同时 `src/qwenpaw/app/routers/tool_calls.py:224-233` 的 `/output` 丢掉 `final_response.metadata`，而 `src/qwenpaw/app/routers/tool_calls.py:251-259` 的 `/stream` 又会原样 dump 每个 chunk（含 metadata）。
- 问题：RFC 只规定 END 和历史读回，导致主 SSE、取消后历史、独立 chunk SSE、独立最终输出四条路径对同一次工具调用可能分别为有 meta、无 meta 或只有中间态 meta。
- 修正建议：在 §1.3 增加逐路径矩阵，规定完成、取消、部分保存、`/stream` 终帧和 `/output` 的统一最终 meta 来源，并为无法观察到终态 metadata 的取消结果明确“无 qp、必须 fallback”的契约。

### P1-3：AgentScope 大结果二次切分会在 pruning 关闭时剥掉 metadata

- 证据：`src/qwenpaw/agents/react_agent.py:230-263` 的 state dump/load 正常 round-trip metadata，`src/qwenpaw/agents/middlewares.py:613-626` 的历史 pruning 也原地保留 block metadata；但 `.venv/lib/python3.11/site-packages/agentscope/agent/_agent.py:2248-2260` 重建 `ToolResultBlock` 时未复制 metadata，且 `src/qwenpaw/runtime/builder.py:747-755` 只在 pruning 开启时把该二次上限设为不生效。
- 问题：RFC `docs/rfc/tool-runtime.md:78-80` 假定历史 context 中 metadata 必然存在，但 pruning 关闭且结果超过 AgentScope 上限时 `qp` 会在保存前消失。
- 修正建议：把“AgentScope split 后 metadata 仍等于原值”列为实现要求，并在 pruning 开/关两种配置下通过复制 metadata 或等价适配消除该丢失路径。

### P1-4：多 chunk 的 `qp` 合并语义未定义，当前实际行为是整个 `qp` 后写覆盖

- 证据：`.venv/lib/python3.11/site-packages/agentscope/tool/_response.py:133-143` 对 metadata 做浅层 `dict.update`，`src/qwenpaw/tool_calls/_coordinator.py:541-556` 也用同一 `append_chunk()` 聚合后台工具结果。
- 问题：若两个 chunk 分别携带不完整的 `metadata["qp"]`，后一个会覆盖前一个完整对象而不是合并，最终 END、历史和独立 chunk SSE 将看到不同结构。
- 修正建议：契约明确要求 `qp` 是终态原子值且只允许最后一个 chunk 产生（推荐），或定义并实现逐字段深合并及冲突规则，且增加两 chunk/三 chunk 回归测试。

### P1-5：`web_search.source_count` 对 hosted 后端没有机器可信真值

- 证据：`src/qwenpaw/agents/tools/web_search.py:361-380` 的 hosted 搜索只返回一段文本和两个布尔值，没有结构化 source 集合；只有 Tavily 路径在 `src/qwenpaw/agents/tools/web_search.py:402-425` 持有原始 `results` 列表。
- 问题：按 `docs/rfc/tool-runtime.md:47-50,63` 的“机器可信”要求，hosted 的 `source_count` 只能再解析 prose，正好重引入本 RFC 要消除的文本反推。
- 修正建议：把 `source_count` 定为 Tavily-only/可缺字段，或先让 hosted provider 返回结构化 sources 再把它列为两后端共同必填字段。

### P1-6：`file_sent.delivered` 在工具执行层不代表真实交付

- 证据：`src/qwenpaw/agents/tools/send_file.py:78-94` 只构造一个本地 `file://` DataBlock 就返回 “File sent successfully.”，后续才由 `src/qwenpaw/app/channels/renderer.py:230-260` 转成 FileContent、由 `src/qwenpaw/app/channels/base.py:2050-2065` 调用渠道发送（默认 `send_media` 甚至是 no-op）；并且 `src/qwenpaw/agents/tools/send_file.py:50-69,96-104` 的不存在、非文件和异常分支仍标成 `ToolResultState.SUCCESS`。
- 问题：在工具真实执行处写 `delivered=true` 会把“已生成可交付 DataBlock”误报为“用户已收到”，而沿用现有 state 又会把三种错误误判成成功。
- 修正建议：将该字段定义为本层可证明的 `attached`/`available` 语义，或把真实 `delivered` 的确认点移到渠道发送成功处，并同步规定上述错误分支的 `qp.ok=false`。

### P1-7：`file_edit` 的“精确 ±行数”没有算法定义，验收基准也缺少前置状态

- 证据：`src/qwenpaw/agents/tools/file_io.py:416-435` 只做 Python 全局 `replace` 且没有 diff 统计，前端 `app/src/lib/fileChanges.ts:149-176` 对单份 old/new 做 LCS 并明确承认多次替换会少算；RFC 却在 `docs/rfc/tool-runtime.md:68-70,110-111` 同时要求后端“精确”且等于 `git diff --numstat`。
- 问题：`replacements` 可由执行前内容计数得到，但 additions/deletions 会受 diff 算法、文件原有未提交改动、未跟踪/二进制文件影响，当前文字没有唯一可实现的真值。
- 修正建议：明确以执行前后快照运行哪一种 numstat 等价算法、换行/二进制规则及干净 tracked fixture，并把“同一编辑”限定为隔离仓库中单次操作的前后差。

### P1-8：`file_write` 没覆盖前端 F7 实际消费的统计字段，且现有 bytes 文案不是真字节数

- 证据：`app/src/lib/fileChanges.ts:179-190` 对 write/append 实际提取并消费 `additions/deletions`，`app/src/components/chat/FileToolCard.tsx:304-312` 又从结果提取 bytes；但 `docs/rfc/tool-runtime.md:58` 仅给 `lines_written`，而后端 `src/qwenpaw/agents/tools/file_io.py:308-315,505-512` 使用 `len(content)` 并把 Unicode 字符数标成 bytes（`.txt/.csv` 还使用 `utf-8-sig`，见 `src/qwenpaw/agents/tools/file_io.py:71-92`）。
- 问题：F7 若宣称迁移到 meta 仍需从 arguments 推算增删行，且照抄当前 `len(content)` 会违反“机器可信 bytes_written”并在非 ASCII/BOM 文件上给错数。
- 修正建议：为 `file_write` 明确增加/映射 F7 所需的 `additions/deletions`（含覆盖写语义），并规定 `bytes_written` 是编码后的物理写入字节、`created` 在同一文件锁内判定、行数采用统一尾换行规则。

### P1-9：`batch.steps` 与 4KB 硬上限互相冲突

- 证据：RFC `docs/rfc/tool-runtime.md:51,64` 要求完整 `steps:[{tool,ok,kind}]` 且 meta ≤4KB；`src/qwenpaw/agents/tools/run_tool_batch.py:1105-1112` 默认允许 500 次执行，循环会把每次结果追加到 `results`（`src/qwenpaw/agents/tools/run_tool_batch.py:900-917`）。
- 问题：数百个步骤即使每项只含短工具名也会稳定超过 4KB，实施者无法同时满足 batch 契约和全局体积断言。
- 修正建议：规定 steps 的确定性截断/首尾采样与 `total/completed/failed/truncated` 汇总，或为 batch 设独立有界上限，并给出恰好越过 4KB 的边界测试。

### P1-10：`FunctionCall.ui.display` 没有可查询的数据源

- 证据：RFC `docs/rfc/tool-runtime.md:81-84` 要从 ToolUISpec 生成 `{icon, display}` 并替代前端名称表，但 `src/qwenpaw/runtime/tool_registry.py:47-54` 的 ToolUISpec 只有 `description`、`icon` 和布尔 `display_to_user`，没有显示名称字段。
- 问题：实施者只能把 description 或布尔值误当 display，无法按 RFC 替代 ToolCard 的工具名称映射。
- 修正建议：在 RFC 中明确 `display` 的类型和来源，并在 ToolUISpec 增加真正的显示名/本地化 key，或删除“名称表迁移”要求只透传现有 icon/description。

## P2

### P2-1：envelope 丢 metadata 的引用位置不准确，容易只改调用点而漏改 builder

- 证据：RFC 指向 `envelope.py:607-641`，实际 END 分支在 `src/qwenpaw/runtime/envelope.py:613-646`，真正构造并丢弃 metadata 的公共 builder 在 `src/qwenpaw/runtime/envelope.py:776-815`，尤其 `802-808` 只写 call_id/name/output/state。
- 问题：按 RFC 行号实施可能只读取 `event.metadata` 却没有把它传入 `_build_tool_result_content()`，最终帧仍不会出现 meta。
- 修正建议：把证据改为 END 调用点加 builder 两处，并明确 builder 新增 `meta` 参数且只有 final END 调用传入 `event.metadata.get("qp")`。

### P2-2：kind 表没有定义 `ok=false` 时 data 字段的必填/可缺规则

- 证据：RFC 表中只有 `shell.violation` 标注“可缺”（`docs/rfc/tool-runtime.md:56-64`），但 shell 在自杀保护和执行异常时没有进程 exit code（`src/qwenpaw/agents/tools/shell.py:589-603,816-826`），send_file 的不存在/异常路径也拿不到可靠 `size_bytes`（`src/qwenpaw/agents/tools/send_file.py:50-69,96-104`）。
- 问题：若所有失败也要提供 `qp.ok=false` 供 batch 消费，未注明条件字段会迫使实现填假 exit code/size，或让同一 kind 产生不可预测 shape。
- 修正建议：为每个 kind 分别列出 always/`ok=true`/`ok=false` 字段要求，禁止用 sentinel 冒充不可得事实，并为每类失败分支加 schema 测试。

### P2-3：验收标准缺少可复现夹具和可观察断言点

- 证据：`docs/rfc/tool-runtime.md:103-113` 要求“各来一次”、dev-only 计数器、旧会话逐像素一致和 git numstat 一致，却未指定 SSE 端点/终帧、hosted/Tavily 与 sandbox 夹具、旧历史 golden、浏览器尺寸字体或 Git 初始状态。
- 问题：这些条件目前只能人工解释，CI 无法稳定判定通过，尤其无法覆盖 metadata 丢失所依赖的取消、pruning 开关和多 chunk 分支。
- 修正建议：把验收改成确定性测试矩阵，至少覆盖主 SSE final frame、历史 round-trip、取消保存、独立 tool-call `/stream`/`/output`、pruning 开关、两 chunk 覆盖、两种搜索后端和 batch 4KB 边界，并固定视觉/Git fixture。

## 已核实且无需报错的现状引用

- `src/qwenpaw/schemas.py:146-150`：`FunctionCallOutput` 的确配置 `extra="allow"`。
- `src/qwenpaw/app/chats/utils.py:616-652`：历史读回的 `tool_result` 分支位置和行为属实，当前确实未回填 block metadata。
- `src/qwenpaw/governance/tool_adapter.py:442-455`：sandbox violation 已 metadata 优先、文本 split fallback。
- `src/qwenpaw/agents/tools/run_tool_batch.py:162-206`：batch 已优先看 ToolResultState/JSON `ok`，仍对纯文本调用 `_is_error_text()`；改为 `qp.ok` 优先的方向属实。
