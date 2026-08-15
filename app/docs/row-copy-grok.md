先读现状文案、渲染位置和图标，再独立评估「图标承担动词、文字只留对象」能不能省到自明。用户要的是只读评估文案，不是完整设计文档。接着核对各行的渲染、图标和运行/失败态。先核一下字数，保证评估落在 600–900 字，再交终稿。**结论：单条过程行可以激进到「图标=动词、文字=对象」；聚合行不能把单位词和对象一起删；「正在」只留在直播头上。**

原则：语义图标已在 `FAMILY_ICONS` / 卡片里（lucide，不必新引入）。图标能说清的动词删；图标说不清的不补字，只留对象。失败是状态，必须留字。`FileChangesCard` 不动——结果清单仍写「修改 N 个文件」。

**单条改动**（`FileToolCard` 完成态已是此形）：`FilePenLine` + 路径 + `+4 −1`。笔=改、±=幅度，动词冗余。首次能懂。写/改/追加共用笔，差在展开层。

**单条只读**：`FileText` + 路径。文件图标单独较弱，但与笔对立后=看过。不换 Eye。

**单条 shell**：`Terminal` + 命令。删「运行命令」。命令自明。

**搜索**：不要叠地球+放大镜。`Search` + 关键词 = 搜（自明）；地球留给 fetch（`Globe` + URL）。grep=`FileSearch`+pattern，glob=`Files`+pattern，同删动词。

**技能 / other**：`Sparkles`/`Wrench` 不是动词。按「没有图标就不要用文字」：只留技能名，删「调用技能」。首次弱于文件行，可接受。

**聚合**：否决「笔 + 3 个 · ±」——「个」悬空，首次不懂。采用：有对象则 `图标 + 2–3 个 basename + 等 + ±`（删「个文件」；改动卡仍写全称）。无对象才留最小单位 `3 个` / `3 files`。搜索：`关键词 ×3`，不写「次」。shell：首条 argv0 + 等。

**思考**：`• 标题` 已最简；无标题保留「思考过程」（身份，不是动词）。

**头**：静息只留 `8.4s` + chevron，删「用时」。`8.4s` 是时长格式，首次能懂。失败：`8.4s · 2 失败`。直播头保留「正在… / 思考中 / N 个进行中」——头是唯一「此刻」，无对象时 spinner+数字不够。

**运行**：行上 spinner+shimmer 已是进行时，「正在写入」是双重状态。有语义图标+对象的行：删「正在」，图标不变。无对象或非语义图标（技能/other/头）：保留「正在…」。

**失败**：整行 danger。文件行保留行尾「失败」（失败时 ± 不出现，路径看起来像成功）。shell 有命令+变色即可。通用失败：名词 + 错误首行。禁止用 × 图标替代「失败」。

图标均已在：Search / Globe / FileSearch / Files / FileText / FilePenLine / Terminal / Sparkles / Wrench / ChevronRight。

### 文案表（行类型 × 静息 / 运行 / 失败）

| 行 | 静息 zh / en | 运行 zh / en | 失败 zh / en |
|---|---|---|---|
| 改文件 | `FilePenLine` `e2e.txt +4 −1` | 同上 + spinner（无「正在」） | `FilePenLine` `e2e.txt` **失败** / **Failed** |
| 读文件 | `FileText` `foo.ts` | 同上 + spinner | `FileText` `foo.ts` **失败** / **Failed** |
| shell | `Terminal` `wc -l e2e.txt` | 同上 + spinner | `Terminal` `wc -l e2e.txt`（整行红，不加「失败」） |
| 搜网页 | `Search` `关键词` 或 `关键词 ×3` | 同上 + spinner | `Search` `关键词` + 错误首行 |
| 读网页 | `Globe` `example.com/…` | 同上 + spinner | `Globe` URL + 错误首行 |
| 搜/匹配文件 | `FileSearch`/`Files` `pattern` | 同上 + spinner | 图标 + pattern + 错误首行 |
| 技能 | `Sparkles` `技能名` | **正在调用** `技能名` / **Running** `name` | `Sparkles` `技能名` + 错误首行 |
| 改×N | `FilePenLine` `a.ts, b.md 等 +12 −4`；无对象才 `3 个 +12 −4` / `3 files +12 −4` | 同上 + shimmer | 失败条单列，不进聚合 |
| 读×N | `FileText` `a.ts, b.md 等`；无对象才 `3 个` / `3 files` | 同上 + shimmer | 单列 |
| shell×N | `Terminal` `wc 等`；无对象才 `3 条` / `3 cmds` | 同上 + shimmer | 单列 |
| 思考 | `• 标题`；无标题「思考过程」/ Reasoning | 头：「思考中」/ Thinking | — |
| 头 | `8.4s` + chevron | **正在修改** · `8.4s` / **Editing** · `8.4s`（或「N 个进行中」/ `N running`） | `8.4s · 2 失败` / `8.4s · 2 failed` |
| 溢出 | `另有 n 步` / `n more` | 同静息 | — |

`FileChangesCard` 仍是「修改 N 个文件 + 清单」，不在此表。