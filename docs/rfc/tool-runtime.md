# RFC: Tool Runtime — 结构化工具结果契约（P1）

状态：Draft r1（架构师：Claude；后端实施：codex；前端呈现：Claude 亲自）
分支：`refactor/runtime-kernel`（承接 P0）
前置调研：全链路地图（本文件引用的 file:line 均已核实）。

## 0. 本质与判断

现状：`ToolChunk.metadata`（AgentScope 原生字段，文档原话"让 agent 不必
解析工具结果文本"）从工具产出一路携带到 `ToolResultEndEvent`
（agentscope/agent/_agent.py:1798-1803），**在 envelope.py:607-641 被丢弃**。
因此前端 11 处、后端 8 处在用正则/字符串匹配从 prose 里反推结构
（完整清单见 §5）。`FunctionCallOutput` 为 `extra="allow"`
（schemas.py:139-149），加字段向后兼容。

P1 的本质 = **接通这一跳 + 为 metadata 立契约 + 两端迁移到契约上**。

明确非目标：

- 不重写治理链路。`PolicyGuardedTool` 已经是统一管线（policy →
  sandbox → approval → audit 三审计点齐全），不动。
- 不动 AgentScope 内部的 `<<<TRUNCATED>>>` prose 信号（B6，框架所有）。
- 不动非 web 渠道的 prose 渲染（B8，那是结构→文本的正向渲染）。
- 不删旧数据兼容：历史会话没有 meta，前端正则**降级保留**为 fallback。
- deprecated `GuardedFunctionTool`（runtime/tool_guard.py）本期不退役，
  记 backlog。

## 1. 契约设计（Claude 拥有，review 可挑战字段但不挑战分层）

### 1.1 载体与命名空间

工具把规范化结果放进 `ToolChunk.metadata["qp"]`（单一命名空间键，
避免与 `sandbox_violation`、`qwenpaw_truncation` 等既有散键混淆；
既有键不迁移、不删除）：

```python
metadata["qp"] = {
    "v": 1,                  # 契约版本
    "kind": "file_write",    # 结果种类，见 §1.2
    "ok": True,              # 语义成败（区别于 ToolResultState 的执行状态）
    "data": {...},           # kind 专属规范化数据，见 §1.2
}
```

设计规则：

- `data` 只放**机器可信字段**（路径、字节数、行数、退出码），
  不放展示文案——展示是前端的事，后端不预渲染 UI 字符串。
- 一律 JSON-serializable；禁止把大内容（文件全文、完整 diff）塞进
  meta——大内容仍走 content blocks，meta 只放刻画性事实。
  单条 meta 序列化后 > 4KB 视为契约违规（测试断言）。
- 工具**可以不产 meta**（渐进迁移）；消费方必须容忍缺失。

### 1.2 首批 kind（覆盖现有全部正则反推点）

| kind | 产出工具 | data 字段 |
|---|---|---|
| `file_write` | write_file / append_file | `path`, `bytes_written`, `lines_written`, `created`(bool) |
| `file_edit` | edit_file | `path`, `replacements`, `additions`, `deletions` |
| `file_read` | read_file | `path`, `bytes_read`, `line_start`, `line_end`, `total_lines` |
| `shell` | execute_shell_command | `exit_code`, `sandboxed`(bool), `violation`(str, 可缺) |
| `file_sent` | send_file_to_user | `path`, `size_bytes`, `delivered`(bool) |
| `web_search` | web_search | `backend`("hosted"/"tavily"), `source_count` |
| `batch` | run_tool_batch | `steps`: [{`tool`, `ok`, `kind`(可缺)}] |

字段语义以现有前端正则**实际提取的东西**为准（F1 提 bytes、F7 算
±行数、F2 判 delivered……），不发明前端不消费的字段。
`file_edit.additions/deletions` 由后端在真实执行处精确计数，替代前端
F7 的 LCS 近似（其 `:157-158` 注释自认全局替换计数不准——这是本次
少数"顺带修正确性"的点，因为后端有真值而前端只能猜）。

### 1.3 传输与持久化

1. **Envelope**：`TOOL_RESULT_END` 处理（envelope.py:607-641）把
   `event.metadata["qp"]`（若存在）原样放入 `FunctionCallOutput` 新增
   字段 `meta`。只透传 `"qp"` 键，其他 metadata 键不上行（沙箱违规等
   内部键不进前端）。
2. **持久化读回**：`chats/utils.py:626-651` 的 tool_result 分支同样
   回填 `meta`（历史消息里 AgentScope context 的 `ToolResultBlock.
   metadata` 已存在即带出）。
3. **工具调用帧带 UI spec**：`TOOL_CALL_START`（envelope.py:421-460）
   在 `FunctionCall` 上新增 `ui: {icon, display}`（来自 ToolUISpec，
   经 ToolDescriptor 查询）。前端 ToolCard 的硬编码 icon/名称表
   （ToolCard.tsx:313-400）降级为 fallback。

### 1.4 后端消费迁移

- B1：`tool_adapter.py:442-455` 只读 `metadata["sandbox_violation"]`
  （已是主路径），文本 split fallback 保留但加注释标记 legacy。
- B2/B3：`run_tool_batch` 的 `_is_error_text` 前缀嗅探改为优先读
  子工具 `metadata["qp"].ok`，缺失时回落现有嗅探。

## 2. 分工与顺序

1. **P1-a（codex）**：契约类型（`runtime/tool_meta.py`，TypedDict/
   dataclass + 校验帮助函数）、7 个 kind 在各工具的产出、envelope 透传、
   持久化读回、B1/B2 消费迁移、单测。纯后端，不碰 app/。
2. **P1-b（Claude 亲自）**：前端消费——`ToolPair` 增加 `meta` 解析、
   F1-F9 逐个改为 meta 优先/正则回落、卡片视觉按需微调、icon 走
   `ui` 字段。涉及呈现判断，不派 codex。
3. **P1-c（codex review）**：对 a+b 整体对抗审查。

## 3. 验收标准

- 新会话里：写/编辑/读文件、shell、send_file、web_search、batch 各来
  一次，SSE 帧携带正确 `meta`，前端卡片数据与 meta 一致且**未触发
  任何 legacy 正则路径**（前端加 dev-only 计数器断言）。
- 旧会话读回：无 meta 的历史消息卡片渲染与现状逐像素一致（正则
  fallback 生效）。
- `file_edit` 的 ±行数与 `git diff --numstat` 对同一编辑的计数一致
  （新单测，替代 LCS 近似）。
- meta 体积断言：所有内建工具单测中 meta ≤ 4KB。
- 全量单测 + 前端 `npm test` / `tsc` / build 全绿。

## 4. Backlog（记录不做）

- deprecated `GuardedFunctionTool` 退役（等确认无 governor=None 场景）。
- `ToolCallContext.governance_metadata`（_context.py:44）无读写者，
  待 Tool Runtime 二期决定用途或删除。
- audit 同步写在事件循环线程（audit.py:99-104 已有 TODO）——归
  P1.5 持久化下移。
- F10（git diff 解析）与 F11（标题启发式）维持现状：前者数据源是
  git 本身，后者是纯呈现启发式。
- 非 web 渠道 renderer 消费 meta 优化 prose 输出。

## 5. 附：文本反推点全清单（迁移核对表）

前端 F1-F11 与后端 B1-B8 的 file:line 与提取内容，见全链路地图
（对话记录 2026-08-14）；F1-F9 本期迁移，F10/F11 维持，B1-B3 迁移，
B4 是纯文本抽取工具函数（保留），B5 truncation legacy 正则保留
（历史数据），B6/B7/B8 不动。
