# Codex (ChatGPT 桌面 app) 设计参照采集日志

- 日期:2026-07-28
- 窗口:`window "ChatGPT" of process "ChatGPT"`,采集开始时位于 {94, 126},尺寸 {1686, 982}(注:较早前记录为 {117,33},窗口在采集前已被移动过,采集期间不再移动)
- 方法:AX 读取菜单 / AXPress 打开原生菜单 / System Events 键盘事件;每张截图用窗口精确区域(或菜单弹层精确区域),无全屏截图

## 菜单栏全量清单(AX 读取,含快捷键)

修饰键编码(AXMenuItemCmdModifiers):0=⌘,1=⇧⌘,2=⌥⌘,3=⇧⌥⌘,4=⌃⌘,8=无⌘(功能键),12=⌃⌥⌘,24=⌃⌘(fn组合),28=⌃⌥⌘(fn组合)。系统自带 Apple 菜单略(非 app 设计)。

### ChatGPT 菜单
- 关于 ChatGPT
- Settings… — ⌘,
- Check for Updates…
- Log Out
- Services(系统)
- Hide ChatGPT — ⌘H
- Hide Others — ⌥⌘H
- Show All
- Quit ChatGPT — ⌘Q

### File 菜单
- New Window — ⇧⌘N
- New Chat — ⌘N
- Open Folder… — ⌘O
- Close — ⌘W

### Edit 菜单
- Undo — ⌘Z / Redo — ⇧⌘Z
- Cut — ⌘X / Copy — ⌘C / Paste — ⌘V / Paste and Match Style — ⇧⌘V
- Delete / Select All — ⌘A
- Substitutions / Speech / 自动填充 / 开始听写 / 表情与符号(系统标准项)

### View 菜单
- Toggle Sidebar — ⌘B
- Toggle Bottom Panel — ⌘J
- Toggle Pinned Summary
- Open Terminal — ⌃⌥⌘`(mods=12)
- Toggle File Tree — ⇧⌘E
- Toggle Review Panel — ⌥⌘B
- Browser 子菜单:Open Browser Tab — ⌘T / Focus Browser Address Bar / Reload Browser Page — ⌘R
- Find — ⌘F
- Previous Chat — ⇧⌘[ / Next Chat — ⇧⌘]
- Back — ⌘[ / Forward — ⌘]
- Zoom In — ⌘+ / Zoom Out — ⌘- / Actual Size — ⌘0

### Window 菜单
- Minimize — ⌘M、Zoom、填充/居中/移动与调整大小(系统标准)、Bring All to Front
- 窗口列表:ChatGPT

### Help 菜单
- Documentation
- Keyboard Shortcuts — ⌘/
- What's New
- Troubleshooting / System Status / Send Feedback
- Start Performance Trace

## 操作日志

- AXPress File 菜单 → 菜单展开,读取弹层矩形精确截图 [04-menu-file.png]
- AXPress ChatGPT/Edit/View/Window/Help 菜单 → 各自展开态 [05-menu-chatgpt.png, 06-menu-edit.png, 07-menu-view.png, 08-menu-window.png, 09-menu-help.png]
- 截主窗口基线 → Skills(Plugins)页面,左侧栏含 New chat/Pull requests/Sites/Scheduled/Plugins + Projects 列表 + Recents 列表;侧栏一条会话悬停有 popover [10-main-window-baseline.png]
- ⌘/ → 弹出 "Keyboard shortcuts" 模态(带搜索框,分组:Chat/Navigation/Panels/Project/App/General)[11-keyboard-shortcuts-panel.png]
- 搜索框内按 PageDown/方向键 → 列表不滚动(焦点在输入框)
- Tab 移焦到列表容器(出现琥珀色 focus ring)+ PageDown×2 → 滚到底部,见 Project/App/General 组 [12b-shortcuts-tab-pagedown.png]
- PageUp×1 → 中段:Navigation 后半 + Panels 组 [12c-shortcuts-middle.png]
- 面板内快捷键补充记录:Quick chat ⌥⌘N、Archive chat ⇧⌘A、New standalone chat ⌥⌘O、Toggle pin ⌥⌘P、Next/Prev recently viewed chat ⌃Tab/⌃⇧Tab、Toggle browser panel ⇧⌘B、Toggle voice chat ⌃⇧V、Copy conversation path ⌥⇧⌘C、Copy deeplink ⌥⌘L、Copy session id ⌥⌘C、Close Tab ⌘W;深色 ⌘K 面板另见 Search files ⌘P
- Esc → 关闭快捷键模态
- ⌘K → 命令面板:"Search chats or run a command",Suggested(New chat ⌘N / Open folder ⌘O)+ Settings 区(General/Profile/Appearance/Voice/Pets/Appshots/Git/Connections/Environments/Worktrees)[13-cmdk-palette.png]
- 输入 "QwenPaw" → 过滤:出现项目 chip("1 QwenPaw""2 Q"),结果带 ⌘1/⌘2 快捷徽标 + "Loading chats…" 态 [14-cmdk-filtered.png];方向键×2 后首项高亮(实为 hover 样式)[15-cmdk-selection-moved.png]
- 面板内 Enter(列表刚重渲染时)→ 无效;稳定后 ⌘1 → 打开会话"安装 QwenPaw v2.0.1":AI 回复排版、inline code、"Worked for 3m 3s" 折叠行、右上 Outputs/Sources 卡、composer(Do anything / Approve for me / 5.6 Terra High / 麦克风 / 发送)[16-conversation-opened.png]
- PageUp×3 → 会话不同位置:完成清单、用户右侧灰气泡、"You stopped after 3m 35s" 与 "Worked for 2m 44s" 分隔行、"Searched the web" 工具行、会话顶部用户消息(链接+文字)[17/18/19-conversation-scroll.png](该会话较短,3 次已到顶)
- Tab×2×5 轮走查 → 焦点先入右侧面板:Outputs "Create a file or site" 输入框获蓝色 focus ring [20-tab-focus-3.png],Sources 链接项获蓝色圆角 focus ring [20-tab-focus-5.png];快捷键模态里的滚动容器 focus ring 为琥珀色 [12b]
- ⌘K + "Mock" → 过滤(项目 chip "1 Mock")[21-cmdk-mock-filter.png];等待加载完→ 完整结果列表:每行标题+摘要+项目徽标(chat-web-dev/vllm/blank/wpai/ChatGPT)+ ⌘1~⌘9 [22-cmdk-full-results.png]
- 两次 ⌘1/Enter 无效(按下时列表仍在加载/无键盘选中,系 hover 高亮误判);Down×1 建立键盘选中(第 2 项高亮)[24-cmdk-keyboard-selected.png] → Enter → 打开"排查 iOS 体验影响"
- 会话含:文件编辑卡 "Edited 13 files +2,178 -287"(逐文件 diff 统计、Undo/Review 按钮、Show 10 more files)、右上 Environment 面板(Changes +0 -0 / Local / main / Commit or push / Compare branch)、composer 模型显示 "5.6 Sol Medium" [25-devchat-bottom.png]
- PageUp×3 → "Worked for 55m 44s"、文件链接 chip(ChatViewModel.swift 等)、截图链接 chip、"Worked for 39m 46s"、P0 清单带 file:line 链接、顶部用户气泡 [26/27/28-devchat-scroll.png](28 已到顶)
- ⌘J → 底部面板展开:终端 tab 栏("cd" tab + 新建 +)与 shell 提示行;主区意外切到会话"查找本地 Mooncake 仓库"(顺带采到代码块卡:左上 "text" 语言标签 + 右上导出/复制图标)、composer 模型 "5.6 Terra Extra High" [29-bottom-panel.png];再按 ⌘J 关闭
- ⇧⌘E → 右侧 File Tree 面板:"Open file" tab、路径 "/"、Filter files 输入框、outputs/work 树、空态 "Select a file from the workspace tree" [31-file-tree.png];⌥⌘B Review 面板未见变化(该会话无 review 上下文);⇧⌘E 关闭
- ⌘N → 新会话空态:"What should we build?" + 4 张建议卡(Explore and understand code / Build a new feature, app, or tool / Review code and suggest changes / Fix issues and failures)+ composer 上方 "Choose project" 栏,右下按钮为声波纹(voice)图标 [32-new-chat-empty.png]
- composer 键入 → 被系统中文输入法干扰(出现候选栏,空格上屏了候选词,文字变为 "testcomposer拖油瓶state")但采到输入态:发送按钮从声波纹变为黑色↑ [33-composer-typed.png]
- Esc 取消组字 + ⌘A + Delete → composer 清空成功,按钮回到声波纹 [34-composer-cleared.png]
- ⌘, → Settings 全窗视图,General 页:Permissions(Default permissions/Auto-review/Full access 开关)、General(Default file open destination=Zed、Language、Show in menu bar、Bottom panel、Default terminal location Bottom/Right 分段控件、Prevent sleep、Speed=Standard、Imported agent setup);侧栏分组 Personal/Integrations/Coding/Archived [35-settings-general.png]
- 设置页 PageDown 不滚动;Tab×2 → 焦点入 Search settings [36-settings-tab-focus.png];输入 "theme" → 侧栏过滤出 Appearance>Theme、Keyboard shortcuts>Open the command menu、Connections>SSH authentication method 等(IME 候选栏再次出现)[37-settings-search-theme.png];Esc 清空 [38-settings-after-esc.png]
- Tab×3 → 焦点到 Appearance 侧栏项 [38-settings-tab3.png] → Space 激活 → Appearance 页:Theme 三卡(System 选中/Light/Dark)+ 主题 diff 预览代码、Light theme(Codex:Accent #0169CC、Background #FFFFFF、Foreground #0D0D0D、UI font、Code font、Translucent sidebar 开、Contrast 45)、Dark theme(GitHub)[39-settings-appearance.png]
- Tab×18 → 焦点到 System 主题卡(蓝色 focus ring)[40-appearance-tab18.png] → Right×2 → 切到 Dark:全 UI 变深色,Dark theme=GitHub(Accent #1F6FEB、Background #0D1117、Foreground #E6EDF3、UI font 变 monospace、Translucent sidebar 关、Contrast 60)[41-appearance-dark.png]
- ⌘[ 退出设置 → 深色新会话空态 [42-dark-main.png];⌘[ → 深色会话视图(代码块/inline code/Outputs Sources/composer)[43-dark-conversation.png];⌘K → 深色命令面板(多出 Search files ⌘P)[44-dark-cmdk.png],Esc 关闭
- ⌘, → 深色 Settings General 页(顺带采集)[45-settings-reopen.png];Space 误入搜索框(已清理);Shift+Tab 到 Appearance → Space → Tab×18 → Left×2 → 主题还原为 System,UI 回浅色 [48-theme-reverted.png]
- ⌘[ 退出设置 + ⌘N → 最终停在浅色新会话空态,窗口保持 {94,126} {1686,982} 未动 [49-final-state.png]

## 异常与未达成项

- codex exec 进程在开工检查时已退出,未占用焦点;全程每次键盘事件前均做 frontmost 校验,无一次失败
- 窗口开工时位于 {94,126}(任务简报写的 {117,33} 是更早的位置),采集期间未移动
- ⌘K 面板中 Enter/⌘number 在列表加载中/无键盘选中时无效(hover 高亮易误判为选中);必须先按 Down 建立键盘选中再 Enter
- "Worked for Xm" 折叠行(内含 shell 命令执行日志)未能展开:合成点击对网页内容无效,Tab 焦点走不到该行,故 shell 工具卡的展开态 及 diff 逐行渲染未采到;文件编辑卡(Edited N files)collapsed 态已采到 [25]
- ⌥⌘B Toggle Review Panel 无可见效果(当前会话无 review 上下文)
- Toggle Pinned Summary / Open Terminal(⌃⌥⌘`)未尝试(前者无快捷键且点击不可用,后者会新建终端 tab,规避)
- 系统中文输入法两次干扰键入(composer 与设置搜索),均已清理并截图确认;未发送任何消息、未新建持久实体、未改任何设置(主题切深色后已切回 System)
- PageDown/方向键在部分滚动容器无效(快捷键模态需 Tab 进容器后才可 PageDown;设置页面容器不响应 PageDown)
- Esc 不能退出 Settings,需 ⌘[(Back)
