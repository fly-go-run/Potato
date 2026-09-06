# Rust Potato 薄审批独立审查（Fable 5.1 high）

日期：2026-09-06。应用户要求，通过本机 Claude Code CLI 审查固定代码快照；用户明确授权把审批相关源码发送给 Claude 服务。

- 主审模型：`claude-fable-5-1`，命令行 `--effort high`。
- 只启用 Read / Glob / Grep；safe mode、无 MCP、无子代理，审查器没有编辑文件或执行测试。
- 审查限定实际缺陷、最小修复和有用回归；AUTO + workspace-write 等产品选择已明确接受，不以更严格架构为由挑刺。
- CLI 成功结束，耗时 8.3 分钟，报告费用 $6.2997（其中包含 CLI 内部 Haiku 辅助调用约 $0.0015；主审没有换模型）。
- 快照：`/var/folders/x5/jy_jf8gj0f3_4hwy68s7kn340000gn/T/potato-thin-approval-review-4alse3dx`；文件清单 SHA-256：`7a8668739237fb788ae3b3f446e13093bffff7b4eb2fd1defb59fa30a8ff3745`。
- 快照目录保存 review-prompt.txt、manifest.json、working-tree.diff 和原始 claude-stream.jsonl。下文 Claude 行号均指固定快照，未必等于并行修改后的工作树。

## 主任务核实与处置

Claude 的三个已证实问题在当时工作树仍然存在，现已做小范围修复。主任务结论：本次审批审查发现已处理，回归通过；这不是对未来并行改动的无限期保证，也不是 Claude 对修复后的第二轮审查。

1. **审批卡解析失败：已复现并修复。** 先加入 UI 反序列化回归，旧代码遇到显式 null 确实失败。核心现在总是输出字符串；桌面字段使用 Option<String>，兼容缺失、null 和正常字符串。核心真实审批卡也断言 justification 为字符串。
2. **普通文件工具绕过全局记忆审批：已复现并修复。** 新增真实 execute_tool 测试，旧代码在 NEVER 下成功写入 memory/MEMORY.md。分类现在按全局记忆根识别写入，不因项目包含该目录而自动放行。回归覆盖默认 workspace 项目与包含 workspace 的父项目、相对/绝对路径、write/edit/append：NEVER 拒绝且文件不变，AUTO 创建卡片，批准后才能写入。
3. **删除会话残留审计：已修复。** 按 session_id 清理审计，API 删除成功后也撤销该会话的临时授权，避免重建同名会话复用旧授权。新增测试先取得真实 session grant，再删除会话，验证审计为空、授权数量归零。

对“未证实疑虑”的处置：没有因其新增摘要恢复协议或权限框架。每轮历史归档位置已经由存储任务单独维护，不能把它和记忆索引预览去重混为一谈；全局记忆位置能否在任意摘要后保持可发现，仍未专项验证。设置文案疑虑不改变当前每次 shell 均须审批的行为，不作为此次阻断项。审查已核对的归档保护、静态指导、通知条件与项目笔记发现机制无需再改。

验证由主任务执行，Claude 审查器没有执行测试：

- 修复前：新增全局记忆门控测试与 UI null 卡片解析测试均失败，原因与审查结论一致。
- 修复后：`cargo test --manifest-path native/potato-core/Cargo.toml --offline`：140 通过、1 项在线模型测试按默认忽略（85 lib + 16 harness + 4 memory + 30 runtime + 5 transcripts）。
- `cargo test --manifest-path native/potato-ui/Cargo.toml --offline`：65 通过。
- 其中审批专用测试 11 组全部通过；`git diff --check` 通过。
- 第一次 UI 全量运行仅回环服务监听被受限沙箱拒绝，随后在允许本地监听的环境完整重跑通过。没有跳过失败用例，也没有调用在线模型进行功能测试。

共享工作树还有历史存储、上下文和桌面任务的并行修改。上述全量数量包含那些修改，不能全部算作本次审批成果。

## Claude 原始审查结果（修复前快照）

审查完成。未执行任何测试，以下结论全部来自对快照源码的静态阅读。

## 代码已证实的问题

**1. 高：桌面端无法渲染绝大多数审批卡，除升级 shell 外的所有需确认操作都会超时失败（本次新引入）**
- 位置：`native/potato-core/src/approval.rs:231` 把 `"justification": args["justification"]` 写进卡片。除带 `require_escalated` 的 shell 外，所有工具的参数里没有这个字段，序列化后是 `null`。桌面端 `native/potato-ui/src/interactions.rs:24-25` 把它声明为 `#[serde(default)] pub justification: String`。serde 的 `default` 只处理字段缺失，遇到显式 `null` 会报 invalid type。`backend.rs:127-128` 对整个数组反序列化失败后返回“审批格式错误”，`interactions.rs:153` 只写入 poll_error，卡片列表保持为空。
- 触发：AUTO 下读取项目外文件、读 `.env`、MCP、Computer Use、全局记忆写入、定时任务、图片工具，或 STRICT 下任意普通操作。
- 后果：用户看不到“允许/拒绝”按钮，只看到刷新失败提示。工具在 `approval.rs:246` 等满 300 秒后以 expired 拒绝。行为是 fail-closed，不存在越权，但薄审批的主路径在桌面端不可用。旧代码（working-tree.diff 第 3232-3236 行）没有输出 `justification` 字段，所以这是本次回归。
- 最小修复：`approval.rs:231` 改为输出字符串，例如 `string(args, "justification")`；同时把 UI 字段改成 `Option<String>` 以容忍 null。二选一即可，建议两处都改。
- 回归测试：在 `interactions.rs` 测试模块中用核心端真实字段集（含 `"justification": null`、`"driver": null`）调用 `serde_json::from_value::<Approval>` 并断言成功；在 `approval_tests.rs` 的 STRICT 用例里补一句 `assert!(a["justification"].is_string())`。

**2. 中：默认项目（或任何包含运行时 workspace 的项目）下，普通文件工具可免确认改写用户全局记忆，与 RFC“全局记忆修改仍需确认”矛盾**
- 位置：`native/potato-core/src/tools.rs:320-334` 的自动分类只看“在项目内且非敏感路径”。未选择项目时 `projects.rs:123-132` 返回 workspace 本身作为项目，而全局记忆根就是 `workspace/memory`（`memory.rs:147-149`）。`memory/MEMORY.md` 相对路径既在项目内又不命中 `sensitive()`，于是 `write_file`、`edit_file`、`append_file` 都自动放行。同一文件走 `memory_write` 全局作用域却会弹卡（`tools.rs:334, 343-344`）。以家目录作为项目时同样成立，因为 workspace 在其之下。
- 后果：模型在 AUTO 和 NEVER（包括后台定时任务）下无需确认即可改写全局 `MEMORY.md`，而 `model.rs:90-96` 会把该索引前 2000 字符注入之后每一个会话的 runtime_context。这是跨会话的持久化通道。
- 最小修复：在 `tools.rs:327` 之后计算 `let global_memory = resolved.starts_with(self.memory_root()?);`，把 `!(access == Access::WritePath && global_memory)` 加入 `automatic` 条件，并在该情况下把 reason 设为 “Change persistent user-wide memory”。
- 回归测试：在 `approval_tests.rs` 新增用例，body 不带 `potato.coding_project_dir`，AUTO 下 `pending_tool(write_file, {"file_path":"memory/MEMORY.md",...})` 必须产生审批卡；NEVER 下返回 403；再用 `dir.path()` 作为项目重复一次。

**3. 低：会话删除后审批审计记录残留**
- 位置：`approval.rs:138` 以 `approval_audit:{session}` 存储，`store.rs:624-631` 的 clear_chat_state 前缀列表不包含它。RFC 承诺审计不含命令和内容，所以影响很小。
- 修复：`delete_chat` 中按 spec 的 session_id 删除该键。测试：删除会话后 `GET /api/approval/audit?session_id=...` 返回空数组。

## 已核对、未发现问题的项

- 静态指导只在 system：`model.rs:83-89`，`tests/harness.rs:1279-1284` 断言了 system 含有且后续消息不含。
- 记忆预览去重：`model.rs:90-98` 按会话指纹比较，`store.rs:630` 在删除会话时清理指纹。
- 上下文通知：`compaction.rs:127` 仅在投影变化或压力阈值跨越时追加。
- NEVER 定时任务：`scheduler.rs:350-353` 显式覆盖，模型创建的 spec 不含 request_context。
- 撤销与取消：`approval.rs:156-164` 递增 epoch 并丢弃待决卡，`approval.rs:255-272` 在迟到回复上校验取消、策略变化和 epoch；`steering.rs:38` 与 `lib.rs:353-355` 在 steering 和回合结束时清理待决卡。
- 授权记忆：键含工具、规范化参数、项目根、文件模式，按会话过滤，仅 AUTO 且限定五个工具（`approval.rs:201-224`）。
- `.potato/memory` 可发现性与凭证隔离：`approval.rs:61-64`、`file_search.rs:78-86` 及对应测试成立。
- 运行时归档写保护：`tools.rs:29-45` 覆盖 write/edit/append/memory_write 和 shell cwd，`store.rs:76, 259` 确认 transcript 与 jobs 都在 `workspace/history` 下。

## 未证实的疑虑

- 承载记忆位置的用户消息可能被摘要覆盖（`context.rs:161` 跳过 covered 之前的消息），之后每轮只写“unchanged”（`model.rs:96`），全局记忆路径是否还留在上下文取决于摘要内容。
- `settings_view.rs:811` 文案称文件访问设置只影响内置文件工具，而 `tools.rs:199` 在 danger-full-access 下允许 shell 省略升级参数。行为上仍然逐次审批，只是措辞不准确。

## 结论

**ready after small fixes。** 问题 1 是一行修复但会让桌面端整个审批交互失效，问题 2 是几行分类修正。修复后建议按 RFC 中的两条命令重跑核心与 UI 回归。
