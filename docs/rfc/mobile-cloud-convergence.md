# RFC：手机端与云端的记忆、检索与运行时收敛

状态：**v2**（2026-09-13）。v1 由 Claude 起草，经 codex（gpt-6-astra, high）只读对抗评审后修订。评审原文：`mobile-cloud-convergence-review-codex.md`。v1 → v2 的变更清单见第 8 节。
前置：`docs/design/iphone/ios-architecture-review-20260913/`（复盘、审查、工作包 1/2 报告）。

## 0. 范围与结论

本 RFC 决定复盘中暂缓的四项。

| 项 | 决定 |
|---|---|
| E2B 检索替换 | **做**（P1）。Worker 内 TypeScript 实现替换 Python 沙箱，用固定语料做等价测试。 |
| 记忆合并 | **做，但改名为「云记忆挂载」**（P2）。个人记忆的真相源是云端 R2 manifest；Mac 通过 `remember` / `forget_memory` / `memory_search` 挂载它。Mac 的全局 Markdown 记忆继续存在，改称「Mac 本地笔记」，不再宣称「唯一真相源已收敛」。 |
| D1 FTS5 | **不做**。保留 schema 草案与重开条件。 |
| core 上云 | **不做**。Worker 定位为「云端轻运行时」；远程 MCP 首版只接只读工具；纯内核 crate 仅作方向。 |

新增 **P0**：先把工作包 1/2 的 Worker 改动部署并做生产链路验证，再动上面任何一项。

非目标：不改 iOS 本机聊天 UI；不改 SSE 事件名；不做跨设备对话同步（Mac JSONL 历史不上 R2）；不做审批流上云；不做双向冲突合并。

## 1. 现状事实（v2 核对后）

- 个人记忆两份存储。Mac：`workspace/memory/*.md`，`MEMORY.md` 前 2000 字节注入 prompt，`memory_search` / `memory_write` 工具，写入过审批与 `expected_content` CAS，无自动记忆（`native/potato-core/src/memory.rs:250-273`, `tools.rs:272-301, 337-350`）。云端：R2 `recall/<sha256(owner)>/manifest.json` 的 `memories[]` `{id, text, revision, updated, sources[], forgotten}`，上限 200 条（`native/potato-worker/src/recall.ts:8-10, 113-137`）。
- 云端记忆有效性：`validMemories` 排除 `forgotten`，并要求每个 `source` 所属对话仍存在、未排除、revision 一致（`recall.ts:110-111`）。**对话被追加时**，sync 会更新来源的 revision 并保留记忆（`recall.ts:63-69`），不是「revision 一变即失效」。`sources` 为空数组的记忆恒有效；`POST /v1/recall/memory` 新建的记忆 `sources` 恒为空（`recall.ts:120`）。
- `POST /v1/recall/memory` 的校验：`id` 必须 uuid，`text` **必须是字符串**（forget 时传空串），`forget` 布尔，`base` 为 null 或 64 位 hex（`recall.ts:114`）。
- 跨对话检索：`search_conversations` 按 `conversation_offset` 分页，每页最多 10 个对话、合计 ≤ 1.5 MB，写入 E2B 跑固定 Python（`recall.ts:149-165, 225-236`）。10 个是**每页上限**，不是历史硬窗口；模型可翻页。
- 账号标识：`/v1/recall/*` 与 `/v1/desktop/chat/completions` 都走 `cloudIdentity`（`index.ts:103-118`, `cloud.ts:62-70`），owner 为 `hash(issuer + sub)`（`remote-auth.ts:31`），只接受 `cloud` 角色会话（`remote-auth.ts:64`）。Mac 的 `cloud.rs:69` 与 iPhone 的云登录都以 `cloud` 角色登录。**结论：同一 Cloudflare 身份在两端解析为同一 recall 账号。** 个人令牌模式则固定 `personal-client`。
- Worker 无 `limits` 配置；一次回答最多 8 次工具调用（`search-chat.ts:18`）。
- potato-core 无 Linux 沙箱（`sandbox/mod.rs:10-18`）；`context_policy.rs:49-58` 仍耦合核心错误类型，不是零成本可抽出的纯 crate。

## 2. P0：生产链路验证（新增）

工作包 1/2 只有本地测试。在动本 RFC 任何一项之前：

1. 部署 Worker（`wrangler deploy`），确认 `/v1/chat/completions` 对带 `tool_calls`/`tool` 历史的请求返回 200，对四种非法配对返回 400。
2. 真机或模拟器走一次「搜索 → 追问」和「Python 两次调用，第二次读第一次产物」。
3. 自动记忆关闭时，用真实模型确认 `forget_memory` 不出现在工具列表。
4. 远程模式打开一个 Mac 会话，确认工具行显示真实工具名与参数/输出分栏。

## 3. P1：E2B 检索替换为 Worker 内实现

### 设计

- 新增 `src/recall-search.ts`，导出 `searchConversations(input): {sources, more}`，签名与 `RecallExecutor` 一致。语义逐条对应 `RECALL_SEARCH_CODE`：
  - 规范化：`normalize('NFKC')` + `toLowerCase()`。与 Python `casefold` 的差异（如 `ß`→`ss`）接受，写进测试说明。
  - 分词：Python `str.split()` 按任意 Unicode 空白切分，JS 用 `/\s+/u` 而不是 `split(' ')`。
  - 截断：Python 按码点切 4000，JS 用 `Array.from(text).slice(0, 4000).join('')`，不用 `slice`（UTF-16 单元）。
  - 计分与排序：整句命中 10，否则命中词数；`(score, date)` 降序；日期按 ISO 字符串比较。
  - 分页：`offset` 取 8 条，`more = rows.length > offset + 8`。
  - 取消：每处理 50 个消息检查一次 `signal.aborted`。
- `e2bRecallExecutor` 删除；`index.ts` 的 recall 前置条件从 `RECALL_BUCKET && E2B_API_KEY` 改为只要 `RECALL_BUCKET`；`SANDBOX_RATE_LIMIT` 不再对检索计费。
- 候选选择与分页逻辑不动。

### 验证门槛

- **等价测试**：固定一份 12 个对话、含中英混排、全角标点、Unicode 空白、超 4000 码点消息、跨日期的语料，把当前 Python 脚本在本机跑一次得到期望输出存为 fixture，`searchConversations` 必须逐字节相等（除 casefold 差异的显式例外）。
- **CPU**：不看单次墙钟。本地 miniflare 用 `--cpu-profile` 或 `performance.now()` 测 8 次连续搜索的累计 CPU；部署后看 observability 的 invocation CPU p95/p99 与超限率一周。付费计划在 `wrangler.jsonc` 显式写 `limits.cpu_ms`。上线前确认账号是 Workers Paid。

## 4. P2：Mac 挂载云记忆

### 定位（v2 改）

- **个人记忆**：真相源是云端 manifest，Mac 只持缓存。
- **Mac 本地笔记**：现有 `workspace/memory/*.md` 与 `MEMORY.md`，继续存在、继续注入，但 guidance 里改名为「Mac local notes」，并告诉模型：稳定的个人事实用 `remember`，Mac 专属工作笔记用 `memory_write global`。
- 不宣称两者已合一。手机上删除的记忆，Mac 缓存刷新后消失；Mac 本地笔记里若有同样事实，需用户自己清理。这是有意接受的边界。

### Mac 端（`src/cloud_memory.rs`）

- **激活条件**：`cloud_config.session_token` 有效。未登录云端时模块不存在，行为与今天一致。
- **缓存**：KV 键 `cloud_memory_cache:<sha256(service_url + owner)>`，值 `{memories, fetched_at}`。**退出云端账号时删除**。不同账号不共用键。
- **刷新**：`run_turn` 开始时若缓存超过 5 分钟，后台刷新一次 `GET /v1/recall/status`；失败保留旧缓存，guidance 标注「离线，数据截至 T」。
- **注入**：`memory_guidance` 增加「Personal memory (cloud, N items, synced T)」段，每条一行、截 120 字符、总量 ≤ 4000 字节，超出只列最近更新的；走现有 `memory_context_fingerprint` 去重。
- **工具**：`tool_registry.rs` 新增 `remember {text}` 与 `forget_memory {id}`，wire 名与 Worker 一致；`memory_search` 同时扫缓存，结果带 `scope: "cloud"`。
  - `remember`：调用前生成 `id`，`POST /v1/recall/memory {id, text, base: null, forget: false}`。
  - `forget_memory`：`POST /v1/recall/memory {id, text: "", base: <缓存中的 revision>, forget: true}`。**`text` 必须传空字符串**，否则被 `recall.ts:114` 拒绝。
  - **写入结果确认（v2 改）**：网络错误或超时不直接报失败。用同一个 `id` 立即 `GET /v1/recall/status` 回读：`remember` 看该 id 是否已存在，`forget` 看该 id 是否已 `forgotten` 或不在有效列表。回读也失败才报「云端记忆暂不可用，未确认是否已保存」。不重新生成 id，不排队。
  - **审批**：与 global `memory_write` 同档，永不自动放行。审批卡展示完整记忆文本（或被删除记忆的原文）与云端账号邮箱，不只复用理由字符串。
- **CAS**：`forget` 带 `base`，Worker 409 时刷新缓存后报「记忆已在别处修改，请重试」。

### Worker 端

- 无需改协议。补两个测试固定语义：空 `sources` 的记忆恒有效；`forget` 缺 `text` 返回 400。
- 前置验证从「待验证」改为「已由代码判定」（第 1 节）。运行态只需确认两台设备登录的是同一个 Cloudflare 身份。

### 手机端

不改。

## 5. D1 FTS5：不做

### 理由（v2 改）

- 现有实现已有 `conversation_offset` 翻页，覆盖不足的问题是「模型是否会翻页」而不是「窗口是否存在」。先测覆盖率与翻页成本。
- 中文：`unicode61` 不切分 CJK（按标点与空白分段，段内整体一个 token），`MATCH` 基本失效；`trigram` 对 `MATCH` 要求 ≥ 3 字符，两字词要退回 `LIKE`。既然要 `LIKE` 兜底，FTS5 的增益有限。
- 一致性：外部内容表（`content=`）需要触发器维护，加上 manifest revision 校验、排除、删除、版本回退的双写，复杂度不抵收益。

### 重开条件

任一满足：用户明确遇到搜不到更早对话且翻页无效；P1 上线后 `search_conversations` p95 > 2 s；对话数 > 500。

### schema 草案（不实施）

```sql
CREATE TABLE recall_messages (
  account TEXT NOT NULL, conversation TEXT NOT NULL, message TEXT NOT NULL,
  version TEXT, revision TEXT NOT NULL, role TEXT NOT NULL, date TEXT NOT NULL,
  text TEXT NOT NULL, PRIMARY KEY (account, conversation, message, version)
);
CREATE VIRTUAL TABLE recall_fts USING fts5(text, content='recall_messages', content_rowid='rowid', tokenize='trigram');
-- 需 INSERT/UPDATE/DELETE 三个触发器维护 recall_fts；查询 ≥3 字符 MATCH，否则 LIKE；结果按 revision 与 manifest 核对。
```

## 6. core 不上云；Worker 作为「云端轻运行时」

### 不上云的理由（v2 措辞修正）

potato-core 依赖 Seatbelt/LPAC、cap-std、子进程 MCP、本地 SQLite + JSONL，且没有 Linux 沙箱后端。上云不是「必须容器」，而是「要先做一个不存在的沙箱后端再谈部署形态」。手机「Mac 不在线也能干活」的诉求用下面两条满足。

### 路线 A：Worker 能力边界

- 允许：模型代理、无副作用工具（搜索、沙箱计算、历史检索）、显式开关的记忆写入、远程 MCP **只读工具**。
- 不允许：shell、文件写入、审批流、计划任务、MCP 写工具。
- **远程 MCP（P4，用户拍板后做）**：
  - 配置存 `RemoteAccount` DO，键 `mcp:<key>` → `{name, url, headers, enabled, tools[]}`；手机设置页管理。
  - 目录：用户点「获取工具」时连一次 `tools/list` 并缓存；聊天时不连。
  - **闸门（v2 改）**：首版只暴露 `annotations.readOnlyHint === true` 的工具；无该注解或为 false 的工具在列表里灰显「需要桌面端审批」，不可启用。写工具等 Worker 有审批机制再说。
  - 执行：`@modelcontextprotocol/sdk` streamable HTTP，15 s 连接、60 s 调用、2 MB 结果，结果按不可信数据注入。
- **工具注册表配置化（P3，v2 改为随 P4 一起做）**：`availableTools` 抽成 `{name, definition, enabled, call}` 列表，只在新增 MCP 工具时实施，不单独排期。

### 路线 B：纯内核 crate（方向，不排期）

抽 `context.rs`、`context_policy.rs`、`protocol.rs` 帧定义与 `ToolDispatch` trait 为 `potato-agent`。已知代价：`context_policy.rs:49-58` 耦合 `Error` 类型需解耦，宿主适配与 wasm 验证未做。触发条件：Worker 循环需要压缩，或帧格式需要第二次对齐。

## 7. 实施顺序

| 序 | 包 | 范围 | 门槛 |
|---|---|---|---|
| P0 | 生产链路验证 | 部署 Worker + 4 项端到端检查 | 全部通过 |
| P1 | E2B → Worker 内检索 | Worker | 语料等价测试全等；CPU p95 达标 |
| P2 | Mac 挂载云记忆 | Rust `cloud_memory.rs` + 2 工具 + guidance + 审批卡；Worker 2 个测试 | Mac `remember` 后手机记忆页可见；手机 forget 后 Mac 刷新消失；超时回读确认生效 |
| P3+P4（可选） | 注册表配置化 + 远程 MCP 只读 | Worker + 手机设置页 | 一个公开只读 MCP 服务器端到端 |
| 不做 | D1 FTS5、core 上云 | | 见第 5、6 节 |

## 8. v1 → v2 变更（codex 评审采纳记录）

采纳：
- 新增 P0 生产链路验证，排在一切之前。
- P2 改名「云记忆挂载」，不再宣称唯一真相源；Mac 全局 Markdown 改称本地笔记。
- `forget` 请求补 `text: ""`（确定性 bug）。
- 缓存键按服务 + 账号隔离，退出清除。
- 写入超时不直接报失败，用原 id 回读确认；不排队。
- 审批卡展示全文与账号，不只复用理由。
- P1 补固定语料等价测试，明确 Unicode 空白分词、码点截断、取消语义；CPU 验证改为累计 CPU 与部署后 p95/p99。
- D1 理由改正：10 个是分页上限；外部内容表需触发器。
- MCP 首版只接 `readOnlyHint` 工具。
- P3 注册表配置化并入 P4，不单独做。
- 行号与「revision 一变即失效」的表述修正。

未采纳 / 部分采纳：
- 「先测覆盖率再定是否做 P1」：P1 与覆盖率无关，它只换执行器、去掉一次沙箱冷启动，照做。
- 「2000×512 KB 接近 1 GB 所以扫描不便宜」：D1 本来就不做，且 P1 每次仍只扫一页 1.5 MB，该数字不影响决策。
- 路线 B 的 wasm 验证：仍不排期，不为方向性条目做验证。
