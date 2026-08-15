# Agent Activity Protocol 调研（GPT 稿，用户提供，2026-08-15）

> 来源:用户用 ChatGPT 对 Potato 两条分支源码 + Codex App Server/TUI +
> Claude Agent SDK 做的四层核对。原文照录,供 Claude/grok 交叉评审。
> Claude 已核实其代码事实断言(见 activity-protocol-cross-review.md)。

## 一、总体判断

视觉层已完成约七成(append-only 时间线、工具配对、三层密度、产物卡、
失败提升、时长、8 行)。瓶颈集中在**运行状态语义、事件协议、折叠状态
所有权**。前端目前通过 StreamMessage/turnTimeline/stepGroups 后验归类;
成熟 Agent 产品需要前端直接消费 Run → Stage → ActivityItem → Lifecycle
Event。继续在 TurnFlow 上叠启发式,遇到并行/审批/重试/子 Agent/回放会
迅速失控。视觉组件保留,数据层与状态机升级。

## 二、从截图还原的交互模型(闭源移动端)

| 时刻 | 用户看到 | 语义 |
|---|---|---|
| 简单任务 | 直接出回答+文件链接 | 过程太短,不创建过程壳 |
| 复杂任务启动 | 一句"我会先…"+Thinking | commentary 已提交,reasoning_summary 流式 |
| 工具执行 | Running/Searched web/Ran commands | 当前 Item 活跃,已完成进历史 |
| 阶段切换 | 一句过渡叙述 | 新 commentary 隔开两个阶段 |
| 多任务 | "3 running tasks" | 父任务组下多个 active child |
| 最终回答开始 | 过程转成 "Worked for 1m 43s" | 过程冻结,自动收口 |
| 完成 | 过程默认折叠,回答/产物/改动在外 | Activity 与 Result 分离 |
| 用户展开 | 阶段说明+聚合行+原始详情 | 三层密度 |
| 停止 | "Codex run stopped." 独立保留 | interrupted/cancelled 终态 |

关键规则:**最终回答在折叠区外;开工说明和中间说明在折叠区内。**
Codex App Server 用显式 thread/turn/item 生命周期;agentMessage 有
commentary 与 final_answer 两种 phase;item/completed 携带权威终态快照。
TUI 把 active cell 与已提交 history 分开管理。

## 三、做得好的:turnTimeline append-only;三海拔;qp 元数据;
FileChangesCard/产物卡。

## 四、当前代码问题

1. TimelineRole(fold/answer)算了但渲染没用——narration 一律恒可见,
   过程收起后阶段说明仍留在外面。
2. 过程头 `manualHeader ?? true`,完成后不自动折叠。建议三态
   auto/open/closed。
3. 流式空档一律判 thinking(排队/网络/工具间隙/审批/重试/整理结果/
   生成答案/远程/子 Agent 全糊成一个)。
4. ToolDisclosure 开合状态组件内部持有,父层无法统一收口、不知用户
   意图、卸载丢状态。
5. 只有一个"最后活跃工具"(findLast),表达不了 "3 running tasks"。
   需 active_item_ids 集合 + TaskGroup。
6. 前端协议扁平 message list,缺 Run/Stage/Item/phase/approval/retry/
   verification/task group/parent-child/waiting reason/终态快照。
7. reasoning 混合来源(inline <thinking> 拆分)。建议正式命名
   reasoning_summary,内部逐 token 推理不进常规界面。
8. setEverRaw 在 render 阶段触发状态更新,应进 useEffect。

## 五、Agent Activity Protocol(建议)

层级:Conversation → Run → Stage → {Commentary, ReasoningSummary,
ToolCall, Approval, Verification, Artifact} → FinalAnswer → ResultShelf。

RunStatus: queued|preparing|running|waiting_approval|waiting_user|
waiting_external|verifying|composing|completed|failed|cancelled|interrupted
ActivityItemKind: commentary|reasoning_summary|plan|tool|task_group|
approval|verification|progress|artifact|system_notice
ActivityItemStatus: pending|streaming|running|succeeded|failed|
cancelled|superseded
MessagePhase: commentary|final_answer

ActivityItem{id,run_id,stage_id?,parent_item_id?,kind,status,phase?,
title?,summary?,detail?,group_key?,visibility:primary|detail|debug,
importance:normal|important|critical,started_at?,completed_at?}

AgentActivityEvent{schema_version,sequence,event_id,run_id,turn_id,
stage_id?,item_id?,timestamp,type: run.started|run.status_changed|
stage.started|stage.completed|item.started|item.delta|item.completed|
approval.requested|approval.resolved|artifact.created|run.completed|
run.failed|run.interrupted, delta?, item?(completed 必带完整快照)}

三个契约:同 item_id 贯穿 started/delta/completed;completed 带完整
快照可修复丢失 delta;commentary/final_answer 在产生时定 phase。

Activity Journal 与 AgentScope 模型历史分离:前者服务 UI/审计/回放/
折叠/恢复,不参与模型上下文拼装(保留 Runtime Kernel RFC 的担忧)。

## 六、Run 状态机(头部文案 + 默认过程区)

queued 排队/开 · preparing 准备环境/开 · running-reasoning 正在分析/开 ·
running-tool 正在读取 X/开 · running-taskgroup N 个任务进行中/开 ·
waiting_approval 等待确认/开+突出 · waiting_user 需补充信息/开 ·
waiting_external 等待远程/开 · verifying 正在运行测试/开 ·
composing 正在整理结果/准备收口 · completed 已工作 X/**默认折叠** ·
failed 执行未完成·X/默认开摘要 · cancelled 已取消/折叠 ·
interrupted 已停止/折叠但状态恒可见。thinking 只对应活跃
reasoning_summary。

## 七、自动展开与收口

DisclosureMode auto|expanded|collapsed|pinned;优先级 pinned > 用户手动 >
auto policy;用户手动展开后本 Run 内自动策略不再关闭。
活跃:开始即展开;active Item 强调色+唯一动效;工具详情 3-5 行实时输出;
完成 Item 200-300ms 后压摘要;新 Item 开始上一 Item 立刻停动效;同 Stage
同类合并;reasoning 只显状态头;failure/approval/critical 恒可见;artifact
不进折叠。
收口触发 = final_answer.started:冻结过程 → 头切"已工作 X" → 600ms →
无操作则折叠过程体 → 答案继续流式。**滚动锚点**:折叠前后记录答案顶部
坐标 scrollBy 差值(FLIP)。
简单任务跳过过程壳:hasMeaningfulActivity = visibleItemCount>=2 ||
duration>=1200ms || hasCommentary || hasApproval || hasFailure ||
hasVerification || wasObservedWhileLive。

## 八、阶段性描述走独立通道

commentary phase 承载"我先检查…/远端还没有…/推送完成…";final_answer
承载结论。模型约束:非简单任务首次工具调用前一句打算;新阶段一句已确认
事实+下一步;不逐条复述常规工具;原始参数输出交给结构化 UI;结论用
final_answer。前端不判断、不关键词识别。

## 九、视觉结构

Activity Header(正在验证修改·42s) → Commentary → Stage Summary
(检查了 6 个文件/修改了 3 个文件 ±) → Active Item(正在运行 npm test +
3 行输出) → Verification → 已工作 X 〉;折叠区外:最终回答/交付文件/
文件改动/图片二维码。原则:成功中性色不打勾;强调色只给 active;红只给
失败;同一时刻一个主动效;动效只在 active/settling/collapse;reduced
motion 直切。

## 十、文件改造建议

protocol/types.ts 加 phase/Stage/ActivityItem/lifecycle;stream.ts reducer
改 Run normalized store;turnTimeline 降级旧协议适配器;executionTrack
从显式 Run 状态生成 headline;stepGroups 优先后端 kind/group_key;
MessageList 拆 Activity/FinalAnswer/ResultShelf;TurnFlow→ActivityTimeline;
ToolDisclosure 全受控;ReasoningBlock 显 summary+动态标题;ProgressCard
通用 Item renderer;envelope.py 发 phase/stage/item lifecycle/终态快照;
tool_meta.py 加 group_key/progress label/summary/artifact;持久化层存
Activity Journal。

## 十一、四个 PR

PR1 现协议折叠状态机修复:消费 TimelineRole;fold narration 进 Activity
区、answer 出;完成自动折叠 header;ToolDisclosure 全受控;用户优先;
setEverRaw 进 effect;滚动锚点;简单任务跳壳。
PR2 语义化 Activity Projection:Run/Stage/Item 模型;旧消息→事件 adapter;
normalized reducer;终态 reconciliation;live/replay 同 Projection。
PR3 Runtime 事件协议:phase;stage/tool/approval/verification 生命周期;
run 终态;monotonic sequence;Journal 持久化。
PR4 并发/子 Agent/打磨:TaskGroup 多 active;子 Agent;retry;waiting
external;审批卡;reduced motion;a11y;长轨道虚拟化。

## 十二、验收标准

工具 delta 丢失→completed 快照可修复;重复事件→幂等;乱序→旧 sequence
不覆盖;并行工具→全部 active;用户手动展开→完成后仍开;未操作→答案
开始后自动折叠;失败→折叠后仍可见;交付文件→永在折叠区外;中间
commentary→展开可见折叠隐藏;final answer→永在外;简单任务→无空壳;
历史回放→与刚完成时 DOM 一致;自动折叠→答案位置不跳;reduced
motion→无 shimmer/高度动画;中止→明确已停止不伪装完成。

## 结论

三处核心:commentary/final_answer 变协议事实;扁平 list 升级 Run/Stage/
Item 生命周期;开合状态收归 ActivityTimeline 统一策略。退出核心路径:
后验边界推断、空档判 thinking、组件内部开合、单活跃模型。
