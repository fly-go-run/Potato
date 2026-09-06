# Rust Potato 薄审批：源码研究、实现与验证

日期：2026-09-06。结论：把用户的注意力留给权限边界；已选项目里的日常工作连续执行。执行约束、是否询问、授权记忆是三个独立概念。模型负责理解任务和选工具，运行时负责兑现真实边界。

## 研究范围与来源

本次直接读取 Python Potato 当前工作区和六个参考仓库的可执行源码，不把 README 中的愿景当成已经实现的功能。Rust Potato 的修改基于已有未提交工作，期间同工作区也有工具注册表、并发和 steering 的增量变更；没有回滚或提交这些工作。

参考副本位于 `/tmp/potato-harness-reference.CL5beY`，提交号如下。这是固定版本审计，不宣称覆盖所有后续版本。

| 项目 | 提交 |
|---|---|
| Codex | `6af345407d9c2a568da9d01b6c4b81a9e61495c0` |
| PI | `9767ba275f3e9a5ee0f5c5342249b629ab1b2282` |
| DeepSeek Harness（deepseek-ai 官方仓库） | `d347e703908d0406b7a7ef80e3a0e594d86b2215` |
| Goose | `5e90925962f05acf8e255032de44d16c4a7768a2` |
| Rig | `d9ed455cf5d0c8f13207ab03c7843982a0f4898e` |
| VTCode | `ba2832c62fb293a79ffe2b5841cd751bd4826d85` |

## 各家的实际机制

| 项目 | 源码事实 | 对 Potato 的启发与取舍 |
|---|---|---|
| Python Potato | `governance/policy.py` 做工具注册检查、风险扫描、规则和执行等级决策；`resource_governor.py` 再处理写边界与沙箱升级；`tool_adapter.py` 将 ASK 接到自动评审或人工。当前 AUTO 允许低于 HIGH 且非 escalation/write_boundary/read_only 来源的 ASK 进入模型评审；未获批准时交互会话可回退人工，无人值守拒绝。 | 不照搬多层规则、扫描和每次代审。先消除确定性可放行操作的弹窗，保留统一门控。注意 Python AUTO 与本次 Rust AUTO 语义不同。 |
| Rust Potato（改前） | `tools.rs` 在普通文件读、列目录、搜索、写文件、web search、MCP、Computer Use 等路径都创建一次审批；`api.rs` 只接受 STRICT。文件模式默认只读，shell 只有 danger-full-access 才可申请。历史回忆、技能读取、记忆搜索和已有 job 管理等内部工具原本就不经过该审批卡。 | 首要收益来自普通项目文件操作免打扰。不能把“所有工具都审批”当成现状，因为内部工具已有豁免。 |
| Codex | `exec_policy.rs` 把审批策略与实际权限 profile 分开；受限沙箱中，普通未升级命令可直接运行。明确危险或缺失预期沙箱等情况进入 prompt/forbidden 分支。`Never` 对必须 prompt 的规则返回拒绝。当前代码也有 `guardian/review.rs` 自动评审，超时、解析失败、review session 失败阻止执行。 | 学习沙箱内连续执行、边界才询问、拒绝回给模型；不把命令字符串白名单当沙箱。Rust 当前无 OS 沙箱，不能直接复制其默认 shell 放行。 |
| PI | README 明确默认无权限弹窗；`AgentSession._installAgentToolHooks()` 仅在扩展注册了 `tool_call` handler 时调用 `beforeToolCall`，无 handler 直接返回。扩展异常会阻止执行。 | 最薄的方案是把安全交给容器或扩展。适合可信执行环境；桌面用户账户上的裸执行需要另外明确授权。 |
| DeepSeek Harness | `user-approval/src/index.ts` 的策略只有 ask/never；never 在 dispatch 前直接 rejected。闭集结果为 allowed-once/rejected/cancelled/unavailable；没有服务、非法结果或服务异常不能得到授权。`sandbox/src/escalation.ts` 处理严格更宽的逐次升级；sandbox-local 无可用 runner 时报 SANDBOX_UNAVAILABLE。 | 采用无人值守明确拒绝和一次升级。拒绝不是让运行时悄悄换成更强权限重跑。 |
| Goose | `permission_inspector.rs`：Auto 的权限检查基线允许；Approve/SmartApprove 先看用户工具权限。SmartApprove 可用 read_only_hint 放行，未知候选再交 LLM 判断；其它安全 inspector 还能覆盖权限基线。`ops_tool_approval.rs` 消费人工结果，AlwaysAllow/AlwaysDeny 更新工具级权限。 | 工具只读属性有用，但外部工具的自报 annotation 不是足够的信任依据。本次不凭 MCP 的 read-only hint 自动放行，也不把一次 shell 授权扩大为整个工具允许。 |
| Rig | `AgentRun` 是可序列化的 sans-IO 状态机；durable approval 示例将运行状态保存、恢复后再逐调用 approve/deny/edit/abort。示例对未知输入拒绝、EOF 终止，同时明确业务授权应在工具内落实。 | 是可复用的审批接入点，不是开箱即用的桌面权限策略。本次沿用 Potato 的工具执行、IPC 和持久化接口。 |
| VTCode | `permissions.rs` 按 Bash/Read/Edit/Write/WebFetch/Mcp 等构造请求；规则优先级为 deny > ask > auto > allow。全局 deny 是上限，全局 ask 仍约束 agent 覆盖配置；另有 protected_write_paths 和命令审批类型。 | 借鉴结构化能力和硬约束优先；不加入一套大 DSL、危险关键词正则或新的 agent 编排层。 |

可复核的固定源码链接：

- [Codex 命令策略](https://github.com/openai/codex/blob/6af345407d9c2a568da9d01b6c4b81a9e61495c0/codex-rs/core/src/exec_policy.rs)、[自动评审](https://github.com/openai/codex/blob/6af345407d9c2a568da9d01b6c4b81a9e61495c0/codex-rs/core/src/guardian/review.rs)。官方文档也明确：沙箱决定能访问什么，approval policy 决定何时询问；[Auto-review](https://learn.chatgpt.com/docs/sandboxing/auto-review) 替换的是需要审批时的 reviewer，普通已允许操作不进入评审。[Agent approvals & security](https://learn.chatgpt.com/docs/agent-approvals-security)
- [PI hook 实现](https://github.com/earendil-works/pi/blob/9767ba275f3e9a5ee0f5c5342249b629ab1b2282/packages/coding-agent/src/core/agent-session.ts)、[PI philosophy](https://github.com/earendil-works/pi/blob/9767ba275f3e9a5ee0f5c5342249b629ab1b2282/packages/coding-agent/README.md)。
- [DeepSeek Harness 审批服务](https://github.com/deepseek-ai/deepseek-harness/blob/d347e703908d0406b7a7ef80e3a0e594d86b2215/packages/interaction/user-approval/src/index.ts)、[升级机制](https://github.com/deepseek-ai/deepseek-harness/blob/d347e703908d0406b7a7ef80e3a0e594d86b2215/packages/sandbox/sandbox/src/escalation.ts)。
- [Goose permission inspector](https://github.com/aaif-goose/goose/blob/5e90925962f05acf8e255032de44d16c4a7768a2/crates/goose/src/permission/permission_inspector.rs)、[审批结果处理](https://github.com/aaif-goose/goose/blob/5e90925962f05acf8e255032de44d16c4a7768a2/crates/goose/src/agents/state_machine/ops_tool_approval.rs)。
- [Rig 持久化审批示例](https://github.com/0xPlaygrounds/rig/blob/d9ed455cf5d0c8f13207ab03c7843982a0f4898e/examples/agent_with_durable_approval/src/main.rs)。
- [VTCode 权限实现](https://github.com/vinhnx/VTCode/blob/ba2832c62fb293a79ffe2b5841cd751bd4826d85/crates/codegen/vtcode-core/src/permissions.rs)。

### 对旧报告的修正

`approval-mechanism-research.md` 是 2026-08-16 的历史快照，不能再作为当前缺陷清单。当前 Python `resource_governor.py` 已在受限模式且沙箱不可用时返回 DENY/SANDBOX_UNAVAILABLE，只有明确的 danger-full-access 分支允许裸执行。当前 AUTO 的非允许结果也已支持交互会话回退人工，HIGH finding 和明确权限升级不交给自动评审放行。这些改进并非本次 Rust 工作新增；本次没有改 Python 行为。

## 已实现的薄审批

新的 `src/approval.rs` 统一决定“直接继续 / 等待本次授权 / 无等待拒绝”，不增加 LLM 调用，不加入自然语言意图打分。

| 策略 | 普通项目读、搜索、已启用的编辑 | 越界、敏感路径、外部动作、裸 shell |
|---|---|---|
| AUTO（新配置默认） | 自动执行 | 人工一次批准；部分工具可显式记住完全相同参数 |
| STRICT | 经过审批门的工具逐次确认 | 逐次确认，不复用授权 |
| NEVER | 自动执行 | 立即给模型工具错误，不创建审批卡 |

文件模式独立：read-only 禁止项目写入；workspace-write 允许受约束的项目文件编辑；danger-full-access 允许提交裸 shell 审批，但不意味着关闭审批，也不扩大原有内置文件工具的写入根。新配置默认 AUTO + workspace-write；已有明确保存的 STRICT/read-only 保留。设置 → 工具权限可切换策略。未识别的策略（包括 Python 的 OFF/SMART）报错，避免悄悄变成全部允许。

自动范围包括项目普通 read/list/grep/glob、write/edit/append、专用 project memory_write，以及已经配置好的 web_search。普通项目编辑继续使用 cap-std、写前准备和冲突检测。用户自己的并发修改不会因为审批成功被覆盖。

敏感路径包括 `.git`、`.potato` 的非资料区域、`.agents`、`.codex`、常见凭证目录、`.env*`、pem/key 和 AGENTS/SOUL/PROFILE/SKILL 等持久指令文件。直接访问需确认；递归搜索跳过这些后代。`.potato/memory` 和 `.potato/artifacts` 是普通项目资料区域，普通文件工具与专用项目记忆工具一致；搜索可以穿过 `.potato` 容器发现资料，但跳过其它配置和资料内的凭证。全局记忆修改仍需确认。Git 元数据写入和项目外普通文件写入继续由已有硬边界拒绝。新增写路径父目录 symlink 检查，读路径按 canonical target 判断。

MCP、Computer Use、图片服务和新建定时任务仍走同一个审批入口。这些工具的副作用不能仅凭名称或自报 read-only annotation 推定。本次没有增加模型自批能力，也没有实现任意外部应用的细粒度 scope。

### shell：明确的一次升级

Rust 当前没有 OS 沙箱。受限文件模式下直接调用 shell 会得到明确的错误和可用的升级参数；主模型需要时提交：

```json
{
  "command": "cargo test --offline",
  "sandbox_permissions": "require_escalated",
  "justification": "验证刚完成的代码修改",
  "timeout": 60
}
```

审批展示真实命令、cwd、参数和为什么要跨边界。批准仅覆盖该调用，不改变全局或会话文件模式。运行前仍检查 cwd 没被重定向。没有“先失败，再由运行时全权限静默重跑”。常规 `cargo test` 等任意 shell 的首次执行仍可能询问，这是没有 OS 沙箱的明确限制。

### 授权记忆与撤销

默认按钮始终是“允许本次”。仅 AUTO 下的 shell/read/list/grep/glob 提供“当前会话记住相同参数（1 小时）”。记忆键包含工具名、规范化参数、项目 canonical root、文件模式；会话单独隔离。不按前缀、不按工具名泛化，不给 MCP、Computer Use、记忆写入或定时任务建立宽授权。

相同参数不代表脚本内容或外部数据没变：授权的是在该上下文再次运行同一命令，而不是冻结其全部依赖。切换 cwd、项目、参数、文件模式需要新的授权。授权仅在内存，最多 128 项，固定一小时到期，不滑动续期，重启清空。NEVER 和 STRICT 不使用这些缓存。

聊天下方显示有效临时授权数量，可直接清除。撤销使该会话旧审批失效，并用 generation 检查防止迟到结果重新建立授权。取消、过期、错误用户/会话、重复提交不产生新授权；future 被销毁也清理 pending。撤销不回滚已经开始的操作，不终止已批准的后台 job（可使用现有 job_kill）。

接口保持既有路径，新增能力：

- `PUT /api/workspace/running-config`：`approval_level: AUTO|STRICT|NEVER`，独立的 `sandbox_mode`。
- `POST /api/approval/approve`：缺省或 `scope: exact` 仅本次；`scope: session` 仅在卡片 `allow_session: true` 时接受。
- `POST /api/approval/revoke-session`：`session_id` 和 `user_id: default`。
- `GET /api/approval/list?session_id=...`：附加 `session_grants` 数量。
- `GET /api/approval/audit?session_id=...`：最近 256 条权限决策。只存时间、工具、策略、结果和固定原因，不重复保存命令、内容和凭证。它记录授权决策，不等于工具执行成功，也不覆盖门控前的全部参数/能力校验错误。

这些是本地 IPC runtime API，不是多租户身份认证服务。`request_context` 的权限设置由可信宿主提交，模型工具参数不能更改审批策略。

### 模型与后台运行

静态审批和记忆指导放在 system prompt，要求模型沿用已有用户授权，不要在工具审批之前再问一轮自然语言许可；先把工作做到可审阅，再请求必要权限。每轮 runtime context 只记录时间、项目、有效策略和变化后的记忆位置/索引预览；未变化的索引不重复追加。已有历史保持原样。拒绝作为工具结果返回，模型可继续其他工作。对重复拒绝不额外引入永久黑名单。

定时 agent 执行显式使用 NEVER，普通项目工作能继续，越界动作和 `request_user_input` 即刻返回错误。后台任务不会挂在五分钟审批卡或无限问答上；这并不自动批准创建新的定时任务。

## 三条工作流的一致性修复

按用户转来的 Claude 审查补查并修复：

1. 普通文件工具对 `.potato/memory` 的权限与专用项目记忆工具一致；新增测试实际 read/edit/grep/glob 笔记及 `.potato/artifacts`，同时确认配置与凭证不泄露。
2. `compaction.rs` 不再每个模型步骤追加 runtime notice。只有真实投影变化或预算压力跨过 trigger 阈值才记录；普通步骤不增长通知列表。已有有效通知维持原位置，随摘要覆盖清理。
3. 静态指导移到 system prompt；动态索引按会话指纹去重，发生变化才注入新预览。删除会话时一并清理指纹状态。
4. 默认策略明确选择 AUTO + workspace-write，保留已保存设置。

项目目录约定：`.potato/memory` 是笔记，`.potato/artifacts` 是明确写入项目的结果文件；其余 `.potato` 配置/控制数据不自动信任。本轮审批修改不迁移原始 shell 归档，保留 job_list/job_output 的会话检查。历史归档、项目索引和文件布局正在由同工作区另一个任务单独改造，其最终数据可见范围需按该任务的最终实现校验，不能把本轮结果当成尚未完成的存储改造验收。

新增 HTTP/SSE 回归运行连续 12 个工具步骤及后续两轮对话：普通步骤不追加通知；静态指导只在 system 出现；原索引和变化后的索引预览各出现一次。

## 验证与实际边界

`src/approval_tests.rs` 通过真实 `execute_tool` 与审批 API 验证，不使用在线模型或收费服务。连续九次项目操作的断言为 **0 次请求审批**；旧门控对这些操作会逐次创建卡片。这个计数是确定性 fixture 的结果，不是用户体验或模型任务成功率的线上 benchmark。

覆盖：AUTO 连续读写搜索、项目记忆；NEVER 对外部/敏感/交互动作即时拒绝；STRICT 每次询问；once 不记忆；session 精确参数复用与撤销；错误会话/用户/范围/过期/重复点击；取消清理；文件冲突；读只读模式不可写；项目外写和 Git 元数据边界；symlink 敏感目标；裸 shell 一次升级后文件模式保持不变；等待期间设置改变使审批失效。

全量回归命令：

```sh
cargo test --manifest-path native/potato-core/Cargo.toml --offline
cargo test --manifest-path native/potato-ui/Cargo.toml --offline
```

HTTP/SSE、MCP 和语音 fixture 需要允许本机回环监听。第一次受限环境运行中的 Operation not permitted 是测试环境限制；不能当成产品功能失败或跳过相关验证。随后在允许本地监听的环境执行全量回归。在线 replay 测试仍按原有约定 ignored。

本轮完整验证（2026-09-06）：核心运行时 106 项通过、1 项在线付费测试按默认跳过；桌面 UI 64 项通过。测试数量包含同工作区其它方向的已有/新增回归，不全部归因于本次审批修改。随后另一个任务开始改造历史存储；在其新的磁盘状态上，本次 9 组审批测试再次全部通过。后续存储改动不包含在上述全量通过结论中。

本次不声称拥有 OS 级隔离、不声称敏感路径列表能识别任意源文件中的秘密、不声称可以防御恶意并发文件系统替换的所有竞态。原有 cap-std 写入和 canonical 校验不等同于完整宿主沙箱。没有接入自动风险 reviewer，也没有对外发送生产任务来测审批准确率。

后续最有价值的方向是实际接入并验证平台 sandbox runner，使构建、测试等普通 shell 也能在真实边界内默认连续执行；之后再考虑可选 reviewer 接管少数必须问人的边界操作。把 reviewer 作为所有工具的前置模型，会增加延迟和另一套误判来源，当前不采用。

## Fable 5.1 high 独立审查补充

按用户要求调用 Claude Code CLI 做只读审查，约束为实际缺陷与最小修复，保留 AUTO + workspace-write 等既定产品选择。完整原始结果与主任务复核见 [审批独立审查报告](../../native/potato-core/APPROVAL_REVIEW_FABLE_5_1_2026-09-06.md)。

审查确认并修复：普通审批卡的 justification null 导致桌面反序列化失败；默认项目包含全局记忆时普通文件写入绕过审批；删除会话遗留审计与临时授权。前两项均通过新增测试先复现失败再验证修复。没有新增审批模型、规则 DSL 或额外确认层。

修复后在当前合流工作树全量验证：核心 140 通过、1 项在线测试默认忽略；UI 65 通过；审批专用测试 11 组通过，git diff --check 通过。这个结果更新了前文较早的验证快照，包含同期其它任务的存储与上下文改动。
