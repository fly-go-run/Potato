# RFC: Tool Runtime — 结构化工具结果契约（P1）

状态：r2（吸收对抗审查 `tool-runtime-review-r1.md`；定稿）
分支：`refactor/runtime-kernel`（承接 P0）

## 0. 本质与判断

现状：`ToolChunk.metadata`（AgentScope 原生字段）从工具产出携带到
`ToolResultEndEvent`，在 Envelope 被丢弃——END 调用点
envelope.py:613-646，真正丢弃发生在公共 builder
`_build_tool_result_content`（envelope.py:776-815，:802-808 只写
call_id/name/output/state）。前端 11 处、后端 8 处因此用正则/字符串
从 prose 反推结构（清单见 §6）。`FunctionCallOutput` 为 `extra="allow"`
（schemas.py:146-150），加字段向后兼容。

P1 本质 = 接通这一跳 + 为 metadata 立契约 + 两端迁移。

非目标：不重写治理链路；不动 AgentScope `<<<TRUNCATED>>>` prose 信号；
不动非 web 渠道 prose 渲染；旧数据前端正则降级保留；deprecated
`GuardedFunctionTool` 不退役（backlog）；**前端工具名称/时态 i18n 表
不迁移**（本地化属于前端，后端只透传 icon——审查 P1-10 裁决）。

## 1. 契约

### 1.1 载体、命名空间与原子性

工具把规范化结果放进 `ToolChunk.metadata["qp"]`：

```python
metadata["qp"] = {
    "v": 1,
    "kind": "file_write",
    "ok": True,        # 语义成败（区别于 ToolResultState 执行状态）
    "data": {...},
}
```

规则：

1. **`qp` 是终态原子值：只允许最终 chunk（`is_last=True` /
   ToolResponse 终态）携带**。中间 chunk 不得写 `qp`；合并语义因此
   退化为"唯一写入"，不存在深合并（审查 P1-4；加 2-chunk/3-chunk
   回归测试断言 append_chunk 后 `qp` 等于终态值）。
2. `data` 只放机器可信字段；**不可得的事实不填 sentinel，直接缺字段**
   （审查 P2-2）。每个 kind 的字段标注 always / ok=true only。
3. JSON-serializable；大内容仍走 content blocks。单条 meta 序列化
   > 4KB 为契约违规（单测断言）。
4. 工具可以不产 meta；消费方必须容忍缺失。

### 1.2 kind 表（r2 修订）

| kind | 工具 | data 字段（标注 always / ok=true） |
|---|---|---|
| `file_write` | write_file / append_file | always: `path`; ok=true: `bytes_written`(编码后物理字节), `additions`, `deletions`, `created` |
| `file_edit` | edit_file | always: `path`; ok=true: `replacements`, `additions`, `deletions` |
| `file_read` | read_file | always: `path`; ok=true: `bytes_read`, `line_start`, `line_end`, `total_lines` |
| `shell` | execute_shell_command | always: `sandboxed`; ok=true 或进程完成时: `exit_code`; 可缺: `violation`（自杀保护/执行异常等拿不到 exit_code 时缺省，不填假值） |
| `file_sent` | send_file_to_user | always: `path`; ok=true: `size_bytes`, `attached`(本层语义=已生成可交付 DataBlock，**不是**已送达；审查 P1-6) |
| `web_search` | web_search | always: `backend`; Tavily-only 可缺: `source_count`（hosted 无结构化 sources 真值，不反推 prose；审查 P1-5） |
| `batch` | run_tool_batch | always: `total`, `completed`, `failed`, `truncated`(bool); `steps`: [{`tool`,`ok`,`kind`?}] **上限 50 条**，超出置 truncated=true 只保留前 50（审查 P1-9；加 4KB 边界测试） |

行数算法（审查 P1-7/P1-8）：`additions/deletions` 由**工具内执行前后
内容快照经 `difflib.SequenceMatcher` 行级 diff** 计得；统一尾换行规则
（无尾换行的末行按一行计）；write 覆盖已有文件时与旧内容 diff，
新建文件 additions=行数、deletions=0。验收用 fixture 期望值断言，
不再与 `git diff --numstat` 比对（消除环境依赖）。

### 1.3 传输矩阵（审查 P1-2）

| 路径 | 位置 | meta 规则 |
|---|---|---|
| 主 SSE 终帧 | envelope TOOL_RESULT_END → `_build_tool_result_content` | builder 新增 `meta` 参数；**仅 final END** 传 `event.metadata.get("qp")`，写入 `FunctionCallOutput.meta`（审查 P2-1） |
| 主 SSE 中间帧 | TEXT/DATA_DELTA 重发 | 不带 meta（qp 本就只在终态） |
| 历史读回 | chats/utils.py:616-652 tool_result 分支 | `ToolResultBlock.metadata["qp"]` 存在则回填 `meta` |
| 取消/部分保存 | runtime.py:423-448 补写 ToolResultBlock | **无 qp——契约明确取消结果无 meta，前端 fallback 必须生效**（不伪造终态） |
| tool_calls `/output` | routers/tool_calls.py:224-233 | 加 `meta`（取 final_response.metadata["qp"]） |
| tool_calls `/stream` | routers/tool_calls.py:251-259 | 现状已原样 dump chunk（含 metadata），不改；qp 只出现在终态 chunk，天然一致 |

**AgentScope split 保护（审查 P1-3，实现硬要求）**：pruning 关闭且结果
超过 AgentScope 上限时，`agentscope/agent/_agent.py:2248-2260` 重建
ToolResultBlock 不复制 metadata。实现必须消除该丢失路径（在我方可控层
复制回 metadata 或保证上限不触发），并在 **pruning 开/关两种配置**下
测试 metadata 存活。

### 1.4 工具调用帧 UI 透传（r2 缩窄）

`TOOL_CALL_START` 的 `FunctionCall` 新增 `ui: {icon}`（来自
ToolUISpec.icon，经 ToolDescriptor 查询；display_to_user 已有独立
消费方，不上行）。前端图标硬编码回退保留；**名称/时态表不迁移**。

### 1.5 后端消费迁移

- B1 `tool_adapter.py:442-455`：保持 metadata 优先现状，文本 split
  标注 legacy。（已核实是现状，改动仅注释。）
- B2/B3 `run_tool_batch`：优先读子工具 `metadata["qp"].ok`，缺失回落
  现有 ToolResultState/JSON 嗅探链。

## 2. 分工与顺序

1. **P1-a（codex）**：`runtime/tool_meta.py` 契约类型与校验、7 kind
   产出、传输矩阵全路径、split 保护、B2/B3 迁移、全部单测。
   纯后端，不碰 app/。
2. **P1-b（Claude 亲自）**：前端消费（ToolPair.meta、F1-F9 meta 优先/
   正则回落、icon 走 ui 字段）、卡片视觉微调、dev-only legacy 路径
   计数器。
3. **P1-c（codex）**：整体对抗审查。

## 3. 验收（确定性测试矩阵，审查 P2-3 裁剪版）

后端（CI 可判）：

- 每 kind 一个产出单测（含 ok=false 分支的字段 schema 断言、
  4KB 上限、batch 50 条与 4KB 边界）。
- 传输矩阵 6 路径各一测：终帧带 meta、中间帧不带、历史 round-trip、
  取消无 qp、`/output` 带 meta、多 chunk 终态唯一写入。
- pruning 开/关 × 超限结果的 metadata 存活测试。
- `file_edit`/`file_write` 行数 fixture 断言（含全局多次替换、
  无尾换行、覆盖写、新建）。
- 全量 `uv run pytest tests/unit -q` 绿。

前端（P1-b 交付时）：

- `tsc`、`npm test`、build 绿；meta 解析单测（有 meta / 无 meta
  fallback / 畸形 meta 容错）。
- dev-only legacy 计数器 + 手工冒烟：新会话七类工具各一次，卡片数据
  与 meta 一致、计数器为零；旧会话渲染不回归（人工抽查，不做
  逐像素自动断言）。

## 4. Backlog（记录不做）

- send_file 错误分支 `state=SUCCESS` 误标（send_file.py:50-69,96-104）
  ——真 bug，但改 state 影响模型行为，独立小 PR 修，不混入本期。
- hosted web_search 结构化 sources（等 provider 返回结构化数据后
  `source_count` 升为共同字段）。
- deprecated GuardedFunctionTool 退役；`ToolCallContext.
  governance_metadata` 无读写者待处置；audit 同步写（归 P1.5）；
  非 web 渠道 renderer 消费 meta。

## 5. 工程纪律

同 P0：每 commit 全绿再前进；不引新依赖；小毛病记录延后；
被边角卡住一次即取保守实现并记录。

## 6. 附录：文本反推点迁移核对表（审查 P1-1，入库版）

前端（F）：

| # | 位置 | 现在解析什么 | 迁移目标 |
|---|---|---|---|
| F1 | FileToolCard.tsx:304-313 | `/(\d+)\s*bytes/i` 提文件大小 | `qp.data.bytes_written` |
| F2 | FileToolCard.tsx:315-339 | `text === "File sent successfully."` | `qp.data.attached` |
| F3 | FileToolCard.tsx:341-353 | 试探 size_bytes/byte_size/size 三键 | `qp.data.size_bytes` |
| F4 | ToolCard.tsx:292-310 | JSON.parse 嗅探 output 形状 | 保留（output 编码所需），meta 不经过它 |
| F5 | ToolCard.tsx:276-290 | 参数 JSON 摘要 | 保留（参数侧，非结果反推） |
| F6 | ShellToolCard.tsx:98-105 | 参数 JSON 提 command | 保留（参数侧）；exit_code/sandboxed 改读 qp |
| F7 | fileChanges.ts:145-197 | 参数 old/new LCS 算±行 | `qp.data.additions/deletions`（write/append/edit 全覆盖），无 meta 回落 LCS |
| F8 | conversationArtifacts.ts:100-186 | 参数+markdown 链接推产物 | 产物判定改 `qp`(file_write/file_sent ok=true)优先，链接推导保留 |
| F9 | FileToolCard.tsx:375-394 | 参数 content/old/new 做卡体 | 保留（展示原文来自参数是正确的） |
| F10 | unifiedDiff.ts:41-190 | git CLI 输出 | 维持（数据源是 git） |
| F11 | ProgressCard.tsx:83-92 | 标题启发式 | 维持（纯呈现） |

后端（B）：

| # | 位置 | 处置 |
|---|---|---|
| B1 | tool_adapter.py:442-455 | metadata 优先已是现状；文本 fallback 标 legacy |
| B2/B3 | run_tool_batch.py:119-206 | `qp.ok` 优先，回落现有链 |
| B4 | run_tool_batch.py:96-116 | 保留（纯文本抽取工具函数） |
| B5 | utils.py:340-377 | 保留（历史数据 truncation legacy） |
| B6 | agentscope _agent.py:1743-1789 | 不动（框架所有） |
| B7 | _coordinator.py:216-235 | 不动（text/metadata 双写已合理） |
| B8 | channels/renderer.py:118-146 | 不动（结构→文本正向渲染） |
