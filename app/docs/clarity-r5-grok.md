# 清晰度 r5：混在灰阶，不在缺蓝

对照 ChatGPT 移动端实测 vs `design/crispness-final`。只读 tokens / Sidebar / Composer / IconButton。不改代码。

## 1. 同意「中灰配浅灰 = 混」。secondary 不整体转近黑

#505050 在侧栏底 #f5f5f4 上约 7.4:1，WCAG 过线，观感仍脏：它和 fill-hover #eeeeec、line #ececea、tertiary #6d6d6d 挤在同一段灰阶里，墨不够黑、底不够白。

不把 `--ink-secondary` 整表改成 #2a2a2a。那一档贴 ink #202020，四级字阶塌掉，说明/推理/完成态会和标题抢重量。token 留着，**收窄使用面**：chrome（分组头、项目行、ghost 字、菜单项）升 ink；supporting 正文留 secondary。投诉面是 `--icon #505050`：16px 导航和工具栏静息是炭笔未按实。icon 升到 ink（与 icon-strong 同值）；chevron / spinner 继续 tertiary。

若动 token 值：secondary 最多压到 #3a3a3a。推荐本轮不动值，先收面。

## 2. 侧栏不转纯白。留灰，字标全转 ink

不值得。选中 pill 是 `bg-surface` 白底 + 发丝 + 浅影，活在 `--bg` 上才换得了层。侧栏改 #fff 后 pill 溶进底，拆掉 r3 最贵的一块，深色 elevation 反转也靠四级底。ChatGPT 纯白是移动端单层舞台，不是桌面工作台。

做：`--bg` 保留；推荐再压半档到 #f0f0ee，让白 pill / 画布 / 侧栏能分开（补 r3 接缝糊；line-strong 已在）。未选中导航字已是 ink；分组头、项目名、图标升 ink。不废 WB 地基。

## 3. IconButton 静息不加圆底。Chip 要像一块东西

桌面 ChatGPT 网页版 composer 也是裸图标 + 实心发送。移动端圆底是给手指做靶，不是清晰度。

**抄：** 内容与图标近黑；占位才中灰；发送是实体；分隔靠留白和填充块；选中靠换层。

**不抄：** 17–20px 字与高大行；侧栏改死白；每个 IconButton 静息圆井；发送改实心蓝药丸；砍掉卡片/选中 pill 发丝。

IconButton 保持静息裸、hover 出 `fill-hover`。改墨不改井：`text-icon` 升 ink 后，hover 靠出底，不再靠灰变黑。Composer 审批是 ghost + secondary + `--icon` 盾牌，像漏排的标签；ProjectPicker 触发同病。ModelPicker 已是 ink 字 + hover 底，方向对。审批与项目 chip 静息给 `bg-fill-hover`（或 `bg-bubble-tool`）、字标 ink、不描边。+ / 麦 / 发送不加井。

## 4. 这次是灰阶 + 形状。第四消费点：否决

上轮「A+B 治淡不治灰是色相真空」对当时成立。本轮截图投诉是「灰和混、不够清楚」。ChatGPT chrome 几乎无彩：近黑字、白底、浅灰井、一颗蓝 CTA。用户没喊缺蓝。

混的主因：① #505050 字/标铺在浅灰底；② ghost chip 无实体；③ 侧栏/画布/hover/线挤在约 3% 明度里。tint 扩到发送或 `--btn-primary` 解决不了这三项，只是把品牌从 Codex 近黑换成 GPT 蓝。发送已是 36px 实心圆，全页最硬的一块。

**C2-lite 第四消费点本轮不开。** 三处不动（侧栏选中图标、`:focus-visible` ring、Switch on）。主 CTA 近黑。若 r5 后仍发死，候选是审批 AUTO 小标，不是发送键。

## 5. r5 清单（先 P0）

**P0 token**

- 浅色 `--icon: #202020`（= ink）。深色 `--icon: #ececec`。
- `--ink-secondary` 维持 #505050。
- `--bg` → #f0f0ee。
- 不改 canvas / surface / tint / btn-primary。

**P0 组件**

- `IconButton.tsx`：静息已 `text-icon`，跟 token；不加底。
- `Sidebar.tsx`：分组头、项目行 `text-ink-secondary` → `text-ink`。
- `Button.tsx` `ghost`：`text-ink-secondary` → `text-ink`。
- `Composer.tsx` 审批：ghost 补静息 `bg-fill-hover`；盾牌去掉 `text-icon` 覆盖。
- `ProjectPicker.tsx` 触发：与审批同待遇。

**P1**

- Sidebar `MenuItem` 升 ink。
- 会话行锁 13/medium + 11/regular；composer 输入最多 15→16px。
- 选中 pill 发丝保留。

**P2 不做**

- 侧栏 #fff；IconButton 静息圆底；tint → 发送/主按钮；正文 17–20px；全仓 secondary 改值。

验证：浅色侧栏 + composer 主舞台；深色 pill 仍 raised + line-highlight；ghost 取消钮字 ink、无填充，不得比主按钮更抢。
