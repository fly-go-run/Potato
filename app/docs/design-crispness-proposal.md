# 方案：清晰感重校准（去灰蒙蒙）— r1 讨论稿

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

## B. 控件质感（Apple 三件套）

- 次级按钮/可点 chip：surface 白底 + line 发丝边 + shadow-control
  （已有 token，未被用起来）；深色以表面抬升+line-highlight 替代影。
- 侧栏选中态 pill：fill-active 加深一档或改白底+边，让"当前位置"
  从"稍微灰一点"变成"明确的一块"。
- lucide 图标统一 strokeWidth（候选 1.75/2）与偶数尺寸网格。

## C. 一滴颜色（需用户拍板，二选一）

- C1 全灰保守派：保持无彩，靠 A+B 拉满对比（Codex Desktop 路线）。
- C2 克制 accent：恢复一个低饱和蓝（历史上用过 #2563e0），仅用于
  四处——选中态图标、focus ring、主 CTA、开关 on 态。侧栏图标常态
  仍中性，选中着色（Apple Notes 式）。

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
