# 清晰感交叉评审（Grok 评 Claude）

对照基线 `c9b5794c`。对方 `design/crispness-claude` 四提交
（`81499ad8` 正赛 + 后加三则产品提交），`app/src` 10 文件 / +101−93。
本侧 `design/crispness-grok` 一提交，31 文件 / +292−229。只读，不改实现。

截图 `crisp-shots-claude/` 拍在正赛提交，不含后加三项。

## 1. 对方做了什么

token 升档与本侧同值（tertiary `#6d6d6d` / muted `#8f8f8f`，深色
`#969696` / `#787878`）；`--icon` / `--tint` / `--ring` 转蓝。
IconButton 默认改 `text-icon`，Button secondary 补 `border-line`，
Switch on 改 `bg-tint`。侧栏白 pill（`bg-surface` + `ring-1 ring-line`
+ `shadow-sm`），深色 `raised` + `line-highlight`。工具行文件名 /
shell / 完成标签升 secondary。会话行锁 13/medium + 11/regular。

未做：全仓 muted 语义迁移、IconButton selected、`global.css` 焦点环、
分组头升档、设置钮 pill、导航 strokeWidth。

后加：时间戳 hover 才显现、分组头去计数、首页 composer 托盘并入输入卡。

## 2. 关键分歧

### a. 选中项文字色 — 对方对

对方：字 `text-ink`，图标 `text-tint`。本侧：字标都 `text-tint`。

`#3b6ef0` 在白面 `#ffffff` 上 **4.49:1**，不过 13px medium 的 4.5:1
AA；若字落在侧栏底 `#f5f5f4` 上更差，**4.12:1**。深色 `#6b8fe6` 在
raised `#2a2a2a` 上 4.58:1，擦线过。ink 在同一白面上 16.29:1。

方案原文写「图标+文字着色」，没把 tint 叠在白 pill 上重算。白 pill
已经用换层标明「在这」；再把 13px medium 整段涂蓝，是第二记信号，
而且蓝字在白面上发飘，像超链接，不像选中。一滴颜色的正确落点是
**16px 图标**，不是标签。对方这里比方案原文更对。

### b. 侧栏接缝 — 浅色我对，深色各有道理

基线浅色已有 `border-r border-line`。`#ececea` 贴 `#f5f5f4` 对比
**1.08:1**，几乎看不见——这正是 r3 说「仍糊」的那条缝。对方原样保留，
等于没做这项。`#dcdcd9`（line-strong）到侧栏底 1.26:1，刚好够当一条
发丝，不是框。

深色对方继续 `dark:border-r-transparent`（靠抬升分层）；本侧改
`line-highlight`。深色两边都能成立，本侧略完整。

### c. 迁移策略 — 竞赛语境对方对

审查表 155 muted + 53 tertiary。对方留下 147 处 muted、6 处
`text-icon`；本侧 muted 收到 20（真占位/禁用/装饰），`text-icon` 46。
token 改值让长尾一起加深，但「内容本体停在 muted」的相对关系没变：
设置说明、空态、审批字段、搜索结果仍按占位符的身份出现。

竞赛评的是主舞台（侧栏 + composer + 工具行）清不清，不是 token
分类表。208 处 class 对换在照片里看不出来，却把 diff 撑成「大迁移」。
正赛该做原语 + 投诉面；长尾是合并后的机械 PR。本侧把赛后活做进了
交卷，工程上更对，赛制上过重。

### d. 后加三项（不评分）— 分组头同意；托盘原则同意；时间戳有保留

**分组头去计数。** 同意。用户不能点这个数字，它只是 chrome。项目行
里那条 10px 计数留下是对的——那是「这个项目有几条」，可用来决定展不
展开。`t(..., { count })` 还在传、`workspacesGroup` 文案仍带
`({count})`，是没 sweep 干净，无运行时害处。

**composer 托盘并入输入卡。** 原则同意。首页双层（白卡 + 下挂托盘）
是一块多余的面，并进控制行后与会话页同形，chrome 少一块。
`ProjectPicker` 有 `max-w-40`，窄窗靠 truncate，布局过得去。
`--composer-tray` 成了孤儿 token；`renderApprovalControl` 注释还在
讲托盘，属收尾不净。占位去掉「@ 引用文件，/ 调用技能」偏过：新用户
少了唯一书面提示，发现性应另找落点（空态或第一次触发），不要只靠
「触发器自己可发现」。

**时间戳 hover 才显现。** 静息态标题独占，扫描更干净，方向对。但实
现把「…」绑在 `group-hover/zone` 上：只有指针进入行尾约 `min-w-9`
的时间槽，菜单才淡入。基线是整行 hover 出菜单。这是可发现性回退。
无 hover 的指针（触摸、部分手写笔）既看不到时间，也看不到菜单——
隐形 `IconButton` 仍可点，等于暗按钮。键盘靠 `focus:opacity-100`
能出来，但用的是 `:focus` 不是 `:focus-visible`，和 r3 口径不一致。
若做，应：静息可藏时间，菜单仍走整行 hover。

## 3. 对方 bug / 疏漏

按是否伤到 r3 承诺排列。

1. **C2-lite 焦点环没接上。** `tokens.css` 把 `--ring` 改成蓝，
   IconButton / Button / Switch 的 `ring-ring` 是蓝的；但
   `global.css:76` 键盘 outline 仍是 `var(--accent)` 灰。NavLink
   走全局 outline，工具栏走 `--ring`，同一套键盘焦点两种颜色。
   实现说明写「--ring 转 tint 蓝（focus-visible 路径）」，只做了一半。
2. **r3 的点击清环没做完。** 本侧把 `:focus:not(:focus-visible)`
   的选择器扩到与 `:focus-visible` 对齐（含 select / menuitem /
   tabindex）。对方仍是基线那四个角色。点击残留是 r3 点名的反精致。
3. **设置钮完全没进 pill。** `Sidebar.tsx:367-372` 选中仍
   `bg-fill-active text-ink`，未选中仍 `text-ink-muted`。五个主导航
   换了层，脚下一颗还是灰底灰标。
4. **搜索行状态不完整。** 无 `active:`、无 150ms ease-out，和上面
   四条 nav 不一套控件。
5. **IconButton 无 selected。** r3 写明原语要独立 hover/selected，
   不能只加深图标。标题栏「新会话」在首页无选中态（本侧
   `AppShell.tsx` 接了 `selected={pathname === "/"}`）。
6. **选中 pill 缺 selected+hover。** 对方选中态没有 hover/pressed
   差分。r3：「没有就是 class 不是控件」。
7. **分组头仍是 tertiary 常态。** 方案补则写明折叠组入口不再用
   tertiary。对方 `Sidebar.tsx:234,313` 原样。项目行文件夹 /
   计数 / chevron / 会话 Pin 仍 muted，`--icon` 两档只落到五个
   主导航。
8. **深色 `--shadow-control` 全局改成高光描边。**
   `0 1px 2px rgba(0,0,0,.28)` → `0 0 0 1px rgba(255,255,255,.06)`。
   次级按钮该这么做；但深色主按钮是近白面，黑影是有效像素，换成
   白环等于抽掉主 CTA 的高度。本侧把 `dark:shadow-none` 写在
   secondary / pill 上，不改 token，这点本侧对。
9. **时间戳两段 hover 的触发面过小**（见 2d）。算实现缺陷，不只是
   口味。
10. **死 token / 过期注释。** `--tint-ink` 定义未消费；托盘退役后
    `--composer-tray` 无引用；`tokens.css` 文件头仍写「所有界面
    chrome 都使用中性灰」，和 C2-lite 正文打架。

未构成 bug：`--dur-selected` 没建、用了硬编码 `duration-150 ease-out`，
等价。正赛提交的 tsc / 236 测试对方自称全绿，本评审未复跑。

## 4. 自我修正

看完对方之后，本侧只改一处视觉决策：**选中标签从 `text-tint` 改回
`text-ink`，图标继续 `text-tint`。**

这是本侧唯一站不住的点。方案原文被我执行得太字面，没把「白 pill +
13px 蓝字」的对比度和信号重复算进去。截图并排时，对方的近黑字 +
蓝标更克制，本侧蓝字在白面上发飘。

不会撤回 208 处迁移——那是产品正确，只是不该算进竞赛分。不会改回
`border-line` 接缝。不会把深色 `--shadow-control` 改成对方那样的
全局高光。分组头计数、composer 单卡作为产品 PR 另走；时间戳 hover
要先修触发面再谈吸不吸收。

## 5. 合并策略

以本侧为基底。token、语义升档、IconButton selected、`global.css`
焦点对齐、Button secondary 的深色就地覆盖、接缝 `line-strong`，
已经把 A/B/C2-lite 铺进产品，倒回对方「原语+重点区」是退步。

从对方只吸两件进这次合并：

1. **选中字改 ink，只留图标 tint。** 必须改。pill 边继续用本侧
   `border-line` + `shadow-control`（比对方 `ring-1` 稳，且预留了
   透明边，选中不位移），深色继续 raised + `line-highlight`。
2. **分组头去掉不可操作的计数。**

不吸收：深色全局改 `--shadow-control`（伤主按钮）；时间戳 zone
hover（触发面回归）；composer 单卡（另开产品 PR，顺手删
`--composer-tray`、补回 @ / 的发现性）。长尾 muted 本侧已做完，
对方那份不必再合。
