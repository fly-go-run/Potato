# 任务：实施 Tool Runtime P1-a（后端结构化工具结果）

你是实施工程师。方案 `docs/rfc/tool-runtime.md`（r2，已吸收你的审查
`tool-runtime-review-r1.md`）。本次只做 §2 的 **P1-a：纯后端**。
**一行都不要碰 `app/` 目录**（前端由架构师亲自做）。

## 交付物（按 commit 顺序）

1. **commit 1 — 契约基础**：`src/qwenpaw/runtime/tool_meta.py`：
   `build_qp_meta(kind, ok, data)` 构造帮助 + 校验（kind 白名单、
   JSON-serializable、4KB 上限、qp 只允许终态 chunk 的断言帮助）。
   单测含 2-chunk/3-chunk append_chunk 后 qp 等于终态值。
2. **commit 2 — 七个 kind 产出**：按 RFC §1.2 表（含 always/ok=true
   字段规则、不可得事实不填 sentinel、行数 difflib 算法与尾换行规则、
   batch 50 条截断）。每 kind 一个单测 + ok=false schema 断言 +
   行数 fixture（全局多次替换/无尾换行/覆盖写/新建）+ batch 4KB 边界。
3. **commit 3 — 传输矩阵**：按 RFC §1.3 六路径（envelope builder 加
   meta 参数仅 final END 传入、历史读回回填、取消路径明确无 qp、
   `/output` 加 meta、`/stream` 不动）。六路径各一测。
4. **commit 4 — split 保护**：pruning 关闭且结果超 AgentScope 上限时
   metadata 不得丢失（在我方可控层复制回或保证上限不触发，方案自选
   但不得改 vendored agentscope 源码）。pruning 开/关 × 超限测试。
5. **commit 5 — 后端消费迁移**：B2/B3 `run_tool_batch` 读 `qp.ok`
   优先；B1 加 legacy 注释。回归测试。

## 硬性约束

- 不碰 `app/`；不改 vendored agentscope 包源码；不引新依赖。
- 每 commit `uv run pytest tests/unit -q` 全绿再前进（本机 3 个
  doubao ASR 失败是已知 .env 污染，忽略）。
- 行为等价：content blocks 的文本内容一个字符都不变（模型看到的
  东西不变），meta 是纯增量通道。RFC §4 backlog 项一个不做
  （尤其 send_file 的 state 误标——只写 qp.ok=false，不改 state）。
- 抓本质，拒绝完美主义：小毛病记录到报告延后；被边角卡住一次即取
  保守实现（宁可某字段缺省）并记录，继续推进。

## 完成报告

`docs/rfc/impl-p1a-report.md`：每 commit 一段、split 保护采用的方案
说明、存疑点、backlog 触碰记录。
