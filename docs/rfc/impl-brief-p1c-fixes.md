# 任务：P1-c 审查修复批（后端，3 个小 commit）

依据 `docs/rfc/tool-runtime-review-c.md` 的裁决执行。P1-1（前端负数
exit code）架构师已修（18ee8a72），不要碰。**不碰 `app/`。**

## Commit 1 — P1-2：coordinator 聚合点执行 qp 终态唯一性

`src/qwenpaw/tool_calls/_coordinator.py` 的 chunk 聚合处（:548-556 附近）：
非终态 chunk 的 metadata 若含 `"qp"` 键，**剥离该键并 log warning**
（防御性丢弃，不抛错——工具犯规不应中断执行；终态 chunk 的 qp 正常
保留）。用真实 `ToolCoordinator.execute()` 加回归：
「中间 chunk 带 qp + 终态 chunk 无 qp」→ 最终 metadata 无过期 qp；
「中间带 qp + 终态带 qp」→ 终态值胜出。审查报告说它实测过前者失败,
以它的复现方式为准。`assert_qp_terminal_chunk` helper 若因此失去存在
意义可删除或改造为该剥离逻辑的实现,不留无调用者的死代码。

## Commit 2 — P2-1 轻量版：kind-aware 必填字段校验

`runtime/tool_meta.py` 的 validator 加每 kind 必填字段表（RFC §1.2 的
always 列 + ok=true 列),`build_qp_meta` 构造时校验:缺必填/多拼错键
（未知键警告即可,不拒绝——前向兼容）在单测里直接失败。只改后端
单一源头,不做跨端 schema 生成。为 7 个 kind 各加一个"拼错键名被
抓住"的负例测试。

## Commit 3 — P2-2 收敛版：一条真实整链路测试

一个测试:真实工具产出（用 write_file 或 stub 工具经真实 coordinator）
→ AgentScope END 事件 → Envelope → `FunctionCallOutput.meta` 断言 →
经 chats/utils 历史读回再断言。只此一条,不铺矩阵。

## 纪律

- 每 commit `uv run pytest tests/unit -q` 全绿（3 个 doubao .env 失败
  忽略）。不引新依赖,不碰 app/,不顺手重构。
- 完成后在 `docs/rfc/impl-p1a-report.md` 末尾追加「P1-c 修复批」一节。
