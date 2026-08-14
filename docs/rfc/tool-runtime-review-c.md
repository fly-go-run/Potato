# Tool Runtime P1-c 整体对抗审查

审查范围：`git log 4dfc4fa5..HEAD` 中 Tool Runtime P1 实现，重点覆盖
`ef03fd59` / `72a4cd60` / `2d7548fb` / `075fd76c` 后端与
`7c7dfba7` 前端。基准为 `tool-runtime.md` r2 与
`impl-p1a-report.md`。

## 结论

- P0：0
- P1：2
- P2：2

七个 kind 的当前后端键名未发现拼写漂移：
`bytes_written` / `size_bytes` / `additions` / `deletions` /
`exit_code` / `sandboxed` 皆与 RFC 一致。但 shell 的负数
`exit_code` 在前端被通用 count accessor 过滤，是当前可触发的静默失效。

## P1

### P1-1 负数 shell exit code 被前端当成非法 count 静默丢弃

后端明确以 `-1` 表示超时，并会把该值写入
`qp.data.exit_code`：`src/qwenpaw/agents/tools/shell.py:591-592`,
`src/qwenpaw/agents/tools/shell.py:778-785`,
`src/qwenpaw/agents/tools/shell.py:837-850`。POSIX 子进程被信号终止时也可产生
负数 return code，并走同一个产出分支。

前端却用“非负数计数” accessor 读它：
`app/src/lib/toolMeta.ts:43-48` 对任何 `< 0` 值返回 `null`，
`app/src/components/chat/ShellToolCard.tsx:26-27` 直接用该 accessor，
`app/src/components/chat/ShellToolCard.tsx:39-47` 只在非 `null` 时展示。因此超时/
信号退出虽然 meta 完整到达，UI 仍不显示 exit code。

测试反而固化了这个错误边界：`app/src/lib/toolMeta.test.ts:53-59`
断言负数必须返回 `null`，而 shell 产出测试只覆盖 `0` 和 `1`：
`tests/unit/agents/tools/test_shell.py:540-549`,
`tests/unit/agents/tools/test_shell.py:589-596`。

影响：shell 详情的终态信息不完整，且无报错/无 legacy 计数，属于
跨端静默失效。`exit_code` 需要独立的有符号整数 accessor，不应复用
count 语义。

### P1-2 `qp` 终态唯一性没有在真实 coordinator 聚合路径上执行

`assert_qp_terminal_chunk()` 确实会拒绝中间 chunk 的 `qp`：
`src/qwenpaw/runtime/tool_meta.py:157-169`。但生产聚合点
`src/qwenpaw/tool_calls/_coordinator.py:548-556` 在把 `ToolChunk` append 进
`final_response` 前没有调用该检查；该 helper 在实现代码中无调用者。

这不只是缺测试：当一个中间 chunk 误带 `qp`、真正终态 chunk 不带
`qp` 时，coordinator 的最终 `ToolResponse.metadata` 会保留那个过期的
`qp`，后续 END 透传就会把中间态误当终态真值。本次审查用真实
`ToolCoordinator.execute()` 对“中间 qp + 终态无 qp”序列实测，最终响应
仍含中间 `qp`。

现有回归测试绕过了这个聚合点：
`tests/unit/runtime/test_tool_meta.py:56-74` 手工创建 `ToolResponse`、手工调用
assert helper 后直接 `append_chunk()`，因此只证明了一组合作输入能成功，
没证明 coordinator 会保护“终态原子值/唯一写入”契约。

影响：任一流式工具或后续插件的单点产出错误都会被聚合层放大成错误
终态 meta，并静默进入 SSE/历史。终态约束应放在 coordinator 这个共用
聚合边界，并以 coordinator 端到端序列回归覆盖。

## P2

### P2-1 两端解析器都不验证 kind-specific data schema，键名回归仍会静默通过

后端 validator 只验证顶层键、版本、kind 枚举、`ok`/`data` 类型与
4KB，并不验证每个 kind 的 required/optional 字段和字段类型：
`src/qwenpaw/runtime/tool_meta.py:35-41`,
`src/qwenpaw/runtime/tool_meta.py:57-78`。前端解析器也只验证顶层形状：
`app/src/lib/toolMeta.ts:34-40`。

因此例如 `file_write.data.byte_written` 这类拼写错误会在两端都被视为
“合法 meta”。到具体消费点才变成字段缺失；例如文件大小消费者在
`app/src/components/chat/FileToolCard.tsx:309-313` 会因为“meta 存在”而不再走
legacy fallback，最终是无报错的空大小。

本版人工逐项核对未发现现有产出器拼错，但“单测抓不到跨端漂移”的
结构性风险仍在。建议至少增加一份两端共用的契约 fixture/生成 schema，
或让后端 validator 执行 kind-aware schema 验证。

### P2-2 “六路径已覆盖”的测试口径是孤立 seam 测试，未证明真实整链路

主 SSE 终帧测试直接构造已经带 `metadata={"qp": ...}` 的
`TOOL_RESULT_END` 事件：`tests/unit/runtime/test_envelope_metadata.py:355-391`；
`/output` 测试直接构造已带 qp 的 fake `final_response`/entry：
`tests/unit/app/routers/test_tool_calls_router.py:23-45`；历史测试直接构造已带
qp 的 `ToolResultBlock`：`tests/unit/app/chats/test_utils.py:21-44`。split 测试直接
调用 override，没有通过 AgentScope acting → split → save/history 路径：
`tests/unit/agents/test_tool_result_pruning_middleware.py:655-702`。

这些测试能证明每个局部函数在理想输入下正确，但无法发现“产出器 →
coordinator 聚合 → AgentScope END 事件 → Envelope → 历史读回”之间的丢字段或
附错帧；P1-2 就是此口径未捕获的实例。同样，前端只有 parser 与
`fileChanges` 的 meta 测试，没有 `FileToolCard` / `ShellToolCard` 对“无 meta / 畸形
meta / 合法但缺字段”的渲染级回归：`app/src/lib/toolMeta.test.ts:12-65`,
`app/src/lib/fileChanges.test.ts:476-537`。

这与 RFC 验收中“传输矩阵全路径”和“前端三态容错”的字面口径不等价，
后续修 P1-2 时应一并补一条真实整链路测试，否则传输层再次重构仍容易
静默回归。

## 逐项核对（无额外 finding）

### kind 与字段映射

| kind | 后端产出 | P1-b 消费 | 结论 |
|---|---|---|---|
| `file_write` | `path`, `bytes_written`, `additions`, `deletions`, `created` (`file_io.py:352-360`, `:584-592`) | `bytes_written` (`FileToolCard.tsx:309-313`), `additions`/`deletions` (`fileChanges.ts:149-157`) | 键名一致 |
| `file_edit` | `path`, `replacements`, `additions`, `deletions` (`file_io.py:510-517`) | `additions`/`deletions` (`fileChanges.ts:149-157`) | 键名一致 |
| `file_read` | `path`, `bytes_read`, `line_start`, `line_end`, `total_lines` (`file_io.py:241-251`) | P1-b 无对应结构展示目标 | 符合当前迁移范围 |
| `shell` | `sandboxed`, `exit_code?`, `violation?` (`shell.py:34-46`, `:680-686`, `:711-715`, `:846-850`) | `sandboxed`, `exit_code` (`ShellToolCard.tsx:23-47`) | 键名一致；数值域见 P1-1 |
| `file_sent` | `path`, `size_bytes`, `attached` (`send_file.py:101-106`) | `size_bytes` (`FileToolCard.tsx:309-313`)；产物成功以 `qp.ok` 否决 (`FileToolCard.tsx:52-58`) | 键名一致；`attached` 与当前后端产出的 `ok=true` 同步，未发现当前误判 |
| `web_search` | `backend`, Tavily `source_count?` (`web_search.py:448-456`, `:469-474`) | P1-b 无对应结构展示目标 | 符合当前迁移范围 |
| `batch` | `total`, `completed`, `failed`, `truncated`, `steps` (`tool_meta.py:125-154`, `run_tool_batch.py:973-985`) | P1-b 无对应结构展示目标 | 键名与边界逻辑一致 |

### 前端三态回落与 running

- `parseQpMeta()` 对无 meta/畸形 meta 统一返回 `null`：
  `app/src/lib/toolMeta.ts:30-40`。
- `FileToolCard` 在无 meta/畸形 meta 时走旧文本解析；对形状合法但
  大小缺失的 meta 不猜测：`app/src/components/chat/FileToolCard.tsx:309-326`。
- `fileChanges` 在两个计数都存在时用 meta，否则回落 LCS/全文计数：
  `app/src/lib/fileChanges.ts:146-209`。
- `ShellToolCard` 对无 meta/畸形 meta/缺字段都静默隐藏对应 badge：
  `app/src/components/chat/ShellToolCard.tsx:23-49`；负数值域例外见 P1-1。
- `isSuccessfulArtifactPair()` 的 `ok=false` 先行否决不会把 running 态误记为
  成功产物：`app/src/components/chat/FileToolCard.tsx:52-58`。显式发送在运行中仍由
  `shouldPresentArtifactPair()` 保持突出位置：
  `app/src/lib/conversationArtifacts.ts:80-90`；真正产物卡还有 `!running`
  终态门槛：`app/src/components/chat/FileToolCard.tsx:111-119`。

### 传输、split、内容等价与边界

- Envelope 仅 END 把 `event.metadata["qp"]` 写入 `FunctionCallOutput.meta`，
  TEXT/DATA_DELTA 调用未传 meta：`src/qwenpaw/runtime/envelope.py:572-657`,
  `src/qwenpaw/runtime/envelope.py:804-847`。
- 取消补写的 `ToolResultBlock` 无 qp：`src/qwenpaw/runtime/runtime.py:438-458`。
- 历史回填位置和 `/output` 位置正确：`src/qwenpaw/app/chats/utils.py:616-659`,
  `src/qwenpaw/app/routers/tool_calls.py:224-237`。
- split override 对 reserved/offloaded 都使用 `deepcopy`：
  `src/qwenpaw/agents/react_agent.py:225-240`。
- 对实现 diff 逐分支核对，工具的 TextBlock/DataBlock 文本和块顺序未被
  qp 改写；`run_tool_batch` 的 summary/content 构造也保持原语义：
  `src/qwenpaw/agents/tools/run_tool_batch.py:950-997`。未发现模型可见 content
  block 回归。
- 4KB 用 UTF-8 序列化字节数严格限制 `> 4096`：
  `src/qwenpaw/runtime/tool_meta.py:44-77`；batch 先限 50，再删除完整尾 step
  直到满足 4KB：`src/qwenpaw/runtime/tool_meta.py:125-154`。对应的 50/51 与
  超 4KB 测试在 `tests/unit/agents/tools/test_tool_meta_producers.py:85-121`。

## 审查验证

- 后端定向回归：234 passed。
- 前端定向回归：3 files / 69 tests passed（`toolMeta`, `fileChanges`,
  `ConversationSidePanel`）。
- 本次只新增本审查报告，未修改任何实现代码。
