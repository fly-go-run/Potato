# 侧栏按钮返工（r5 后）

白 pill 否决成立，方向对，但「加深一块灰」只对了一半。`Sidebar.tsx` 选中已是 `rgba(0,0,0,0.08)` / 深色 `0.10` 白，白底+发丝+浅影已经不在。残件是常驻透明 `border`、过渡还夹着 `border-color/box-shadow`、会话行 `rounded-md` 与导航 `radius-sm` 不齐、设置圆钮选中仍 `text-ink` 无 tint。更大的洞是 hover：`fill-hover #eeeeec` 叠在 `#f0f0ee` 上 ΔL* 只有 0.7，几乎看不见，按钮不像能按。

选中用 alpha 黑，不用实色。`fill-active #e6e6e4` 叠底 ΔL* 仅 3.5，比现 8% 黑还浅，当不了选中；也不许改全局 token，chip / 菜单会一起脏。侧栏自管一层：浅 hover `rgba(0,0,0,0.04)` / selected `0.08` / pressed 与 selected+hover `0.12`；深 `0.06` / `0.10` / `0.14`。8% 是 macOS 地板，在 `#f0f0ee` 上够分量；加到 10% 只脏，不更像按钮。无描边、无阴影、无白底。选中图标留 `--tint`，字留 ink。

高度 `py-2`≈36px、`radius-sm` 8px、导航 14/medium 与会话 13/medium：**不改**。五条主按钮要这块靶，本轮不拆密度。`gap-2.5` **改 `gap-2`**（lucide 16 自带光学边，10px 空）。pressed **必补**，选中按下去现在不变。过渡拆成 hover 120ms / 选中 150ms，删常驻 border。

白 pill 退场（同一套灰填充）：新建会话、定时任务、技能、记忆、搜索（无选中，只共享 hover/pressed）、会话行含嵌套、底部设置圆钮（选中补 tint，不要拉成整行）。深色 raised **一并退回填充**——代码已是 10% 白，文档里「深色抬升 pill」作废，禁止写回 `bg-raised`。r5 图标近黑、侧栏 `#f0f0ee`、chip 实体、底部 Potato 头像不动。
