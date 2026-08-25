# Potato 审批机制调研与改进方案

日期：2026-08-16
来源：Potato 源码审计（`src/potato/governance/*`、`security/tool_guard/*`、`app/approvals/*`）、
Codex 本地源码（`~/testProjects/codex` @ `2f5b01ab`，2026-03）、DeepSeek Harness 官方文档、
Claude Code / Cursor / Gemini CLI / Copilot CLI / Amp / OpenCode 公开文档，以及两篇 2026 年评测论文。

---

## 1. Potato 现状（代码事实）

### 1.1 四档与语义

| 枚举 | UI 文案 | 实际行为 |
|---|---|---|
| `AUTO`（默认） | 自动 | 每个 ASK 交给**第二个模型** `review_tool_call` 代批，12s 超时，**任何失败都 DENY**，永不弹人工卡 |
| `SMART` | 重要 | INFO/LOW 放行，MEDIUM+ 弹人工卡 |
| `STRICT` | 每次 | 所有工具都弹卡（ALLOW 规则也改成 ASK） |
| `OFF` | 从不 | 跳过深扫，规则仍生效；语义是"不问就跑"，不是"该问的一律拒" |

`execution_level.py:37` 的 AUTO 文档字符串还写着"only guarded_tools are checked"，与实际（模型代批）不符。
默认值三处不一致：agent profile 默认 `AUTO`，`policy.yaml` 默认 `smart`，`from_config(None)` 返回 `AUTO`。

### 1.2 决策管线（`policy.py:704-840`）

```
Phase 0  ToolRegistry 类型：unknown→DENY，internal→ALLOW
Phase 1  深扫（sensitive_path / pattern / shell_evasion）：CRITICAL→DENY；扫描异常→空 findings（fail-open）
Phase 1.5 shell 危险关键字正则（rm -rf /、fork bomb、dd、mkfs）→DENY
Phase 2  builtin_rules → user_rules 首匹配即返回（findings 只挂载不参与判断）
Phase 3  shell 无规则命中：有 MEDIUM+ finding→ASK，否则 SANDBOX_FALLBACK；其它工具按档位阈值
```

ASK 之后（`tool_adapter.py:560-`）：
- AUTO → `review_tool_call`（模型代批）→ ALLOW / DENY
- 其它 → 先调一次 LLM 做 `generalize_target_for_approval`（≤6s）→ 弹卡 → `add_approved_rule` 写 `policy.yaml`，`duration` 固定 `session`

沙箱：Linux bwrap/Landlock、macOS Seatbelt、Windows 受限令牌；`network_allow=["*"]`；沙箱不可用时 `SANDBOX_FALLBACK` **静默降级为无沙箱执行**（`resource_governor.py:265-285`）。
沙箱违规在执行后被捕获 → 弹卡 → 用户批准则**去掉 sandbox_config 全权限重跑**（`tool_adapter.py:530-538`）。

MCP / Driver 走另一套 `DriverPolicy`（`drivers/handler.py:150-190`），AUTO 在那边 = 普通人工卡，两套子系统对 AUTO 的定义不一致。

shell 工具**没有**任何让模型申请升级的参数（无 `sandbox_permissions` / `justification`）。

### 1.3 具体缺陷清单

| # | 问题 | 位置 |
|---|---|---|
| 1 | user_rules ALLOW（含默认 `Bash(gh *)` 与所有 session 批准规则）短路 Phase 1 的 HIGH finding | `policy.py:791-809` |
| 2 | 批一次 finding 驱动的 ASK 就写一条 ALLOW 规则，同 session 内同类检测被静默 | `resource_governor.py:501,536-548` |
| 3 | 批准只记 session，却持久化到 `policy.yaml`，无 TTL / 上限，session_id 为空时永远不复用 | `resource_governor.py:521`、`policy.py:182-184,1057` |
| 4 | 每个非 builtin ASK 弹卡前多一次 LLM 泛化调用（≤6s），用户点"仅本次"时白等 | `tool_adapter.py:669` |
| 5 | 沙箱违规批准 → 记 ALLOW 规则 → 下次 ALLOW 又带沙箱 → 再违规再问，死循环 | `resource_governor.py:290-295` |
| 6 | 沙箱不可用时 SANDBOX_FALLBACK 静默变裸跑，无提示 | `resource_governor.py:265-285` |
| 7 | AUTO 是最容易在无人值守下**硬拒**的档（无模型/超时/评审说 allow 但 risk=high 都拒），文案却叫"自动" | `auto_review.py:572-701` |
| 8 | 扫描器 fail-open、评审器 fail-closed，两半失败姿态相反 | `policy.py:875`、`auto_review.py:14` |
| 9 | `governor.policy.execution_level` 按请求写共享对象，多会话并发会互相踩 | `tool_adapter.py:336` |
| 10 | 审批 API 只校验 `root_session_id`，`user_id` 不校验 | `routers/approval.py:94-104` |
| 11 | 评审模型只看打码 JSON + 一条 user_intent，比主模型更瞎，且多一次往返 | `auto_review.py` |
| 12 | 无测试覆盖 #1、#5、跨会话持久化 | tests/unit/governance |

---

## 2. Codex（2026-03 源码）怎么做

> 注意：`assess_command_safety` 已不存在；命令安全在 `core/src/exec_policy.rs` + `core/src/tools/sandboxing.rs`，安全清单在独立 `shell-command` crate。

**两个正交旋钮 + 三个预设**
- `AskForApproval`：`untrusted` / `on-failure`(已弃用) / **`on-request`(默认)** / `never` / `Reject{sandbox_approval, rules, mcp_elicitations}`（细粒度自动拒）
- `SandboxPolicy`：`DangerFullAccess` / `ReadOnly{access}` / `WorkspaceWrite{writable_roots, network_access…}` / **`ExternalSandbox`**（已在容器里，跳过嵌套沙箱但不假装 full-access）
- 预设：`read-only`、`auto`（on-request + workspace-write，UI 叫 "Default"）、`full-access`（**never** + danger-full-access）

**核心原则**（`exec_policy.rs:537` 注释）：在受限沙箱里，对非升级、非危险命令**不弹窗，让沙箱兜底**；只在沙箱不是护栏时才问人。

**决策**：`bash -lc` 按管道拆段逐段评估；`is_known_safe_command`（~25 个只读命令，含参数级审计：`find -delete`、`rg --pre`、`git -C` 全局参数跳过）→ 危险清单只有 `rm -rf`/`sudo` → 其它交沙箱。

**模型举手**：shell 工具参数 `sandbox_permissions: use_default | require_escalated | with_additional_permissions`（**沙箱内追加路径，中间档，prompt 明确说优先于全升级**）+ `justification` + `prefix_rule`。默认 `on-request` 下沙箱拒了**不自动重试**，把真实 stderr 回给模型，模型决定要不要 `require_escalated`。

**记忆**：`ReviewDecision` 六种（Approved / ApprovedExecpolicyAmendment / ApprovedForSession / NetworkPolicyAmendment / Denied / Abort）。记前缀有三道闸：`BANNED_PREFIX_SUGGESTIONS`（python3、bash -lc、node -e、sudo、bare git 等 ~40 个）；heredoc/复杂解析禁止记忆；`prefix_rule_would_approve_all_commands` 克隆策略加规则重验每段，验不过不提供"下次不问"。session 缓存 key 经 `canonicalize_command_for_approval` 归一化。

**保护路径**：writable root 内仍只读 `.git`（含 worktree 指向的 gitdir）、`.codex`、`.agents` —— 理由：写这些等于给自己提权。

**平台缺失时 fail-closed**：Windows 无沙箱 → workspace-write 静默降 read-only；in-workspace patch 无沙箱 → AskUser。

**per-variant prompt 片段**：`prompts/permissions/{approval_policy,sandbox_mode}/*.md`，只注入当前生效档位的文本。

## 3. DeepSeek Harness

- 两旋钮：`SandboxMode` read-only(默认)/workspace-write/danger-full-access（仅管文件系统）；`ApprovalPolicy` **只有 `ask` / `never`**，`never` = 所有 ask 确定性 `rejected`。
- 预设：`workspace-write`(+ask)、`danger-full-access`(+never)。
- 结果集封闭：`allowed-once` / `rejected` / `cancelled` / `unavailable`，无审批服务 → `unavailable` → fail-closed。**核心无持久 always-allow**，交插件（oh-my-dsh: approval-vocabulary / persist / grace / diff）。
- 沙箱 `confine()` 拿不到就抛 `SANDBOX_UNAVAILABLE`，无静默裸跑。
- bash 工具描述教模型：拒绝是策略结果，同轮一次 `sandbox_permissions` 更宽 + 一句 `justification` 重试 → 弹审批；升级重试是新调用、不改 provider 状态。
- 社区自动评审插件（dsh-tiered-approval / dsh-auto-approval）：静态规则 → LLM → 人；不确定即拒；作者自认"full access 下 LLM 是最后防线"。

## 4. 业界其它要点

- **Claude Code**（2026-08-14 起 Pro/Max 默认 `auto`）：allow/ask/deny 规则（deny>ask>allow）+ PreToolUse hook + 沙箱 `autoAllowBashIfSandboxed`；违规明细追加到命令输出让模型看到；重试用 `dangerouslyDisableSandbox:true` 走常规审批。auto 分类器**不看 tool 结果与模型思考**（防注入/自我合理化），两阶段（单 token 快筛 + CoT），拒了继续干、连续 3 次/累计 20 次回退问人，"Recently denied → retry"。官方数字：FPR 0.4%，**真实过激行为 FNR 17%**。第三方评测（arXiv 2604.04978）：模糊 DevOps 任务端到端 FNR 81%，36.8% 危险动作因走 Edit 绕过分类器。
- **Cursor**：拒绝了 denylist（Backslash 证明 `"e"cho`、base64、子 shell 轻松绕过），Auto-review = allowlist → 沙箱 → 分类器（可让 agent"换个方式"）；明说"不是安全边界"。
- **Amp**：`delegate` 把 JSON 参数交给外部程序，exit 0/1/2 = allow/ask/reject，stderr 回给模型 —— 极简 hook 契约。
- **OpenCode**：`doom_loop`（重复同一调用）默认 ask；`.env` 默认 deny。
- **arXiv 2606.28679**："能力门控 ≠ 授权"，模型判断应放在确定性门控**之后**，不能替代。

---

## 5. 对 Potato 的建议（按收益/成本排序）

### P0-a —— 先立不变量：权限只能被明确授权逐级放大（GPT 交叉审阅补充，已核实）

0. **非 shell 文件写入没有工作区硬边界。** `Write/Edit(WORKSPACE_DIR/**)` 是 user_rules ALLOW（`policy.py:407-416`），工作区**外**路径无规则命中 → Phase 3 非 shell、无 findings → SMART/AUTO/OFF 直接 ALLOW（`policy.py:955-972`）；`write_file` 随后直接写宿主文件系统（`file_io.py:302`），不进沙箱。工作区**内** `.git/hooks/pre-commit`、`policy.yaml`、skills 目录也是 ALLOW。这是最高优先级：默认只允许写 workspace / coding project，其余写入 ASK；writable 内保留只读子路径（`.git`、`.potato`、治理配置、凭证、持久化指令）。
0'. 沙箱内 `network_allow=["*"]`（`resource_governor.py:418-426`）——文件隔离有效但网络外传未阻断，至少要能配置成 allowlist。
0''. `auto_review.parse_review_response` 兼容裸 `APPROVE`（`auto_review.py:220-224`），此时 risk/authorization 都是 unknown 却放行；应要求完整结构化字段，输出改三态 `allow / deny / require_human`。

### P0-b —— 改 AUTO 的定义（不动大架构）

1. **AUTO 不再走 `review_tool_call`。** ASK 中"无红线、沙箱能兜住"的 shell 走 `SANDBOX_FALLBACK`；沙箱违规**回给主模型**（把 violation 明细追加到命令输出），不再自动弹审批/代批。
2. **给 shell / 写文件加升级参数**：`sandbox_permissions: use_default | require_escalated`（后续可加 `with_additional_permissions{read,write}` 中间档）+ `justification`。带参数才进人工卡；卡上显示模型自己写的理由。无参数的违规只报"被沙箱挡了"。
3. **沙箱不可用时不再静默裸跑**：SANDBOX_FALLBACK → 弹卡（或按档位拒），并在 UI 上明示"本机沙箱不可用"。
4. 评审模型降为 SMART 的**可选实验项**；若保留，照 Anthropic 做法：只在"否则要问人"时触发、不喂 tool 结果、拒了继续、连续 3 次/累计 20 次回退人工、每个裁决入审计。

### P1 —— 修记忆链与规则优先级

5. Phase 2 user_rules ALLOW 命中时，若 findings 有 HIGH，仍进 fallback（或 ASK）；只有 builtin ALLOW 可短路。
6. finding 驱动的 ASK 被批 → **只 `allowed-once`**（绑 call id），不写规则；人批的"同类"才写规则。
7. 记规则加三道闸（抄 Codex）：解释器/shell 前缀黑名单（python3、bash -lc、node -e、sudo、bare git…）；heredoc/多行禁记；克隆策略重验"加了这条真的能免问"。
8. 规则区分 `session` 与 `always`，`always` 写项目级文件（如 `<workspace>/.potato/permissions.json`）可查可删；`session` 不落盘。清掉 #5 死循环：沙箱违规的批准记成"escalated" 类规则，命中时以无沙箱执行。
9. 弹卡前的 LLM 泛化改为**异步/延迟**：先出卡，"批准同类"按钮上等泛化结果再点亮；用户点"仅本次"不等待。

### P2 —— 产品语义

10. 两旋钮拆开：**沙箱档**（只读 / 工作区可写 / 全开）× **审批策略**（问 / 自动拒 / 全放行）；四档改为预设，UI 显示 "custom" 派生态。
11. 「从不」二选一并写清：无人值守要的是 DSH `never`（该问的一律拒，任务不挂）；本地图省事要的是 Full Access。现在的 OFF 更像后者，文案改"完全访问"，另加一个"无人值守/自动拒"给 cron 与渠道机器人。
12. Driver/MCP 与内建工具的 AUTO 定义对齐（同一条 ASK 处理路径）。
13. 保护路径：writable 内仍只读 `.git/hooks`、`.git` 指向的 gitdir、Potato 自身配置目录（`policy.yaml`、skills）。
14. 提供 PreToolUse hook（Amp 式：JSON in，exit code out，stderr → 模型），让高级用户自己写策略。

### P3 —— 测试补齐

- Phase 2 ALLOW vs HIGH finding 优先级；沙箱违规批准后不再循环；`allowed-once` 不落盘；`always` 跨会话可复用；沙箱不可用时不裸跑。

---

## 5.1 三方交叉审阅（Claude / GPT / Grok）一致点与分歧

- 一致：沙箱兜底 + 主模型带理由举手 + 人批是主路径；模型评审只做可选加速；`ALLOW_UNSANDBOXED` 静默裸跑与沙箱违规批准后全权限重跑必须改；两旋钮拆开。
- GPT 补充且已核实为真：文件写入无工作区边界（上面 P0-a.0）；HIGH 可被 user ALLOW 覆盖；深扫异常 fail-open；裸 `APPROVE` 放行。
- GPT 的一处事实错误：它读的是 `tylerbuilds/deepseek-harness`（第三方，session approval 只按工具名缓存），**官方是 `deepseek-ai/deepseek-harness`**（125k stars，`ask`/`never` + `allowed-once`），本报告 §3 依据官方文档。
- 排序调整：把"文件写入边界 + 保护路径"提到 P0 之首，其余不变。

## 6. 一句话

Codex / DSH / Claude Code 三家的共识是：**确定性规则 → 沙箱兜底 → 主模型带理由举手 → 人批**；模型评审只能作为"否则要问人"那一格的可选加速，永远不是安全边界。Potato 现在的 AUTO 把可选项当主路径、把沙箱当可选项，方向反了；先把这两件事掉过来，收益最大、改动最小。
