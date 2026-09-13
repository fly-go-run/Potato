# 真实截图参考（2026-09-06）

以下图片从公开 GitHub 问题报告下载，已逐张查看；不是生成图。截图只代表报告中的版本。

## Codex App：主要视觉参考

- 文件：`codex-composer.png`
- 来源：https://github.com/openai/codex/issues/34021
- 原图：https://github.com/user-attachments/assets/a7cdf54f-5df0-42a1-8fdc-0ef50b88a23c
- 报告版本：26.715.31925，macOS。
- 可见结构：队列以窄条贴在输入框上沿，略窄于输入框；同一深灰表面、细边框、上方圆角。左侧队列图标，中间单行文本省略，右侧 Steer、删除、更多操作。没有独立的大标题区。
- 图中发送菜单明确显示 Queue ↩、Steer ⌘↩。截图可证明菜单文案和布局，不能独自证明 Steer 等价于立即取消运行。
- 原报告讨论新增队列失败；这里只参考图中已有队列和发送菜单，不把失败状态当作正确行为。

## Claude Code CLI：轻量排队列表参考

- 文件：`claude-cli.png`
- 来源：https://github.com/anthropics/claude-code/issues/82511
- 原图：https://github.com/user-attachments/assets/e1b50444-1b2a-482d-afd3-351e14e31f3c
- 报告版本：2.1.220，Linux 终端。不是 Claude 桌面 App。
- 可见结构：运行状态下方、输入分隔线上方有两条待处理内容；采用连续文本行与弱背景，左侧 ! / ❯ 表示内容类型，没有独立卡片和每行按钮。
- 输入位置直接提示 Press up to edit queued messages，最底部有 esc to interrupt。
- 目前未核实到同等清晰的 Claude 桌面 App 队列区域原图，不把 CLI 的外观推定为桌面 App 外观。

## Codex App：溢出反例

- 文件：`codex-overlap.png`
- 来源：https://github.com/openai/codex/issues/12448
- 原图：https://github.com/user-attachments/assets/3d164fff-a79c-49e2-86e7-c3a9a85d1c34
- 这是大量条目重叠的故障截图，不用于复刻正常布局。提醒实现时限制队列高度、允许滚动、避免条目和按钮互相遮挡。

## Potato 设计收敛

以 Codex App 窄条结构为主，采用 Claude CLI 的低视觉负担。队列成为输入框上沿的附属区域，1–3 条直接显示，更多条目提供展开与受限滚动；每条单行摘要，右侧保留“立即发送”、删除、更多（含编辑），减少常驻大按钮。空队列不占高度。Enter 排队，⌘Enter 执行用户要求的打断并发送；明确区别于仅在安全边界注入的 steer。编辑、排序、执行和取消语义沿用 research-and-spec.md，视觉上收敛为紧凑样式。
