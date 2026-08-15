# Activity Protocol 三方交叉评审（Claude 执笔裁决）

材料:activity-protocol-gpt.md(GPT 调研)、codex-states-research.md
(grok 源码调研)、本文件(Claude 核实 + grok 评审 + 裁决)。

## 事实层(源码级,三方一致)

- `commentary`/`final_answer` phase **属实**:Codex protocol 与 App
  Server schema 就是这两值;`item/completed` 带完整快照属实。
- **但 schema 明写 provider 不保证发 phase,`None` 必须当 unknown**
  (TUI 把 None 当 FinalAnswer)。GPT 把 phase 写成"协议事实、前端
  不判断"是夸大——它是可选提示,不是硬边界。
- "回答完过程自动折叠只留 Worked for":**开源 TUI 不做**(只藏
  status 行,工具行永留 transcript,>60s 才写 Worked for 且是答案前
  分隔符);闭源移动端**做**。同一 App Server 两种呈现,可同时为真。
  Potato 在 e1cba855 已跟 TUI 一边。

## GPT 八条代码诊断复核(对照 e1cba855 后)

| # | 判词 |
|---|---|
| 1 TimelineRole 未消费 | 事实在,定性错——是故意的(消灭形态摇摆),当 bug 修会请回旧病 |
| 2 头不自动折 | 半过时——短回合已不画头;"完成后折过程体"未做,也不该做 |
| 3 空档一律 thinking | 仍对 |
| 4 ToolDisclosure 自持 | 半过时——Shell/Reasoning/StepGroup 已受控;FileToolCard/GenericToolCard/FailedToolRow 仍自持 |
| 5 单活跃工具 | 头对轨过时——轨道已能并排多组,缺的只是头上"N 个进行中" |
| 6 扁平 message list | 仍对,但这是 AgentScope 宿主的特征不是漏做 |
| 7 reasoning 混源 | 仍对 |
| 8 setEverRaw 在 render | 仍对,**唯一无争议真 bug** |

## 协议裁决:GPT 的 Activity Protocol 为过度设计

Run/Stage/Item + 11 态 RunStatus + Journal 持久化,对 AgentScope 宿主
过重。RFC runtime-kernel 已定:AgentScope 拥有消息历史,禁止事件日志
当真相源、禁止永久双写。Journal 持久化 = 双写;若只是 StreamMessage
上的投影,不必先发明事件栈。

**能直接落**:前端把一轮当 Run、slot 当 Item(已是);工具终态快照走
已有 qp;可选在 Message 上透传 phase(None=unknown)。
**会冲突**:Stage 当后端对象;Journal 当回放源;lifecycle 事件替换
AgentScope output。

## 折叠裁决:站 TUI,不做"回答完自动折过程体"

Claude 原判"必须等 phase 落地再做";grok 更进一步:即便 phase 落地
也不该做成"只留 Worked for"——phase 是可选提示、TUI 实证不折、
形态摇摆是已消灭的病。**采纳 grok。**用户想要的"完成感"由三处
已落地/在落地的机制提供:短回合不画头、静息态摘要行讲故事、
用户手点头一键全收。若日后仍要移动端式全折,前置条件是 phase
落地 + 用户实际使用反馈,不是现在。

## 采纳清单(GPT 稿中值得留下的)

- 验收标准表(幂等/乱序/快照修复/回放一致/答案位置不跳/中止不
  伪装完成)——进 RFC tool-runtime r3 作为过程展示的硬指标。
- 空档状态拆分(#3):至少把"等待模型首帧/整理结果"与"思考中"
  分开,前端可做。
- 视觉原则段(强调色只给 active、同一时刻一个动效、红只给失败)——
  与现行设计法则一致,归档。
- 提示词约束段(非简单任务首次工具前一句打算、阶段一句过渡)——
  即 PR3,不变。

## 拒绝清单

自动折叠过程体;narration 按 TimelineRole 抽进 Activity 区;四 PR
协议重写;Activity Journal 持久化;11 态 Run 状态机;简单任务跳壳
启发式(短回合不画头已覆盖);为折叠做 FLIP scroll anchoring。

## 第一刀(小、确定、可验)

1. `MessageList.tsx`:setEverRaw 搬进 useEffect。
2. `FileToolCard.tsx` / `ToolCard.tsx`:FailedToolRow、GenericToolCard
   补完受控开合(open/onToggle 从父层来),与 Shell/Reasoning 同一模式。
3. `executionTrack.ts`:头文案支持"N 个进行中"(多 active 工具时),
   替代 findLast 单活跃。
4. `executionTrack.ts`:流式空档拆"等待回复"与"思考中"(有 in-flight
   reasoning 才叫思考中)。
5. phase 若做:`envelope.py` + `stream.ts` 加可选字段,None=unknown,
   **不改折叠**;配合 PR3 提示词让模型主动分段。
