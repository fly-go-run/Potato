# QwenPaw 前端交互验证记录

- 验证时间：2026-07-28
- 环境：Google Chrome，`http://localhost:5174`，真实后端
- 范围：只操作验证，未修改代码；composer 全程未发送消息。
- 截图口径：Computer Use 会在操作后取得首个稳定画面，通常约在操作后 1 秒内。因此文中的“首帧”指工具取得的第一个稳定画面；短于该采样间隔的动画只能记录为“未捕捉到”，不能据此断言不存在。

## 1. 键盘可达性

开始前先聚焦地址栏并按 Esc 收起 Chrome omnibox 弹层，再从地址栏按 Tab 进入页面。连续 15 次 Tab 的结果如下。

1. 地址栏按 Tab → 焦点进入 composer 文本区；整块文本区出现连续蓝色圆角焦点环，焦点可见。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-tab2-01.jpg)
2. 再按 Tab → 焦点落在“添加附件”；按钮外出现蓝色矩形焦点环，焦点可见。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-tab2-02.jpg)
3. 再按 Tab → 焦点落在工作区选择器 `infra-learn`；控件外出现蓝色矩形焦点环，焦点可见。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-tab2-03.jpg)
4. 再按 Tab → 焦点落在模型链接 `gpt-5.6-sol`；控件外出现蓝色矩形焦点环，焦点可见。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-tab2-04.jpg)
5. 再按 Tab → 焦点落在“审批：自动”；控件外出现蓝色矩形焦点环，焦点可见。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-tab2-05.jpg)
6. 再按 Tab → 焦点离开网页，落在 Chrome 工具栏的“问问 Gemini”按钮；按钮外出现蓝色焦点环，焦点可见。页面中的禁用发送按钮未进入 Tab 顺序。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-tab2-06.jpg)
7. 再按 Tab → 焦点落在 Chrome 地址栏；地址栏出现蓝色焦点环。AX 状态未报告具体 focused element，但画面中的焦点位置可见。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-tab2-07.jpg)
8. 再按 Tab → 焦点回到网页的“新建会话”；行外出现蓝色矩形焦点环，焦点可见。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-tab2-08.jpg)
9. 再按 Tab → 焦点落在“定时任务”；行外出现蓝色矩形焦点环，焦点可见。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-tab2-09.jpg)
10. 再按 Tab → 焦点落在“收件箱”；行外出现蓝色矩形焦点环，焦点可见。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-tab2-10.jpg)
11. 再按 Tab → 焦点落在“技能与插件”；行外出现蓝色矩形焦点环，焦点可见。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-tab2-11.jpg)
12. 再按 Tab → 焦点落在“记忆”；行外出现蓝色矩形焦点环，焦点可见。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-tab2-12.jpg)
13. 再按 Tab → 焦点落在“搜索会话”；行外出现蓝色矩形焦点环，焦点可见。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-tab2-13.jpg)
14. 再按 Tab → 焦点落在会话 `Laughter`；会话行外出现蓝色圆角焦点环，焦点可见。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-tab2-14.jpg)
15. 再按 Tab → 焦点落在 `Laughter` 的“会话操作”按钮；行尾小按钮出现蓝色圆角焦点环，焦点可见。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-tab2-15.jpg)

连续 15 步中没有出现画面和 AX 状态同时无法定位焦点的步骤；第 7 步 AX 未给出 focused element，但地址栏焦点环可见。第 6、7 步焦点进入了 Chrome 自身 UI，第 8 步才回到网页侧栏。

## 2. 侧栏

Computer Use 当前没有单独的 mouse-move 动作，本节使用“从空白处拖动并在目标上释放”让指针停在目标位置，作为 hover 操作。

- 逐个把指针停在“新建会话 / 定时任务 / 收件箱 / 技能与插件 / 记忆 / 搜索会话 / 设置” → 一次“收件箱”采样中，首个稳定画面显示浅灰色、整行圆角背景；未观察到延迟出现过程。其余逐项采样未稳定复现同样的可见背景，因此导航 hover 反馈在本次 Computer Use 采样中不一致。 [出现灰底的截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-sidebar-nav-hover.jpg)
- 逐个把指针停在 5 条可见会话行 `Laughter / 识别目录内容 / 查看工作区文件并总结 / Casual Greeting / 模型身份询问` → 5 条的首个稳定画面均未观察到行尾“更多”按钮浮现，也未观察到与导航项相同的整行 hover 背景；AX 树始终存在对应的“会话操作”弹出式按钮。 [示例截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-sidebar-conversation-hover.jpg)
- 右键会话 `Laughter` → 出现名为“会话操作”的菜单，菜单项依次为“改名 / 置顶 / 删除”；未触发删除。
- 在右键菜单点“改名” → 页面出现居中的白色模态框和整页灰色遮罩；首个稳定画面已是最终位置，未捕捉到可辨识的进场动画。输入框自动获得焦点，现有标题 `Laughter` 被全选。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-rename-modal.jpg)
- 在改名模态框按 Esc → 模态框和遮罩关闭，标题未修改。

## 3. 聊天空态与 composer

- 点“新建会话” → 主区显示居中的“今天要处理什么工作？”标题、说明文字和下方 composer；composer 为空，发送按钮为灰色禁用状态。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-new-chat-empty.jpg)
- 在 composer 输入 4 行文字但不发送 → composer 文本区域由单行高度增长到约 2 行可见高度；首个稳定画面同时显示前两行，其余行未同时显示。发送按钮从灰色禁用圆形变为黑色可用圆形，AX 状态由 `disabled` 变为可用。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-composer-multiline.jpg)
- 清空 composer → 文本区恢复空态，发送按钮恢复灰色禁用；未按 Enter、未点击发送。

## 4. 页面切换

- 点“定时任务” → 首个稳定画面直接显示标题、说明、新建任务按钮和空态卡片；未见骨架屏。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-page-crons.jpg)
- 点“收件箱” → 首个稳定画面直接显示两条结果消息；未见骨架屏。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-page-inbox.jpg)
- 点“技能与插件” → 首个稳定画面直接显示搜索框和技能列表；未见骨架屏。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-page-skills.jpg)
- 点“记忆” → 首个稳定画面直接显示“日记 / 流程 / 知识”三组条目；未见骨架屏。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-page-memory.jpg)
- 点“设置” → 首个稳定画面直接显示模型、Provider、外观等设置区；未见骨架屏。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-page-settings.jpg)
- 依次切换上述页面 → 未在首个稳定画面中观察到闪白或中间态；侧栏宽度保持不变。各页主内容容器的最大宽度和左起点不同：定时任务使用更宽容器，收件箱、记忆、设置使用较窄容器，因此切换完成时主内容会横向重排。

## 5. 技能页

- 在搜索框输入 `browser` → 首个稳定画面从 17 个技能过滤为 3 条：`browser_cdp`、`browser_visible`、`dingtalk_channel`；第三条的描述中包含 browser。未见提交按钮或加载状态。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-skills-filtered.jpg)
- 关闭 `browser_visible` → 首个稳定画面中开关值已从 on 变为 off，未见 spinner、禁用等待态或错误提示。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-skill-toggle-off.jpg)
- 再次打开 `browser_visible` → 首个稳定画面中开关值恢复为 on，未见 spinner、禁用等待态或错误提示。最终状态已恢复。
- 点 `browser_visible` 技能行 → 右侧出现固定宽度白色详情抽屉，页面其余区域覆盖半透明灰色遮罩；焦点落在抽屉关闭按钮。首个稳定画面已是最终展开位置，未捕捉到可辨识的滑入过程。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-skill-drawer.jpg)
- 抽屉打开时按 Esc → 抽屉和遮罩关闭。

## 6. 设置页主题

- 在设置页点“深色” → 首个稳定画面中设置页、侧栏和页面背景均已切换为深色，单选状态变为“深色 = 1”；未在采样画面中看到白色闪屏。 [设置页截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-dark-settings.jpg)
- 深色下点“技能与插件” → 技能列表、搜索框、卡片和侧栏保持深色，开关仍为蓝色。 [技能页截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-dark-skills.jpg)
- 深色下点“新建会话” → 聊天空态、composer 和侧栏保持深色，空 composer 的发送按钮仍为灰色禁用。 [聊天页截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-dark-chat.jpg)
- 回到设置点“浅色” → 首个稳定画面恢复浅色，单选状态为“浅色 = 1、深色 = 0、跟随系统 = 0”；语言保持“中文 = 1、English = 0”。最终状态为浅色、中文。

## 7. 收件箱与记忆详情

- 点开收件箱 `Auto-dream result` → 条目不是侧边抽屉，而是在原列表卡片内向下展开；箭头由向右变为向下/收起态，卡片高度增加并显示完整结果正文。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-inbox-detail.jpg)
- 点开记忆 `2026-07-27.md` → 右侧出现详情抽屉，主区覆盖半透明灰色遮罩；抽屉头部显示文件名、编辑按钮和关闭按钮，正文直接显示 Markdown 内容。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-memory-detail.jpg)
- 记忆详情打开时按 Esc → 抽屉和遮罩关闭，未进入编辑状态。

## 8. 约 900px 窄窗

- 将 Chrome 右侧窗口边缘拖到约 900px → 侧栏保持完整显示，主区收窄，记忆各组和条目仍保持两栏关系；未见横向滚动条。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-narrow-memory.jpg)
- 900px 下打开“定时任务” → 顶部标题和右侧新建按钮仍在同一行，空态卡片完整；未见横向滚动条或重叠。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-narrow-crons.jpg)
- 900px 下打开“收件箱” → 两条消息卡片完整，长摘要使用截断；删除按钮仍在卡片内，未见横向滚动条。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-narrow-inbox.jpg)
- 900px 下打开“技能与插件” → 搜索框、技能列表和开关均在主区内；长描述使用单行省略，未见横向滚动条。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-narrow-skills.jpg)
- 900px 下打开“设置” → Provider、模型、API key 和 Base URL 控件仍在卡片内，页面各设置区纵向排列完整；未见横向滚动条。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-narrow-settings.jpg)
- 900px 下打开聊天空态 → 标题和 composer 居中，composer 工具条保持一行，未见横向滚动条或控件重叠。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-narrow-chat.jpg)
- 完成窄窗检查后恢复 Chrome 原窗口大小。

## 可疑问题汇总

### 中

1. 地址栏开始的 Tab 路径 → 第 6 步焦点从网页 composer 工具条跳到 Chrome 的“问问 Gemini”，第 7 步到地址栏，第 8 步才回到网页侧栏。页面中的主区控件与侧栏控件之间被浏览器 UI 打断。 [第 6 步](../../../.reference-shots/qwenpaw-verify/qwenpaw-tab2-06.jpg) [第 7 步](../../../.reference-shots/qwenpaw-verify/qwenpaw-tab2-07.jpg)
2. 会话行 hover → 逐个把指针停在 5 条可见会话行，均未观察到行尾“更多”按钮浮现或导航项同类 hover 背景；但 AX 树中“会话操作”按钮存在，键盘 Tab 第 15 步也能聚焦该按钮。 [hover 示例](../../../.reference-shots/qwenpaw-verify/qwenpaw-sidebar-conversation-hover.jpg) [键盘聚焦截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-tab2-15.jpg)

### 低

1. composer 输入 4 行 → 文本区只增长到约 2 行可见高度，后两行未同时显示；发送按钮状态切换正常。 [截图](../../../.reference-shots/qwenpaw-verify/qwenpaw-composer-multiline.jpg)
2. 页面切换 → 定时任务与收件箱、技能、记忆、设置使用不同的主内容宽度和左起点，切换完成时主内容发生横向重排；未观察到骨架屏或闪白。 [定时任务](../../../.reference-shots/qwenpaw-verify/qwenpaw-page-crons.jpg) [设置](../../../.reference-shots/qwenpaw-verify/qwenpaw-page-settings.jpg)

### 未观察到

- 900px 宽度下未观察到横向滚动条、控件重叠或明显布局破坏。
- 主题切换的首个稳定画面中未观察到闪白。
- 重命名模态框、技能抽屉和记忆抽屉均可用 Esc 关闭。
- 页面首次打开的首个稳定画面中未观察到骨架屏。
