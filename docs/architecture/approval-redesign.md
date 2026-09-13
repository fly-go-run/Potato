# 原生权限与自动审批改造建议

日期：2026-09-08。状态：第一、二阶段已实现并完成代码审查；第三阶段仍为后续工作。验证使用独立临时数据目录，未修改用户日常客户端的授权或配置。

范围遵循 [native-only.md](native-only.md)：只面向 potato-core 与 potato-gpui。

## 结论

将文件访问范围、审批触发策略、审批执行者和授权有效期分开。普通目录操作优先匹配用户保存的权限规则；仍需审批的操作可交给独立模型调用；明确拒绝、需要补充授权和服务故障分别处理。

不要仅把目前 `authorize()` 中的人工等待替换成一次大模型调用：那会继续逐文件审批、额外消耗 token，并保留原来的授权粒度和界面问题。

## Codex 源码依据

分析固定在官方仓库提交 [`74d3a5bf1046f004ee33a200ee497dc7593a5687`](https://github.com/openai/codex/commit/74d3a5bf1046f004ee33a200ee497dc7593a5687)，提交时间 2026-09-08 11:25:15 UTC。不是对所有已发布 CLI 版本的概括。

| 关注点 | 实际实现与可借鉴之处 | 源码 |
| --- | --- | --- |
| 是否需要审批 | 工具编排先取得 `ExecApprovalRequirement`，结合执行环境、文件权限和审批策略选择执行路径；审批与实际执行边界分离 | [tools/orchestrator.rs](https://github.com/openai/codex/blob/74d3a5bf1046f004ee33a200ee497dc7593a5687/codex-rs/core/src/tools/orchestrator.rs#L121) |
| 谁来审批 | `request_approval()` 先处理 permission hooks，再路由到 Guardian 或用户；当前提交还通过 decision extension 处理决定和同步审批调用 | [tools/approvals.rs](https://github.com/openai/codex/blob/74d3a5bf1046f004ee33a200ee497dc7593a5687/codex-rs/core/src/tools/approvals.rs#L467)、[guardian/decision.rs](https://github.com/openai/codex/blob/74d3a5bf1046f004ee33a200ee497dc7593a5687/codex-rs/core/src/guardian/decision.rs#L51) |
| 模型审批的开关 | 常规路由要求审批策略为 `OnRequest` 或 `Granular`，且 reviewer 为 `AutoReview`；`Never` 不是自动通过 | [guardian/review.rs](https://github.com/openai/codex/blob/74d3a5bf1046f004ee33a200ee497dc7593a5687/codex-rs/core/src/guardian/review.rs#L213) |
| 审批者隔离 | 独立配置、只读权限、禁止递归申请审批；去掉技能、记忆、MCP、应用、hooks 等与审批无关的能力 | [guardian/reviewer_config.rs](https://github.com/openai/codex/blob/74d3a5bf1046f004ee33a200ee497dc7593a5687/codex-rs/core/src/guardian/reviewer_config.rs#L31) |
| 授权证据 | 紧凑的会话证据与精确的待执行动作分开构造；有原始用户授权、上下文版本及增量游标 | [guardian/prompt.rs](https://github.com/openai/codex/blob/74d3a5bf1046f004ee33a200ee497dc7593a5687/codex-rs/core/src/guardian/prompt.rs#L76) |
| 结构化结果 | `outcome` 为 allow/deny；支持 risk_level、user_authorization、rationale。当前 parser 允许低风险的简短 allow 结果，不强制四字段齐全 | [guardian/assessment.rs](https://github.com/openai/codex/blob/74d3a5bf1046f004ee33a200ee497dc7593a5687/codex-rs/core/src/guardian/assessment.rs#L13) |
| 风险判断 | 明确规定不能仅因路径在工作区外就判高风险；按实际副作用和授权语义判断，不要求授权文字与命令语法完全相同 | [policy.md](https://github.com/openai/codex/blob/74d3a5bf1046f004ee33a200ee497dc7593a5687/codex-rs/core/assets/guardian/policy.md#L61)、[policy_template.md](https://github.com/openai/codex/blob/74d3a5bf1046f004ee33a200ee497dc7593a5687/codex-rs/core/assets/guardian/policy_template.md#L14) |
| 会话授权与永久规则 | 会话缓存只保存 `ApprovedForSession`；持久命令授权有独立的规则追加机制，使用 token 前缀，不是对整条 shell 文本做 starts_with | [tools/sandboxing.rs](https://github.com/openai/codex/blob/74d3a5bf1046f004ee33a200ee497dc7593a5687/codex-rs/core/src/tools/sandboxing.rs#L70)、[execpolicy/amend.rs](https://github.com/openai/codex/blob/74d3a5bf1046f004ee33a200ee497dc7593a5687/codex-rs/execpolicy/src/amend.rs#L65)、[execpolicy README](https://github.com/openai/codex/blob/74d3a5bf1046f004ee33a200ee497dc7593a5687/codex-rs/execpolicy/README.md) |
| 权限范围申请 | `request_permissions` 对额外权限路径做规范化，通过专用协议返回权限及有效范围，区别于批准一次命令 | [request_permissions handler](https://github.com/openai/codex/blob/74d3a5bf1046f004ee33a200ee497dc7593a5687/codex-rs/core/src/tools/handlers/request_permissions.rs#L88)、[协议](https://github.com/openai/codex/blob/74d3a5bf1046f004ee33a200ee497dc7593a5687/codex-rs/protocol/src/request_permissions.rs) |
| 失败与成本 | 区分拒绝、超时、取消和解析/连接错误；仅部分暂态失败有限重试；拒绝有熔断。审批上下文可以复用，但有版本、锁和取消管理 | [guardian/review.rs](https://github.com/openai/codex/blob/74d3a5bf1046f004ee33a200ee497dc7593a5687/codex-rs/core/src/guardian/review.rs#L1057)、[guardian/review_session.rs](https://github.com/openai/codex/blob/74d3a5bf1046f004ee33a200ee497dc7593a5687/codex-rs/core/src/guardian/review_session.rs#L141) |

不照搬 Codex 的全部 hooks、扩展体系、审查会话分叉或内部模型名。先实现同样清晰的边界与调用契约。

## 改造前已确认的问题

1. `potato-gpui/src/view.rs` 的“本机 · 自动”只由 `sandbox_mode` 推导，没有读取 `approval_level`，更没有 reviewer 设置。显示的“自动”并不代表模型自动审批。
2. `potato-core/src/tools.rs` 把项目外的 `ReadPath` 判为不能自动执行。同一目录的不同文件都到 `authorize()`。
3. `approval.rs` 的会话 grant 以整个 `{tool,args,project,mode}` 为键，过期时间一小时。同一文件只增加 `start_line` 都不复用；现有 `session_grant_is_exact_revocable_and_never_overrides_never` 测试明确验证了这一行为。
4. `potato-gpui/src/interactions.rs` 只显示“拒绝”“允许本次”，提交固定 `scope=exact`，没有接后端的 session scope，也没有持久规则。
5. 权限值不一致：GPUI 提交 `full-access`，后端只接受 `danger-full-access`。应统一类型与序列化，不能继续各处手写字符串。
6. 审计只记录时间、工具、策略、结果和原因，缺少目标、授权来源、规则 ID、审批模型、耗时及成本。
7. 原生 shell 没有 OS 沙箱，文件写入另有 `PreparedWrite` 的目录约束。批准一个只读目录，既不等于 shell 被限制在该目录，也不能直接使项目外写入合法。

## 建议的运行流程

```text
工具请求
  → 规范化动作与目标，生成不可变的待执行快照
  → 检查硬性边界、显式禁止规则
  → 检查基础文件权限、用户持久授权、会话授权
  → 已允许：直接执行；明确禁止：返回原因
  → 仍需审批：按 reviewer 交给用户或独立模型
  → 复查权限版本、目标和取消状态
  → 执行原来的那一个动作，并记录决定来源
```

“硬性边界”保留当前核心私有数据库、密钥与运行时状态的保护，以及执行器实际能实施的限制。目录 grant 只能扩充明确支持的文件能力；不能靠审批提示词替代执行器限制。

### 1. 用户配置的目录授权

首期提供四个不同选择：允许本次、本会话允许读取此目录、始终允许读取此目录、拒绝。目录可以调整，明确显示是否含子目录；默认建议当前文件的父目录，不能自动扩大到桌面或用户主目录。

建议在现有 SQLite 配置存储中新增带版本的 permission rules，不另建一份可能与数据库冲突的配置真相。设置页面管理规则，后续再提供 JSON 导入导出。

建议规则字段：

```json
{
  "id": "rule-id",
  "kind": "directory",
  "path": "/Users/liuxu/Desktop/wfastcache-liuxu-patch",
  "recursive": true,
  "operations": ["read", "list", "search"],
  "decision": "allow",
  "lifetime": "persistent",
  "created_by": "user"
}
```

- 读取授权以目录和操作类型匹配，不包含起止行号、搜索词等不扩权的参数。文件工具别名先归一化。
- canonical path 按路径组件匹配；处理符号链接、目录替换和审批等待期间目标变化，不用文本前缀判断。
- 目录规则不隐式包含写入、执行命令、网络访问或原有受保护数据。首次只交付读取/列目录/搜索授权。
- 同一任务并行申请同一范围时合并待审批项，执行时重新检查规则。撤销递增权限版本，使尚未执行的旧决定失效。
- 只有用户操作可以写入永久规则。模型批准一次操作不能偷偷变成永久目录授权。
- 明确禁止优先于允许。会话授权不跨会话；永久规则可查看、删除，重启后继续有效。

### 2. 独立模型审批

新增 `reviewer=user|model`，与文件权限模式分开。设置中显示审批所用 provider/model；默认可复用当前 provider/model，但使用独立请求、固定政策和独立缓存标识，不复用主助手 system prompt、skills 或工具执行循环。

首版复用 `model.rs` 的连接与协议适配层，在其上增加专门的 review adapter。给模型的输入是：用户任务与明确授权、相关的有限历史、当前精确动作、已有权限范围、已有拒绝，以及本次为什么需要审批。工具输出和模型生成的理由属于证据，不能自行建立用户授权。

首版不开放工具。若普通文件判断缺少必要事实，可由宿主提供有限元数据；模型不能自行启动 shell、请求审批或改配置。之后如增加只读探查，必须走独立的受限接口。

建议 Potato 采用自己的严格返回类型：

```text
ReviewAssessment {
  outcome: allow | deny | ask_user,
  risk: low | medium | high | critical,
  rationale: string,
  authorization_evidence_ids: [...]
}
```

`ask_user` 是 Potato 的产品设计选择，不是声称 Codex 同步模型的 JSON schema 有该值。代码还应区分 `ReviewFailure`：超时、断流、无效结构、取消。

通过：执行本次动作；需要授权：展示人能看懂的卡片；明确拒绝：把具体原因反馈给主助手，避免重复同一申请；服务故障：说明故障并提供人工处理，不冒充风险拒绝，更不自动通过。人工新授权可以触发一次带新证据的复审；硬性禁止仍有效。

模型每次只决定当前动作。目录级的连续放行主要靠用户规则解决；首版不将模型的一次 allow 泛化成目录或命令前缀许可。

模型成本控制：有规则的动作不调用模型；审查输入有大小上限；同一不可变请求合并；请求有总截止时间、取消和有限暂态重试。不要把整个主会话反复发送，也不要缓存一个工具名的 allow 供所有参数复用。上下文增量复用可作为后续优化。

### 3. 文件范围与审批方式的 UI

输入框菜单展示两个独立设置：

- 文件访问：只读 / 工作区读写 / 完全访问，以及额外授权目录数量。
- 审批方式：手动审批 / 模型自动审批。

审批卡优先展示“读取文件”“为什么需要确认”“将允许的目录与操作”，原始工具 JSON 收进详情。“正在自动审批”是过程状态；通过后在过程记录中可展开查看理由，不弹人工卡。

旧 `AUTO` 迁移为当前规则行为与人工 reviewer，不能悄悄开启额外模型请求；用户选择模型自动审批后才启用。`STRICT`、`NEVER` 的语义保留并映射到新的内部策略类型，避免把 NEVER 错当成全部允许。现有配置和 UI 文案必须读取同一类型。

### 4. shell 单独处理

目录只读规则不匹配 `execute_shell_command`。其 `cwd` 位于授权目录，不能证明命令只访问该目录。

首版 shell 仍以完整命令、cwd、权限和环境快照独立审批；模型收到真实的“无 OS 沙箱”事实。当前无法实施的约束不能显示为已实施。

后续命令规则可借鉴 Codex 的 argv 前缀，但需要可靠地处理 shell 包装、管道、重定向、命令替换、脚本内容和实际可执行文件，不能用 `command.starts_with("python")` 等字符串白名单。完整 OS 沙箱属于单独阶段。

## 实施拆分

| 阶段 | 主要改动 | 验收结果 |
| --- | --- | --- |
| 1：配置与目录规则 | 核心增加 typed action / permission rule / decision 类型；`authorize()` 接规则匹配；API 增加授权 CRUD；GPUI 接目录授权和设置；统一模式值 | 对截图目录只授权一次，连续读多个文件和不同起止行均不再弹卡；重启和撤销有效 |
| 2：模型 reviewer | 独立 review adapter、证据构建、决策路由、取消/故障/审计；GPUI 接审批状态和 reviewer 设置 | 无规则的合理只读请求可被模型批准，用户能看见依据；模型故障不会执行工具，也不会伪装成拒绝 |
| 3：命令与成本优化 | 执行边界、命令规则、审批上下文复用、拒绝熔断与统计 | shell 规则不因 cwd 或简单前缀误放行；长期使用无重复审批风暴 |

`tools.rs` 负责构建动作和执行前复查，`approval.rs` 负责协调；将规则与 reviewer 拆到独立模块。`api.rs` 管理配置与授权；`interactions.rs` 管审批卡；`settings.rs` / `view.rs` 共用权限类型与文案。

首期可以只提供只读持久目录授权，但第二期的 reviewer 接口应在第一期保留，避免再重写流程。项目外写入要连同 `PreparedWrite` 的授权根目录选择一起设计，不只修改审批判断。

## 必须验证的行为

- 同目录多文件、同文件不同行号、不同搜索词、工具路径别名都复用正确的只读 grant。
- 相似前缀目录、符号链接逃逸、审批后目录变化、受保护路径不误放行。
- 会话不串用、重启保留永久规则、撤销与并行请求不会使过期授权复活。
- 目录 read grant 不能授权 write/shell；READ_ONLY、STRICT、NEVER 保持各自明确语义。
- 用假模型覆盖 allow/deny/ask_user、格式错误、超时、取消、政策更新、提示注入及失败重试；不依赖真实模型账单做核心回归。
- 模型审查中取消或用户补充指令后，旧的批准结果不能继续执行。
- GPUI 验证目录授权、规则删除、模型审查状态、失败文案，以及 `danger-full-access` 的配置往返。
- 审计记录动作 ID、目标、规则/人工/模型来源、权限版本、理由、耗时、模型和 token 用量，避免保存密钥或不必要的完整文件内容。

## 第一阶段交付与验证

已实现 `permissions.rs` 中的配置类型、目录规则、会话范围与路径快照；`approval.rs` 接入规则复用、权限版本和撤销处理。规则通过 `/api/permissions/rules` 管理，持久规则保存在现有 SQLite 配置中，会话规则只存在当前运行时。

GPUI 审批卡提供拒绝、允许本次、本会话允许目录、始终允许目录，支持调整目录；设置 → 安全可添加、编辑和撤销目录规则。文件范围与审批策略独立显示，统一 `danger-full-access` 序列化并兼容旧值。第一阶段预留的 `reviewer=model` 已在第二阶段接入实际调用。

代码审查补充了以下边界：非递归授权不能执行递归搜索；从祖先目录搜索不能绕过子目录拒绝；目录或目标身份变化使旧审批失效；权限变化或用户补充指令后移除过期审批；仅在按规则确认下合并同目录等待，每次确认仍逐动作审批。搜索范围包含明确拒绝子树时，首版保守拒绝整个搜索，不自动裁剪搜索结果。

验证记录：

- potato-core 完整测试 180 通过、1 忽略；最终审批协调补丁后重跑审批相关测试 21 通过。
- GPUI 完整测试在单线程运行下曾达到 62 通过。早期运行出现 GPUI 测试调度线程断言，基线与当前版本随后均通过单线程复查。并行的动画任务随后新增 `process.rs` 测试，最新全量复跑被该测试的 `#[test]` 宏递归错误阻断；已通知对应任务修复，不能将此前 62 通过视为当前共享工作树的最终全绿结果。
- 独立 GPUI 调试客户端实机验证审批卡、展开详情、目录规则创建与编辑入口、重启持久化和撤销；四个审批按钮在 1000 × 760 窗口完整显示。审批卡使用可视化 fixture，实际审批与规则复用由核心测试验证。
- 调试构建通过。未发布安装包、未替换用户正在使用的客户端，也未点击用户截图中的实际待审批请求。

第一阶段的目录权限仅覆盖文件读取、列目录和搜索，不扩大写入、shell、网络或受保护数据的访问范围。

## 第二阶段：已交付的模型自动审批

输入框权限菜单可直接选择「模型自动审批」。设置 → 安全 → 审批方式可选择手动或模型审批，审批模型默认跟随当前对话模型，也可指定已配置服务商和模型 ID。保存模型自动审批时切换到 `AUTO`（按规则确认）；`STRICT` 仍每次人工确认，`NEVER` 仍阻止需要审批的动作。旧配置保持人工 reviewer，不静默增加模型调用。

实现路径：

- `reviewer.rs` 使用独立固定政策、原始用户消息和当前精确动作调用 `model.complete()`，兼容 Chat Completions 与 Responses；不给工具，不启动主助手循环，不加载主助手 system、skills、memory 或附件展开后的 wire 内容。
- `approval.rs` 在硬边界、拒绝规则、已有授权之后路由待审动作；模型只批准本次操作，不创建会话或永久目录规则。`ask_user` 与故障转现有人工卡，明确拒绝返回工具错误。
- 审批结果使用严格 JSON，包含 outcome、risk、rationale、authorization_evidence_ids。无效字段、伪造证据 ID、违规工具调用和 critical allow 不会执行动作。原始用户消息中引用的第三方材料也不能自动成为用户授权。
- 45 秒总截止、最多一次暂态重试、2048 输出 token、256 KiB 响应上限；原始用户证据超过 32 KB / 64 条，或总输入超过 48 KB 时转人工，不删掉旧限制后继续批准。
- 返回结果后复查取消、用户证据、配置选择、权限版本和目标身份；过期结果记为取消。相同证据与精确动作复用拒绝结果，第三次重复拒绝停止本轮主助手，并补齐已声明工具的结果。
- `/api/approval/list` 返回 active_reviews 和按时间正序排列的最近 32 条模型决定；完整审计沿用每会话最多 256 条的持久记录。GPUI 显示「正在自动审批」、批准/拒绝/故障、理由、模型和耗时；模型 token 用量保存在审计中。

验证：核心完整测试 **195 通过、1 忽略**，包含 13 个新增 reviewer 回归；核心 Clippy `--lib -D warnings` 通过。GPUI 完整测试 **65 通过**（单线程）。使用本机假模型服务进行了原生 UI 端到端验收：主助手提出外部文件读取 → 独立 reviewer（空工具列表）批准 → 实际文件读取 → 主助手返回结果，全程无人工卡；也观察到审批进行状态、明确拒绝和故障人工回退。模型审计包含用量并支持重启读取。

假模型验证的是调用、权限与状态流程，不代表对真实模型风险判断准确率的评测。shell 仍没有 OS 沙箱，审批模型会收到这一事实；当前审批不会改变执行器的文件边界。后续可加入受限只读探查、上下文增量复用与更完整的 shell 执行边界。

可复现验收工具与使用步骤见 [原生自动审批验收](../../native/potato-gpui/design/auto-approval/README.md)。

## 第三阶段研究：上下文复用与快速决定

进一步查阅本机官方 Codex 仓库的 `6af345407d9c2a568da9d01b6c4b81a9e61495c0`（2026-09-06）后，确认 Guardian V2 的快速决定不等于“保存某次 allow，下次同名工具直接通过”。[approval.rs 的 cached_evidence](https://github.com/openai/codex/blob/6af345407d9c2a568da9d01b6c4b81a9e61495c0/codex-rs/ext/guardian-v2/src/async_scorer/approval.rs) 在 adaptive 模式下检查异步风险评分、工具调用滞后量、最近评分失败、授权上下文完整性和授权版本；条件不满足就回到同步审查。风险评分可以跨动作类别复用，因此不能把它描述成精确动作结果缓存。此路径受功能和策略配置控制，也不能声称所有 CLI 默认行为都相同。

[ScoreAuthorization](https://github.com/openai/codex/blob/6af345407d9c2a568da9d01b6c4b81a9e61495c0/codex-rs/ext/guardian-v2/src/async_scorer/authorization.rs) 绑定任务设置、执行环境、本任务及根任务的授权版本、模型信息。同步审查记录会作为后续评分的参考，但 [review_evidence.rs](https://github.com/openai/codex/blob/6af345407d9c2a568da9d01b6c4b81a9e61495c0/codex-rs/ext/guardian-v2/src/async_scorer/review_evidence.rs) 明确说明：旧决定仅针对当时的动作，理由不构成新授权。

另一个独立机制是 [复用隔离的 reviewer 会话](https://github.com/openai/codex/blob/74d3a5bf1046f004ee33a200ee497dc7593a5687/codex-rs/core/src/guardian/review.rs#L915)：空闲时向已有审查会话追加请求，并发时从最后提交的主干分叉，配置变化时重建。这有助于服务商利用稳定前缀缓存，但仍会发起模型推理；不能与直接复用决定、完全跳过模型调用混为一谈。

Potato 本阶段采用明确的产品边界：只在当前运行中的任务内，短期复用低风险原生文件读取的精确批准；使用文件状态、用户证据、权限版本和模型连接等条件检查失效。shell、写入、网络及不同读取动作继续独立审查。不会根据一个文件的批准自动生成目录、命令前缀或永久规则。审查对话单独保留有限历史，并将工具结果、助手文字和旧审批理由明确标为非授权参考；用户消息和真实交互回答才是授权证据。具体交付契约与验证结果如下。


## 第三阶段交付（2026-09-09）

- 正向缓存仅覆盖当前运行时、当前任务、精确参数的 low-risk `read_file`，普通文件最多 1 MiB；600 秒固定有效期，命中不续期，全局最多 128 项。宿主核对 SHA-256、文件元数据、路径身份、全部授权证据、权限版本、审批连接和会话 generation。文件指纹准备超过 250 毫秒则不缓存；不向模型提供指纹读取的文件内容。
- 用户授权仍完整保留在边界内，32 KB / 64 条；卡片回答单独从已提交记录读取，附提问背景、选择和时间顺序边界，跳过不算授权，整体结构化授权预算 34 KB。最近助手/工具参考最多 8 项，共 6 KB，截断明确标注。审批对话最多 6 轮、总输入 48 KB、64 个任务，基础授权/配置变化时重建，并发分叉通过版本比较提交。
- `DELETE /api/approval/review-cache` 撤销当前任务的决定缓存和审查对话，执行器沿会话 generation 检查在途结果失效，不影响其他任务。审计和用户目录规则保留；重启清空内存缓存。
- 根据后续用户要求，新安装默认 `reviewer=model`，明确保存的 `reviewer=user` 保留。未配置模型时仍可修改普通运行设置；实际审批无法完成会回退人工。输入框仅显示审批方式，快捷菜单移除只读项。审批历史与清除入口移至右侧栏，聊天区仅保留需要用户处理的交互卡。
- 实机假服务闭环：三个连续读取产生两次模型审批，中间命中正向缓存，最后一次请求复用固定授权前缀并带上一轮评估。确认聊天区无审批历史，右侧栏可展开查看。文件/用户指令/权限/连接/过期/撤销失效、并发分叉及问答来源由回归验证。文件检查是执行前核对，不声称已实现 OS 沙箱或原子文件描述符隔离。
- 本轮最终 core 全量 212 通过、1 忽略；GPUI 65 通过；core library Clippy 通过。既有 all-targets Clippy 还有非本次改动的测试代码告警，未据此声称全目标清零。

## 搜索耗时修正（2026-09-09）

用户实际会话的前两次搜索并行（38.566 秒、26.749 秒），后两次串行（66.025 秒、57.474 秒），并非同 query 网络重试。默认 hosted 搜索让当前模型通过 Responses API 完成内部搜索、阅读和报告生成，额外工作累积了延迟；旧解析也会把 commentary 过渡语混进结果。

`search.rs` 改为检索摘要指令、1600 输出 token、低推理和 60 秒网络上限，排除 commentary，保留并去重引用/source 链接，标注预算截断的部分结果；缺少搜索/引用证据的普通生成不冒充搜索。未更换用户服务商或模型。DeepSeek 的 `max_tool_calls` 不构成硬限制，真实限制依赖输出和请求预算，不能承诺一定只检索一次。

同一公开查询对照：旧请求 51.445 秒，8 次内部 search 与 11 次 open_page；新请求两次观测 3.324 秒、4.057 秒，各 1 次 search，带来源与摘要。这是本机有限样本，不保证固定速度。四项搜索回归覆盖请求预算、来源提取、commentary 排除、部分输出和无搜索证据拒绝。
