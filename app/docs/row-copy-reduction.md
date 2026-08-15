# 过程行文案减法（Claude 裁决，合并 grok 评估 row-copy-grok.md）

用户规则：去"了"；能用图标就不用字；没有图标就不要用字；能省就省。

## 原则

1. 图标 = 动词，文字 = 对象。语义图标（笔/文件/终端/放大镜/地球）
   已能说清动作的行，动词全删。
2. 图标说不清的（技能/other）不补字——只留对象名。
3. 状态词是例外：失败必须留字（禁用 × 图标替代）；运行态由 spinner+
   shimmer 表达，行上不再写"正在"；头部是唯一"此刻"，保留"正在…/
   思考中/N 个进行中"。
4. 聚合行不许删到单位悬空：有对象列 basename（逗号，最多 3 个 + 等），
   无对象才留最小单位。
5. FileChangesCard 不动——那是结果清单，不是过程行。

## 文案表（行 × 静息/运行/失败，中英成对）

| 行 | 静息 | 运行 | 失败 |
|---|---|---|---|
| 改文件 | 笔 `path +4 −1` | 同 + spinner | 笔 `path` **失败/Failed** |
| 读文件 | 文件 `path` | 同 + spinner | 文件 `path` **失败/Failed** |
| shell | 终端 `命令` | 同 + spinner | 整行 danger，不加字 |
| 搜网页 | 放大镜 `关键词`；多次 `关键词 ×3` | 同 + spinner | + 错误首行 |
| 读网页 | 地球 `域名/…` | 同 + spinner | + 错误首行 |
| 搜/匹配文件 | FileSearch/Files `pattern` | 同 + spinner | + 错误首行 |
| 技能 | Sparkles `技能名` | **正在调用** `技能名` / **Running** | + 错误首行 |
| 改 ×N | 笔 `a.ts, b.md 等 +12 −4`；无对象 `3 个 / 3 files +12 −4` | 同 + shimmer | 失败条单列 |
| 读 ×N | 文件 `a.ts, b.md 等`；无对象 `3 个 / 3 files` | 同 + shimmer | 单列 |
| shell ×N | 终端 `wc 等`；无对象 `3 条 / 3 cmds` | 同 + shimmer | 单列 |
| 思考 | `• 标题`；无标题「思考过程/Reasoning」 | 头「思考中」 | — |
| 头 | `8.4s` + chevron | **正在修改** · `8.4s`；或 `N 个进行中` | `8.4s · 2 失败 / 2 failed` |
| 溢出 | `另有 n 步 / n more` | 同 | — |

## 与 grok 稿的差异

- 搜索多次：`关键词 ×3` 采纳；单次不写 ×1。
- 头静息：删"用时"只留 `8.4s`，采纳——时长格式自明。

## 实施

i18n：chat.step.* 改为对象模板（去动词、去"个文件/条命令/次"）；
tool.tense.*.done 全部置空（完成态不再有动词）；tool.tense.*.running
仅技能/other/头保留，文件/shell/搜索行删；chat.workedFor → `{duration}`；
新增 chat.step.more「等 / etc.」。stepGroups 增 basename 列表输出
（最多 3）。渲染：StepGroupRow/TrackRow/FileToolCard/ShellToolCard/
ToolCard 按表改。
