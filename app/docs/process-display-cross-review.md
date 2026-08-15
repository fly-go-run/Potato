# 过程展示两稿互评（Claude 执笔）

两稿：process-display-claude.md（Claude，先成稿）、
process-display-grok.md（grok，后成稿）。

**独立性披露**：Claude 稿先提交进共享工作区，grok 重跑时读到了它
（其 References 明确引用并列差异）。grok 稿有大量独立代码考证
（时间戳坍缩、键名、debug 后门均自查属实），不是转写，但本轮
不构成严格双盲。责任在 Claude 的提交顺序。

## 收敛面（两稿一致，视为定论）

三海拔信息架构；头部时长优先（"工作了 X"）、计数退位；连续同类
聚合成摘要行；静息态 = 头 + 摘要行（讲故事不报数）；单条组直出
不套两层；叙述/失败/产物恒可见不动；计划句纯提示词、前端不检测;
行数封顶 ~8 + 溢出行;turnTimeline 不动、重构只在物化层。

## grok 稿独有且经 Claude 核实的增量

1. **历史时钟事实**：持久化历史里 call/output/answer 共用一个
   timestamp（fixture 实证），过程条目 min/max 恒为 0。Claude 稿的
   "无计时回落「查看过程」"因此不成立且更差;grok 的
   historyTurnDuration（用户消息→最后助手消息）+ 回落保留
   「已完成 N 步」正确。**采纳 grok。**
2. **fold-row 状态机**（focus ≠ row、rowByKey/everRaw 提到 TurnFlow、
   思考行复用 ReasoningBlock 头避免双 chevron、思考永不自动展开）:
   Claude 稿没到这个深度,全部采纳。
3. 记账与渲染分离（失败升 visible 仍进计数/时长/showHeader）;
   聚合分界规则（无名/失败切断合并）;shell 摘要只露 argv0
   （隐私考量）;去掉头上密度切换、只留 debug 后门。均采纳。

## Claude 的保留意见（不阻塞,实现期处理）

1. **文案终稿要过"文案克制五规则"**:grok 的键值是骨架,中文措辞
   （"搜索了网页 3 次"档）实现时由设计侧终审。
2. **edit 摘要行与 FileChangesCard 的同轮重复**:一轮改了文件,
   故事线里有"修改了 N 个文件 ±",收口后下方又有改动卡。两者角色
   不同（过程位置 vs 结果清单）,Codex 也两者并存,先保留;真实
   使用中若被判冗余,砍摘要行保结果卡。
3. PR1 先行(头部时长化 + 历史墙钟)是低风险高感知的第一刀,同意
   其排序。

## 结论

以 grok 稿为实施规格（PR1 → PR2 → PR3），Claude 稿作为方向对照
归档。两稿在未通气的前半程独立收敛于同一架构，方向置信度高。
