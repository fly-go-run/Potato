# 方案：清晰感重校准（去灰蒙蒙）— r2 定稿

r2 说明：吸收 codex 事实审查（design-crispness-review.md）。外部多模型
评审未成行（agy 地区不可用、grok CLI 无批处理模式），观点席由方案
owner 承担。r2 变更：tertiary 定值 #696969（清 bubble-user 上 4.5:1）、
muted #8f8f8f 维持（收紧语义后仅占位/禁用/装饰,3:1 豁免成立）、
深色 tertiary #969696 / muted #787878、--icon 落点定为 IconButton
原语+清单页逐处、B 项按 codex 盘点缩为「补一条 border-line + 深色
shadow→line-highlight 切换 + 侧栏白底选中 pill(局部改,不动全局
fill-active)」、奇数尺寸图标归格(13→14/15→16,67 处)独立成
mechanical 工作包派 codex、C2 缩为 C2-lite(见 §C)。

背景：用户反馈界面"过于淡、过于灰、不够精致"，点名侧栏按钮；要
清晰感不要朦胧感，参照苹果的控件质感。诊断（已核实 file:line）：

1. 侧栏主导航图标用 `--ink-muted #9b9b9b`（Sidebar.tsx:141,158,178,
   198,209）——token 注释里 muted 的定义是"占位符、禁用"
   （tokens.css:31）。muted 对侧栏底 #f5f5f4 对比度 ≈2.7:1。
2. `--accent` 是灰色 #4a4a4a（tokens.css:43）：全界面无彩色锚点，
   选中态=灰底灰字灰图标。
3. 次级控件无"白底+发丝边+浅影"三件套，靠单层灰填充暗示边界。

## A. 对比度重校准（无争议项）

- 新增语义 token：`--icon`（常态图标）= ink-secondary 档、
  `--icon-strong` = ink 档。主导航图标 muted→icon，选中→icon-strong。
- muted 用途收紧回定义：只允许占位符/禁用/装饰;现有误用逐个升档。
- tertiary 浅色 #747474→#6d6d6d，muted #9b9b9b→#8f8f8f（仍是四级,
  但底部两档不再跌穿可读线）;深色对应微调。
- 时间戳/元信息统一 tertiary，不再混用 muted。
- **原则（用户 r1 反馈补充）：「安静」靠版式与尺寸表达，不靠灰度。**
  完成态的降级只允许发生在行高/边框/占位上,内容本体不降档:
  - 工具行文件名 `text-ink-muted`→内容档(FileToolCard pathNode);
    shell 命令完成态 tertiary→secondary;通用工具完成标签同理。
  - 侧栏会话标题→ink;分组标题/时间戳统一 tertiary(加深后的);
    折叠组入口不再用 tertiary 当常态。

## B. 控件质感（Apple 三件套）

- 次级按钮/可点 chip：surface 白底 + line 发丝边 + shadow-control
  （已有 token，未被用起来）；深色以表面抬升+line-highlight 替代影。
- 侧栏选中态 pill：fill-active 加深一档或改白底+边，让"当前位置"
  从"稍微灰一点"变成"明确的一块"。
- lucide 图标统一 strokeWidth（候选 1.75/2）与偶数尺寸网格。

## C. 一滴颜色（需用户拍板，二选一）

- C1 全灰保守派：保持无彩，靠 A+B 拉满对比（Codex Desktop 路线）。
- **C2-lite（owner 推荐,r2 缩窄）**：不动现有中性 `--accent`（它被
  12 处装饰用途共用,重映射成本高——codex §5.3）,新增独立
  `--tint`(低饱和蓝,基准 #3b6ef0 微调),只接三处:侧栏选中态的
  图标+文字着色、focus ring、Switch 开启态。**主 CTA 保持近黑**
  (Codex/WB 式黑按钮已是品牌感的一部分,不改)。这样 C2 从"半天
  语义重映射"缩为"一个新 token + 三个消费点",且不违反"装饰零
  彩色"——三处全是状态信号。历史决议(45c81d09 去彩)以本方案为
  显式重开记录。

## r3 增补：grok 4.6 评审吸收（owner 裁决后并入）

grok 站队 C2-lite，核心论证被采纳记录："苹果质感来自换层、发丝边和
态的可读，不来自彩；但 A+B 治淡不治灰——那是色相真空。Codex 能全灰
是因为主舞台有语法高亮供彩，本产品侧栏没有这个锚。"

采纳的修正：

1. **tertiary 回调 #6d6d6d**：不让最差底(bubble-user)绑架全局值,
   时间戳不该逼近正文重量;气泡内 tertiary 文本极少,4.38:1 可接受。
2. **图标分两档**：`--icon`(#505050)只给导航/工具栏档;行内 chrome
   (chevron、spinner、disclosure 箭头)走新 tertiary 档,避免"炭笔
   点阵"。IconButton 原语需配套独立 hover/selected 态,不只加深图标。
3. **锁死排版层级**（补"安静靠版式"的落地）：侧栏行=标题 13px/
   medium + 时间 11px/regular——差一个字号加一档字重,不靠灰度分层。
4. **侧栏-画布接缝**加 1px line（#f5f5f4 贴 #fbfbfb 仍糊）。
5. **选中 pill 补全状态**：pressed、selected+hover、折叠态;深色不抄
   白 pill,改抬升面+line-highlight;选中切换 150ms ease-out。
6. **focus ring 只走 `:focus-visible`**,点击不得残留(实施时验证)。
7. **图标 stroke 按尺寸配**:14px 配 2.0 会偏粗,归格时逐档定
   (14→1.8 / 16→1.75~2 实测定),不做全局一刀切。
8. C2-lite 加护栏:`--tint` 出现第四个消费点必须重新决议。

## codex 审查请求（不要审美判断，只要事实与风险）

1. 全仓 grep `text-ink-muted`/`text-ink-tertiary` 使用点分类：哪些
   是"真占位/禁用"（A 项不动），哪些是误用（要升档）？给清单。
2. token 值修改的波及面：有没有组件把 muted/tertiary 当"装饰性弱
   化"依赖（升档后会显得突兀的场景）？
3. 对比度核算：A 项新值在 canvas/bg/surface/bubble 四种底上的
   对比度表（现值 vs 新值）。
4. B 项三件套对现有 Button/chip 原语的改动范围盘点（组件清单）。
5. C2 若采纳，历史 r3 曾把彩色 accent 拿掉的原因（查 git log/
   设计文档）是什么，是否仍成立？

输出写入 `app/docs/design-crispness-review.md`，只报事实与清单，
不作审美裁决。

## r4 增补：文案克制规则（2026-08-14 文案轮沉淀，来源 copy-restraint-grok.md）

1. **先删再问**：小字拿掉后用户会不会选错、点错、把灰控件当坏了?
   三个都否→不配。错了靠事后错误提示，不提前讲课。
2. **含义写进控件，不写在控件底下**：选项文案已说清"是什么"时，
   行下 description 必删（审批四档从"自动+一行解释"改为"自动放行"
   即此例）。
3. **只留三类字**：(a) 选错会坏且选项看不出;(b) 禁用/未生效的原因;
   (c) 当前状态值。其余删。
4. **给人看的和给模型看的分开**：SKILL.md description、内部 id、后端
   枚举不当主文案;用户可见文案走 skillPresentation 层。
5. **中英成对、三处一套**：技能文案改动必须同步 / 弹层、设置列表、
   SkillsView;不许页面私写 Title Case。
