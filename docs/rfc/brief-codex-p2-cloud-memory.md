# P2：Mac 挂载云记忆（仅 Rust，potato-core）

依据：`docs/rfc/mobile-cloud-convergence.md` v2 第 4 节。你评审过这份 RFC，你的意见已合入 v2（第 8 节）。本包只做 Rust 侧；Worker 侧的两个固定语义测试归 P1 包，不要动 `native/potato-worker`。

约束：
- 仓库根 `/Users/liuxu/lifeProjects/Potato`，分支 `feat/process-track-codex`，工作区有大量未提交改动，**不要 commit、stash、revert 任何你没改的文件**。
- 只改 `native/potato-core/src/`（新建 `cloud_memory.rs`，改 `lib.rs` 挂模块、`tool_registry.rs`、`tools.rs`、`memory.rs` 的 `memory_guidance`、`cloud.rs` 的 logout）与 `native/potato-core/src/cloud_tests.rs` 或新建 `cloud_memory_tests.rs`（在 `lib.rs` 里 `mod`）。不碰 GPUI、iOS、Worker。
- 不追求完美：规格没写的按最简单处理，不顺手重构，不补规格外测试。
- 完成后写 `docs/rfc/p2-cloud-memory-report.md`：改动文件与规格对应位置、真实验证输出摘要、有意跳过或偏离处及理由。

## 已知事实（直接用，不必重新论证）

- 云端会话：`cloud.rs` 的 `cloud_config` KV 含 `relay`、`email`、`expires`、密封的 `session_token`；`alive()` 判有效；`cloud_http(&relay, path, token, body)` 已封装带 Bearer 的 GET（body=None）/ POST（`cloud.rs:305-330`），解封令牌的做法参考 `cloud.rs:236-270` 现有调用。
- Worker 端点（不改协议）：`GET /v1/recall/status` 返回 `{version, scope, entries, memories:[{id,text,revision,updated,sources[],forgotten}]}`；`POST /v1/recall/memory {id: uuid, text: string, base: null|64hex, forget: bool}`，**forget 时 `text` 必须传 `""`**，`base` 不匹配返回 409，新建返回该记忆对象。云端账号在 Mac 与 iPhone 间已确认为同一 recall 账号。
- 记忆 guidance：`memory.rs:250-273` 的 `memory_guidance(&project)`；`model.rs:100-107` 用 `memory_context_fingerprint:{chat}` 去重。
- 工具表：`tool_registry.rs` 的 `registry!` 宏，每行 `(Builtin, wire 名, Access, parallel, image, description, JSON schema)`；`memory_search` 分发在 `tools.rs:100-114`；审批的 `automatic` / `reason` 计算在 `tools.rs:325-357`，global 记忆写入的理由是 `"Change persistent user-wide memory"` 且永不自动放行。
- KV：`db.get(key, fallback)` / `db.put(key, &value)`（`store.rs:90-110`）。测试里可参考 `remote_tests.rs:203` 用 `TcpListener::bind("127.0.0.1:0")` 起本地 HTTP fixture；`cloud.rs::service_url` 在 debug 构建下允许 `http://127.0.0.1`。

## 规格

### 1. 模块 `cloud_memory.rs`

- `impl Runtime`：
  - `fn cloud_memory_active(&self) -> Result<Option<(reqwest::Url, String /*token*/, String /*owner key*/)>>`：`cloud_config` 有效（`alive`）时返回 relay、解封令牌、以及 `sha256(relay + "\n" + email)` 作为缓存键后缀；否则 `None`。
  - 缓存键：`cloud_memory_cache:<后缀>`，值 `{"memories": [...], "fetched_at": <unix ms>}`。
  - `async fn refresh_cloud_memory(&self, force: bool) -> Result<()>`：不活跃直接返回；缓存不足 5 分钟且 `!force` 直接返回；否则 `GET v1/recall/status`，把 `memories` 中 `forgotten != true` 的写入缓存。失败时保留旧缓存并返回 Err（调用方决定是否忽略）。
  - `fn cloud_memory_guidance(&self) -> Result<Option<String>>`：不活跃返回 `None`。有缓存时返回一段：`Personal memory (cloud, {N} items, synced {RFC3339 时间}{", offline" 若 fetched_at 超过 30 分钟}):` 换行后每条 `- [{id 前 8 位}] {text 截 120 字符}`，按 `updated` 降序，总量 ≤ 4000 字节，超出停止并追加 `- … {剩余数} more; use memory_search`。缓存为空且从未拉取过：返回 `Personal memory (cloud): not loaded yet.`
  - `async fn cloud_remember(&self, text: &str) -> Result<Value>`：生成 uuid v4 作 id；`POST v1/recall/memory {id, text, base: null, forget: false}`。若请求失败（网络/超时/5xx），**用同一 id** `GET v1/recall/status` 回读：id 存在则视为成功；回读也失败返回 Err(502, "云端记忆暂不可用，未确认是否已保存")。成功后 `refresh_cloud_memory(true)`（失败忽略）。返回 `{"id", "saved": true}`。
  - `async fn cloud_forget(&self, id: &str) -> Result<Value>`：从缓存取该 id 的 `revision` 作 `base`（缓存无此 id → Err(404, "记忆不存在或已被删除")）；`POST {id, text: "", base, forget: true}`。409 → 强制刷新后返回 Err(409, "记忆已在别处修改，请重试")。其他失败同样回读确认（id 不在有效列表即视为已删除）。成功后强制刷新。返回 `{"id", "forgotten": true}`。
- `cloud.rs::logout_cloud` 成功清空 `cloud_config` 时，同时删除该账号的 `cloud_memory_cache:*` 键（按登出前的后缀计算并 `put` 为 `Value::Null`，不需要扫描）。

### 2. 工具

- `tool_registry.rs` 新增两行，`Access` 用新变体 `CloudMemory`（在 `Access` 枚举里加）：
  - `Remember, "remember", CloudMemory, false, false, "Save one stable personal fact or preference the user explicitly stated to cloud personal memory shared with the phone. Never store secrets, guesses, or transient questions. Use memory_write global for Mac-only working notes.", {"type":"object","properties":{"text":{"type":"string","maxLength":1500}},"required":["text"],"additionalProperties":false};`
  - `ForgetMemory, "forget_memory", CloudMemory, false, false, "Forget one cloud personal memory by exact id from memory_search results, only on an explicit user request.", {"type":"object","properties":{"id":{"type":"string"}},"required":["id"],"additionalProperties":false};`
- `tools::definitions(images)` 只在 `cloud_memory_active` 为 `Some` 时包含这两条（找现有过滤图片工具的位置照做；注意 `model.rs:117-123` 按名排序）。
- 分发（`tools.rs`）：`Builtin::Remember` → `cloud_remember`，`Builtin::ForgetMemory` → `cloud_forget`。
- 审批（`tools.rs:325-357`）：`Access::CloudMemory` 时 `automatic = false`，`reason = "Change persistent user-wide memory"`。审批卡的 `action_detail` 要能看到完整 `text`（remember）或被删记忆的原文与 id（forget，从缓存取），以及云端账号邮箱；沿用现有传 `action_detail`/`exact_target` 的机制，不新造字段。
- `memory_search`：现有行为不变；额外当 `cloud_memory_active` 时，把缓存中 `text` 包含所有查询词（NFKC + 小写、按空白分词）的记忆追加进结果，每条 `{"scope":"cloud","id","text","updated"}`，最多 30 条，放在文件结果之后。工具描述末尾追加一句 "Cloud personal memories are included with scope cloud when signed in."

### 3. guidance 与刷新

- `memory_guidance` 末尾追加 `cloud_memory_guidance` 的内容（`Some` 时）。现有两段的标题把 user-wide 改称 `Mac local notes` 并加一句：`Stable personal facts go to remember (cloud); Mac-only working notes go to memory_write global.`
- `model.rs::run_turn` 开始处（`memory_guidance` 之前）调用 `refresh_cloud_memory(false)`，错误忽略。不要阻塞超过 `cloud_http` 现有超时。

### 4. 测试（`cloud_memory_tests.rs`，`#[tokio::test]`）

用本地 `TcpListener` fixture 模拟 Worker，登录状态用直接 `db.put("cloud_config", ...)` 构造（token 用 `db.seal`），relay 为 fixture 的 `http://127.0.0.1:<port>/`：
1. `remember` 成功：fixture 收到 `{id, text, base: null, forget: false}`，返回记忆；之后 `cloud_memory_guidance` 含该 text，缓存键含后缀。
2. `remember` 首次 POST 返回 500，回读 status 含该 id → 返回 saved=true。
3. `forget` 携带缓存 revision 作 base，请求体 `text == ""`；fixture 返回 409 → Err 409 且缓存被强制刷新。
4. 未登录（`cloud_config` 为 Null）：`definitions` 不含两工具，`cloud_memory_guidance` 为 None。
5. `logout_cloud` 后缓存键为 Null。

验证命令：
```sh
cd native/potato-core && cargo test --lib cloud_memory_tests -- --nocapture 2>&1 | tail -20
cd native/potato-core && cargo test --lib 2>&1 | tail -5
```
第二条是全量单测回归，只需报告结果。
