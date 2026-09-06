# Computer Use 修复审查 R1

## 结论

- P0：无。
- P1：3 条，应在合入前修复。
- P2：无。

## P0（必修）

没有发现 P0 问题。

## P1（应修）

### 1. hold 在策略目标提取阶段无条件生效，可被非审批/未执行调用反复续期

- 位置：`src/potato/computer_use/protect.py:72-84`；相关执行分支：`src/potato/agents/tools/computer_use.py:669-684`
- 问题：`policy_target_for_computer()` 同时承担“提取治理目标”和“延长 observation”两个职责。目标提取发生在 `governor.assert_policy()` 之前，因此只要 observation 当前有效，无论后续决策是 `ALLOW`、`DENY` 还是 `ASK`，都会先把它延长到当前时间之后 330 秒；hold 也不会在拒绝、参数错误或未执行时撤销。
- 为什么是真问题（触发场景）：把某应用设为 always-allowed（或已有该 Computer 工具/Bundle 的 allow 规则），在原始 120 秒 TTL 内调用 `ComputerClick(observation_id=..., x=1)`。这个输入能通过函数参数/JSON Schema 校验，但因为缺少 `y` 且没有 `element_index`，`computer_click()` 会在进入 `_run_action()`、执行 `take()` 之前返回 `TARGET_REQUIRED`。治理目标提取已经把 observation 延长；在新期限前重复同类无效调用即可无限续期，最后再用数分钟前的元素或坐标执行真实动作且无需新的 observation。用户快速拒绝一次审批后，hold 同样残留，并可被其它已允许的 Computer 动作复用。这破坏了“只允许审批等待跨过 TTL”的边界，也让 120 秒的新鲜度约束失效。
- 建议最小修法：让 `policy_target_for_computer()` 恢复为纯读取；仅在治理结果确实进入 Computer `ASK`/自动审查等待路径时建立 hold。拒绝、超时、取消以及获准后实际函数未 `take()` observation 的路径必须清理该 hold（最保守可直接 `drop()` 该 observation）；成功动作仍由现有 `take()` 一次性消费。这样无需改变 `exact_target` 或规则匹配逻辑。

### 2. hold 的截止时间早于 AUTO 回退后的完整人工审批窗口

- 位置：`src/potato/computer_use/protect.py:84-94`；`src/potato/governance/tool_adapter.py:662-675,864-903,938-942`
- 问题：当前 hold 的截止时间锚定在最初的目标提取时刻，长度固定为“人工审批超时 + 30 秒”。但 AUTO 模式会先执行模型审查，只有审查未放行后才创建 pending 并开始 300 秒人工计时。`AutoReviewConfig.timeout_seconds` 合法范围最高为 120 秒，超过 30 秒的部分没有被 hold 覆盖。
- 为什么是真问题（触发场景）：AUTO 审查配置为 120 秒且模型超时/要求人工确认时，observation 在目标提取后 330 秒失效，而人工审批卡从约第 120 秒才开始其完整的 300 秒有效期。用户在卡片出现 210 秒后、但仍处于服务端允许的审批窗口内批准，`_run_action()` 会得到 `STALE_OBSERVATION`。也就是说，本次修复在受支持的配置下仍有约 90 秒的“批准成功但动作必失败”窗口。
- 建议最小修法：在 `create_pending()` 完成后、调用 `wait_for_approval()` 前，对该 Computer observation 再按 `pending.timeout_seconds + slack` 刷新一次 hold，使截止时间与真实人工等待起点对齐。若按上一条把 hold 移出目标提取，则 AUTO 审查开始时还需先覆盖其自身的有界超时，并在转人工时再次刷新。

### 3. Click 同时携带元素和坐标时，审批卡展示的目标与实际点击目标相反

- 位置：`src/potato/computer_use/protect.py:114-138`；实际执行优先级：`src/potato/agents/tools/computer_use.py:669-690`
- 问题：`describe_computer_action()` 只要看到 `element_index` 就优先展示该元素，坐标分支是后续的 `elif`；但 `computer_click()` 只要 `x`、`y` 同时存在就优先执行坐标点击，并完全忽略 `element_index`。
- 为什么是真问题（触发场景）：调用参数同时给出 `element_index=3, x=900, y=700` 时，如果元素 3 是“Cancel”按钮，审批卡会显示 `click · button "Cancel"`，实际却点击 `(900, 700)`；该坐标可以落在完全不同的敏感控件上。参数 Schema 允许三者同时出现，因此这不是不可达输入。新增的审批摘要会给用户错误的安全判断依据。
- 建议最小修法：让描述逻辑严格复用 `computer_click()` 的目标选择优先级：对 `ComputerClick` 先判断完整的 `x`/`y`，存在时展示坐标；否则才解析 `element_index`。也可以在入口拒绝同时提供两类目标，但不能继续让展示和执行各选一个目标。

## 其余核查结果

- `ObservationStore.take()` 的一次性消费仍由同一把锁保护；本次字段新增没有破坏该原子性。
- 截图裁剪的“超过 24 小时或不在最新 100 张内即删除”条件与需求一致。
- 已核对 OpenAI Chat、Anthropic、Gemini、DashScope、OpenAI Responses 五种 formatter 的 part 形状；`_text_part()` 重构及缺失文件占位符未发现确定性 provider 回归。
- 后端三个审批序列化入口都会经 `approval_display_fields()` 透传 `action_detail`；前端字段为可选并有条件渲染，旧响应缺字段不会崩溃。中英文词条均存在。

## 验证记录

- 指定 Computer Use / formatter / approval scope 测试：91 passed。
- 额外治理、AUTO review、Computer policy、OpenAI Responses/provider wiring 测试：91 passed。
- `tsc -p tsconfig.app.json --noEmit`：通过。
- `vitest run src/lib/approvals.test.ts`：2 passed。
- 另做了两个最小复现：确认单次目标提取会把 observation 延长到原 TTL 之外；确认同时传元素和坐标时，描述选择元素而执行选择坐标。
