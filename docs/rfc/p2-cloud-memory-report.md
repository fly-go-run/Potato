# P2：Mac 挂载云记忆（Rust）交付记录

日期：2026-09-13。分支：`feat/process-track-codex`。

当前状态：Rust 实现及指定 5 项测试已完成；使用临时 manifest 补齐直接依赖后，专项和全量单测通过。仓库正式构建尚未完成：完整 NFKC 需要声明 `unicode-normalization = "0.1"`，但 Cargo.toml / Cargo.lock 不在本包授权修改范围，已请求范围确认，尚待回复。当前源码引用该依赖，原仓库 manifest 下会编译失败，不能据临时验证宣称正式交付完成。

## 文件与规格对应

| 文件 | 对应规格 / RFC v2 第 4 节 |
| --- | --- |
| `native/potato-core/src/cloud_memory.rs`（新增） | §1：有效会话、relay + 换行 + email 的 SHA-256 账号缓存键、5 分钟刷新、遗忘过滤、失败保留缓存；120 字符单条、4000 字节总量、更新时间降序、RFC3339 同步时间、30 分钟 offline 标记及未加载提示。保存 UUID v4、失败按原 id 回读、删除 revision CAS 与空 text、409 刷新及指定错误。§2：NFKC + 小写的全词匹配，最多 30 条云记忆。 |
| `native/potato-core/src/lib.rs` | 挂载云记忆模块与指定测试模块。 |
| `native/potato-core/src/cloud.rs` | §1：成功登出时按登出前账号后缀将缓存置 Null；将已有 alive / service_url / cloud_http 可见性改为 pub(crate)，供兄弟模块复用。 |
| `native/potato-core/src/tool_registry.rs` | §2：两条指定工具 schema / 描述，新增 CloudMemory access；memory_search 描述追加云端说明。 |
| `native/potato-core/src/tools.rs` | §2：按当前登录状态过滤工具定义、分发、automatic=false 及指定审批理由；沿用 action_detail 的参数序列化展示邮箱、完整 text 和删除 id，exact_target 标明账号及目标；文件结果之后追加云记忆。 |
| `native/potato-core/src/memory.rs` | §3：memory_guidance 本地标题改称 Mac local notes，添加指定用途说明，末尾追加云端 guidance。 |
| `native/potato-core/src/model.rs` | §3：memory_guidance 前等待 refresh_cloud_memory(false)，忽略刷新错误，复用现有 20 秒 HTTP 超时；改用带登录状态过滤的工具定义入口，保留原排序和 fingerprint 去重。 |
| `native/potato-core/src/cloud_memory_tests.rs`（新增） | §4：仅新增要求的 5 个 tokio 测试，本地 TcpListener 模拟 Worker，直接 seal token / put cloud_config。 |

缓存刷新沿用 cloud_generation 锁和账号快照，防止登出期间的旧请求回写缓存。审批后再次核对账号和缓存删除目标，防止等待期间目标变化。未新增审批卡字段；邮箱和删除原文加入现有 action_detail 所序列化的参数，实际 HTTP 请求仍只发送协议规定字段。

## 真实验证结果

按要求在 `native/potato-core` 运行：

```sh
cargo test --lib cloud_memory_tests -- --nocapture 2>&1 | tail -20
cargo test --lib 2>&1 | tail -5
```

为可靠获取退出码，实际使用 `set -o pipefail`，并在 tail 前用 tee 留存日志。两条正式命令均因缺少 `unicode_normalization` 直接依赖而编译失败（E0432 / E0599，退出码 101）。日志：`/tmp/potato-p2-targeted.log`、`/tmp/potato-p2-full.log`。

在 `/tmp/potato-p2-check-_51cu3hg/Cargo.toml` 创建临时 manifest，唯一功能性依赖差异为加入 `unicode-normalization = "0.1"`，lib path 指向本仓库当前 `src/lib.rs`，不修改仓库 Cargo 文件。使用 `--offline` 和原 target 目录验证：

```text
running 5 tests
cloud_memory_tests::signed_out_hides_cloud_tools_and_guidance ... ok
cloud_memory_tests::forget_409_uses_revision_and_empty_text_then_refreshes ... ok
cloud_memory_tests::remember_saves_and_populates_account_cache ... ok
cloud_memory_tests::remember_500_is_confirmed_using_original_id ... ok
cloud_memory_tests::logout_clears_account_memory_cache ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 188 filtered out; finished in 0.07s

test result: ok. 182 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 3.64s
```

临时验证第一次受执行沙箱限制，4 个 TCP fixture 无法绑定 127.0.0.1（Operation not permitted），1 个未登录测试通过；获准在沙箱外重跑后得到以上全部通过结果。全量回归也在沙箱外运行。临时日志：`/tmp/potato-p2-temporary-test.log`、`/tmp/potato-p2-temporary-full.log`。

## 范围解释、跳过项与待收尾

- 用户范围白名单未列 model.rs，但 §3 明确要求修改 run_turn；仅按这项明确要求修改该文件。报告文件也按用户交付要求新增。
- 仍需授权修改 `native/potato-core/Cargo.toml` 和 `native/potato-core/Cargo.lock`，补一项已有间接依赖的直接声明，然后在正式仓库重跑两条验证命令。未以手写近似规范化替代完整 NFKC。
- 回读成功但无法确认写入结果时，保留原 POST 错误；回读也失败才返回规定的未确认 502。没有排队或重发 POST。
- 刷新按本包 §3 的具体要求在 run_turn 内 await，沿用 cloud_http 20 秒超时；未另起后台任务。
- 不修改 Worker、GPUI、iOS、旧运行时。Worker 两个固定语义测试归 P1，本包未实施。
- 未做真实账号 Mac / iPhone 端到端验证或生产部署；fixture 验证不能代替跨设备验收。11 项既有 ignored 测试保持原状，未增加规格外测试。
- 未 commit、stash 或 revert。任务开始前的 remote.rs、remote_tests.rs、GPUI、iOS 等未提交改动未触碰。

## 收尾（Claude，2026-09-13）

- 已在 `native/potato-core/Cargo.toml` 补 `unicode-normalization = "0.1"`（Cargo.lock 仅新增一行直接依赖条目，版本沿用锁定的 0.1.25）。
- 正式仓库验证：`cargo test --lib --offline cloud_memory_tests` 5 passed；`cargo test --lib --offline` 182 passed / 0 failed / 11 ignored。
- 上面「仍需授权修改 Cargo」的待办已关闭。跨设备真实验收仍未做。
