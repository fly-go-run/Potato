先只读调研：浅克隆 Codex 源码，并对照你们现有的 TurnFlow 状态机。克隆完成。接下来分头读 Codex 的 TUI 渲染，以及你们这边的 TurnFlow 状态机。仓库结构已摸清。接着在 Codex 的 tui crate 里定位 history cell、status line 和折叠逻辑。状态头和推理摘要已经对上了。接下来读 streaming、exec cell 和折叠提交逻辑。# Codex 过程展示状态编排调研

范围：`/tmp/codex-research` 浅克隆（`openai/codex`）。主证据在 TUI（`codex-rs/tui`：`status_indicator_widget.rs`、`chatwidget/streaming.rs`、`command_lifecycle.rs`、`exec_cell/`、`history_cell/{messages,separators}.rs`）。`codex exec` 只是 `eprintln` 事件，无折叠状态机。下列是开源 TUI，**不是** ChatGPT 网页/手机 Codex；用户口述「收口后只留答案」在 TUI 源码里不成立。

---

## 1. Codex 状态与转换（实证）

舞台是两层：**composer 上方一条活 status**（spinner + shimmer 头 + 耗时 + Esc），**transcript 里 append-only 的 HistoryCell**（至多一个 `active_cell` 原地改）。思考 **不** 流进主列。

| 阶段 | 行为 | 来源 |
|---|---|---|
| 空闲 | 无 status、无 active_cell | — |
| 任务开始 | 头固定 `"Working"`，shimmer 2s 扫一遍，32ms 帧，耗时从 0s | `on_task_started` |
| 思考中 | delta 只进 `reasoning_buffer`；`extract_first_bold(**…**)` 才换 shimmer 头；未闭合 `**` 保持旧头 | `on_agent_reasoning_delta` |
| 阶段句 | **两路，都是模型给的，不是模板。** ① reasoning 首个粗体 → status 头（「Exploring codebase」源码未见，即此）。② `MessagePhase::Commentary` → 当正文流式写入 transcript | `streaming.rs` |
| 另有硬编码 | 只读/列目录/搜索合成一组：活着 `"Exploring"`，完成 `"Explored"` + `Read a, b` / `Search q in p`。与 status 头是两套词 | `exec_cell/render.rs` |
| 工具运行 | 占 `active_cell`。Exploring 组不展示 stdout，可 `add_call` 合并。Shell：`"Running cmd"` + 输出尾最多 5 行（流式 `append_output`）。MCP `"Calling"`，搜索 `"Searching the web"` | `command_lifecycle` / `tool_lifecycle` |
| 工具完成 | **无延迟。** Shell/MCP/搜索 `should_flush()` 立刻 commit 成 `"Ran"` / `"Called"` / `"Searched…"`（仍截 5 行）。Exploring **不立刻 flush**，改成 Explored，等下一段非 exploring 内容冲走 | `should_flush` |
| 回答流式 | `flush_active_cell` + **藏 status**；未完成行在 `StreamingAgentTailCell`。已提交工具行保持紧凑，不收回 | `handle_streaming_delta` |
| Commentary 结束 | 队列空且 turn 仍在跑 → **恢复 status** | `pending_status_indicator_restore` |
| FinalAnswer / 轮次结束 | status 不恢复；flush；有过工具才插 `FinalMessageSeparator`。`"Worked for Xs"` **仅 elapsed > 60s**，否则一条横线。**已提交工具 cell 不藏、不并进一头** | `separators.rs`：`filter(\|s\| *s > 60)` |
| 手动展开 | 主视图无 per-cell 点击。`Ctrl+T` overlay 才给完整 stdout / `transcript_only` 推理。源码未见「点开后阻止自动折」——因为主列本来就不藏工具 | `pager_overlay` |

思考收口：`**Title**` 单独一段 → 主列 `• Title`（dim italic）；`**Head**\n\nbody` 主列只留 body（头已消耗在 status）；无粗体的长推理 `transcript_only`，主列空白。收到助手正文时藏的是 status，不是把思考「收起来」。无 600ms settle、无收口折叠动画。

---

## 2. 对照（Codex / 我们 / 差在哪）

我们：`TurnFlow` 头 + fold-row；`summarizeTrack` 在 streaming 间隙归 `thinking`；活跃工具组 `autoRaw`；被接替段 summary；live 结束 600ms `settling`；头 shimmer；时长优先 `trackDurationLabel`。

| 状态 | Codex | 我们 | 差 |
|---|---|---|---|
| 空闲→思考 | status=`Working`，思考不占主列 | pending `TurnFlow` +「思考中」模板，空 reasoning 也占 fold-row | 活信息在头 vs 在轨 |
| 阶段句 | 模型 `**bold**` 换头；Commentary 当叙述留下 | 头=「思考中 / 正在{工具}」；无 phase | **最大感知差** |
| 工具中 | 类型分流：exploring 永不展开输出；shell 最多 5 行 | 活跃 pair **自动 raw 整卡** | 我们更「散装」 |
| 工具完成 | 立即变紧凑一行/一组，无 timer | 被接替才收；失败恒可见 | 我们有延迟收口，Codex 无 |
| 回答中 | 藏 status，工具行不动 | pulsing 停，fold-row 仍在，叙述恒可见 | 叙述同构已对齐 |
| 收口 | 卸 status；工具行永久留着；>60s 才 Worked for | 600ms 后 raw→summary；头默认仍开；任意时长都报 | 用户要的「只留答案」=关头；TUI **不做** |
| 手开 | Ctrl+T 全文 | 点头关整轨；点行 raw/summary；`rowByKey` 记住 | 我们更细，也更重 |

---

## 3. 改造建议（按感知收益）

1. **头吃模型粗体阶段句。** 把 in-flight reasoning 的首个 `**…**` 写成头（无则回落「思考中」）。落点：`MessageList.TurnFlow` 头文案；可从 `reasoning` 消息抽，不改 SSE。这是「Exploring codebase」的真来源。
2. **取消活跃工具自动 raw。** 学类型分流：read/grep/search 只留摘要；shell 最多 5 行预览。落点：`TurnFlow` 的 `autoRawKey`；`FoldRowView` / `ToolCard`。用户说的「有的折有的开」是这个，不是时间态。
3. **思考不再占 fold-row 窗口。** 活着只驱动头；收口 title-only 落一行 dim `• …`，长文进点开。落点：`stepGroups.materializeRun` + `ReasoningBlock`。
4. **收口不要自动 `header=collapsed`。** TUI 卸的是 status，不是工具列表。默认 `summary`（已做）对齐开源实现；「只留答案」留给用户点头。落点：不要改 `manualHeader` 默认。
5. **Worked for 加 60s 门槛，放到答案后而不是头上抢戏。** 落点：`TurnFlow` 头 + 可选答案后分隔。短回合不报时长。
6. **阶段叙述靠提示词，不发明 phase。** Commentary 在我们这边已经是恒可见 narration。落点：后端提示词（既有 PR3），前端不动。
7. **600ms settle 可留。** TUI 无此延迟；Web 需要过渡，不是差距。

**不要学的：** 把收口理解成「工具从主列消失」。那是闭源客户端观感或用户记忆，TUI 源码明确相反。骨架（append-only、叙述恒可见、同族合并）已经对齐；该学的是 **活信息在头、工具永远紧凑、阶段词来自模型**。