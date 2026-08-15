# 任务：对抗审查 RFC《Tool Runtime》（只审查，不写代码）

审查 `docs/rfc/tool-runtime.md`。产出 `docs/rfc/tool-runtime-review-r1.md`，
P0/P1/P2 分级，每条给 file:line 证据 + 一句话问题 + 一句话修正建议。

## 重点核查

1. RFC 引用的现状 file:line 是否属实（envelope.py 丢 metadata 的位置、
   FunctionCallOutput extra="allow"、chats/utils.py 读回分支、
   tool_adapter/run_tool_batch 的文本嗅探）。
2. §1.2 的 kind 表：逐个对照工具真实实现，字段是否可得？
   （如 edit_file 现有实现能否精确数 additions/deletions；shell 工具
   能否拿到 exit_code；send_file 的 delivered 语义。）漏了哪个前端
   正则实际提取、但表里没有的字段？
3. 传输链完整性：TOOL_RESULT_END 之外，是否有别的路径产出
   FunctionCallOutput（取消路径 collect_tool_output、部分保存、
   历史读回、tool_calls 路由的独立 SSE），漏透传 meta 会不会造成
   同一会话内 meta 时有时无？
4. AgentScope 约束：ToolResultBlock.metadata 在 context 持久化里
   是否真的保存（决定历史读回能否带出 meta）？pruning middleware
   会不会剥掉 metadata？
5. 流式工具（多 chunk）：metadata 在 chunk 合并（_drain/append_chunk）
   时的语义——最后一个 chunk 的 meta 生效还是合并？RFC 没写，需要定。
6. 验收标准可执行性。

## 纪律

- 不挑战既定分层（meta 走 metadata["qp"]、前端正则降级保留、
  治理链路不动、非目标清单）。
- 只报会导致返工或 bug 的问题；风格与理论洁癖不报。
- 只读代码 + 写报告，不改任何其他文件。
