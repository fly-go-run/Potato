# Commit 9c39a9ef 代码审查

## 高

- `app/src/stores/chat.ts:396`：SSE 非终态 EOF 被标成 `failed` 后，`finally` 立即清除 `isStreaming`，但后端明确会在断线后继续运行；用户按提示重试时 `attach_or_start` 只会附着仍在运行的旧任务并忽略新 payload，导致新消息显示在本地却从未执行（`app/src/lib/stream.test.ts:30` 只测试了 EOF 纯判断，未覆盖这一 store 生命周期）。

## 中

- `app/src/components/ui/ConfirmDialog.tsx:34`：`busy` 只禁用了两个按钮，没有阻止 Escape 或点击遮罩触发 `onOpenChange(false)`；用户确认删除后在请求完成前关闭弹窗，会看到弹窗消失、误以为操作已取消，但删除仍会在后台完成。

## 低

- `app/src/components/ui/SegmentedControl.tsx:28`：组件声明 `tablist`/`tab` 却没有 roving `tabIndex`、方向键切换或 `aria-controls`，并且主题/语言调用方本质上是单选项而非 tab；键盘或读屏用户会得到错误的控件语义，并且必须逐个 Tab 才能遍历选项。
- `app/src/views/CronsView.tsx:236`：每个 cron 开关的可访问名称都只是随状态变化的“已启用/已暂停”，没有任务名；读屏用户在多个任务之间 Tab 导航时无法区分正在操作哪一个开关。

## 复核与处理结果（2026-07-28）

1. **高：SSE 断线后重试丢 payload**
   - 复核：**成立**。后端 `src/qwenpaw/app/task_tracker.py` 的 `attach_or_start` 在同一 `run_key` 仍运行时只创建订阅队列、回放旧 buffer 并返回 `is_new_run=False`，不会调用本次传入的 `stream_fn(payload)`；`src/qwenpaw/app/routers/console.py` 的普通发送路径又没有处理该布尔值。因此运行中重复 POST 的新 payload 确会被忽略。
   - 修复：非终态 EOF 后先根据当前会话列表核对后端状态；若为 `running`，前端只发送 `reconnect:true` 自动附着旧流，不重发 payload。若重连再次断开且 history 仍为 `running`，保留 `in_progress`/`isStreaming` 运行态和停止能力；若连接已结束但 response 仍未终态，`sendMessage` 会明确拒绝新 payload 并显示“上一条消息仍在后台运行”的提示。只有明确的 `ApiError` 才将本轮标记为 `failed`。
   - 测试：扩充 `src/lib/stream.test.ts` 的 EOF/未完成状态覆盖；新增 `src/stores/chat.test.ts`，覆盖自动附着不重发 payload、重复断线保持后端运行态、未完成 response 拒绝新 payload。

2. **中：ConfirmDialog busy 可被 Esc/遮罩关闭**
   - 复核：**成立**。原实现只禁用了确认和取消按钮，Radix Content 没有拦截 Escape 或 outside interaction，Root 的 `onOpenChange(false)` 仍可关闭受控弹窗。
   - 修复：busy 时在 `onEscapeKeyDown` 与 `onInteractOutside` 调用 `preventDefault()`，并在 `onOpenChange` 入口额外拒绝关闭；未修改任何视觉 className。

3. **低：SegmentedControl 语义**
   - 复核：**成立**。原实现使用 `tablist`/`tab`，所有 button 均处于默认 Tab 顺序，且没有方向键切换；当前控件 API 是单一 value 的互斥选择模型。
   - 修复：改为 `radiogroup`/`radio` 与 `aria-checked`，选中项使用 `tabIndex=0`、其余为 `-1`；支持左右/上下方向键循环切换，并支持 Home/End。未修改任何视觉 className。

4. **低：Crons 开关 aria-label 缺任务名**
   - 复核：**成立**。原 accessible name 只有“已启用/已暂停”，同页多个开关无法区分所属任务。
   - 修复：新增中英文 `crons.toggleLabel`，将任务名和当前状态共同写入 `aria-label`。

验证：在 `app/` 执行 `npm test`，15 个测试文件、51 个测试全部通过；执行 `npm run build` 成功。`app/src/styles/tokens.css` 和所有视觉 className 均未改动。

# r5 批次审查

审查范围：`app/` 下全部 tracked diff 与 untracked 新文件；仅审查，不修改实现。父目录的 `.reference-shots/` 不在本次 `app/` 范围内。

## 中

- `app/src/stores/chat.ts:456`：新会话第一次请求尚未拿到 `activeChatId` 时点击停止，只清除 `isStreaming` 而不把 `responseStatus: "created"` 改为终态，新增的 unfinished-response 防护随后会拒绝所有发送；慢网下在会话出现在列表前点停止，会看到发送按钮恢复，但再次发送始终报“上一条消息仍在后台运行”，且该分支还遗留已上传附件预览。
  修复:无 `activeChatId` 的 stop 现会终结为 `cancelled`、撤销并清空附件预览及请求状态，且回归测试确认停止后仍可再次发送。
- `app/src/views/SkillsView.tsx:102`（结合 `app/src/lib/capabilities.ts:190`）：抽屉在 busy 时仍可关闭并打开另一技能，而单个 `busySkill` 和“整份旧 skills 数组”回滚都不是并发隔离的；先切换 A、关闭抽屉再切换 B，若 A 后失败，A 的回滚会连同 B 的 optimistic 状态一起覆盖，两个请求的 `finally` 还会互相提前清掉 busy。
  修复:busy 改为按技能名隔离的集合，optimistic 与失败回滚均只更新目标技能，A 失败和各自 finally 不再覆盖 B。
- `app/src/components/layout/ChatSearchDialog.tsx:199`：命令面板输入框把 `Input` 自带的 accent border/ring 显式清零，外层又没有 `focus-within` 替代；全局规则移除 input 后，打开 `⌘K` 或 Tab 回输入框时没有可见文本焦点。
  修复:命令面板输入框恢复 `focus-visible:border-line-strong` 的安静边框聚焦，同时继续禁用 accent ring。

## 低

- `app/src/views/ChatView.tsx:38`（结合 `app/src/components/chat/Composer.tsx:69`）：建议卡不感知 `isSubmitting`，附件上传期间空态仍可点击，草稿会被 disabled Composer 消费到局部 state，首条 user message 到达导致空态/非空态分支换树后又被丢弃；上传较慢时点任一建议卡即可触发。
  修复:建议卡现同时感知 `isSubmitting` 与 `isStreaming`，busy 时禁用点击并沿用现有减淡样式，避免草稿被换树丢失。
- `app/src/views/SkillsView.tsx:106`：optimistic 更新只写列表，抽屉使用的 `selectedSkill` 要到请求成功后才更新；在抽屉内切换时开关会保持旧值直到成功，失败时也看不到开关回滚过程，错误只出现在抽屉遮罩后的页面 Banner。
  修复:切换技能时同步乐观更新抽屉 `selectedSkill`，请求失败则按目标技能同步回滚，让 Switch 即时响应并可见回弹。
- `app/public/__qa.html:2`：dev-only QA 跳转页位于 Vite 会原样发布的 `public/`，若随本批次提交/打包，生产环境会暴露一个可由 URL 参数改写主题/语言 localStorage 的入口；文件自身也注明“发布前删除（不提交）”。

## 已验证

- **文本输入焦点**：除上述 `ChatSearchDialog` 外，全部入口都有可见表达。`LoginView`、`SettingsView`、`CronsView` 的两个 Input、Skills 搜索/Hub/URL、Sidebar 重命名和 ProjectPicker 均走 `inputClasses`；Crons prompt 与 Memory 编辑器也直接复用 `inputClasses`；Composer 由外层 `focus-within` 表达；ZipUpload 的 sr-only file input 由 label 的 `focus-within` 表达。Composer 的 hidden file input 不进入 Tab 顺序。
- **details/summary**：三个工具卡的父级 key 仍是稳定的 message id，running/quiet 两分支根节点也同为 `<details>`，没有 React key/错配问题；完成时 React 会移除 running 分支的 `open` attribute，因此当前展开状态确定会丢失并收起。这与“运行态突出、完成态安静且默认折叠”的本批设计一致，但它确实会中断正在查看明细的用户，不是状态保留实现。
- **composerDraft**：正常连点由最后一次 store 写入胜出，Composer effect 消费后立即清为 `null`；当前唯一生产者与 Composer 同处空态，正常点击后卸载不会留下 draft。busy 上传窗口的数据丢失见低严重度项；`newChat`/`openChat` 本身没有防御性清理 `composerDraft`。
- **回到底部/atBottom**：`[historyLoading, isEmpty]` 能覆盖 skeleton、空态和消息列表 DOM 的挂载切换；新 message/approval 在底部时跟随，离底时设置 `hasNewContent`；有未读新内容时流式结束 pill 仍保留，到底后由 scroll handler 清除。仅“流式中上滑后再无任何新内容”的 pill 会随流结束消失，符合 `hasNewContent` 语义。
- **命令面板**：query 变化重置索引，结果长度变化再 clamp；空结果时方向键/Enter 安全 no-op，数字只是普通查询字符；执行页面、会话或新建动作后关闭，下一次 open 会重置 query/index。未发现越界执行或旧状态复用。
- **技能抽屉**：单请求下关闭抽屉后成功/失败不会把结果写到错误技能，失败时列表 helper 会回滚；多技能并发和抽屉内 optimistic 表现分别见中、低严重度项。
- **发送按钮**：常规会话中 `isStreaming` 会同步切换 stop 的 title/方形图标，成功 stop 把 response 置为 `cancelled`，随后重新输入可发送；仅首次新会话尚无 `activeChatId` 的提前 stop 存在中严重度死锁。
- **上轮四项修复**：SSE 首次断线改为 reconnect payload 且重复断线保持后台运行态；ConfirmDialog busy 已拦截 Root close、Escape 和 outside interaction；SegmentedControl 的 radio/roving tabindex/方向键逻辑成立；cron switch accessible name 已包含任务名和状态。
- **其余改动**：Inbox 删除按钮在 hover、focus-visible 和 group-focus-within 下均可见；ShortcutsDialog 的 Esc/遮罩关闭与全局 `⌘/` 互斥打开逻辑未见状态错误；Sidebar 移除重复快捷键提示、Composer 两行起步/自动增高、accent/列宽、工具卡 quiet 展示、i18n、参考文档未发现额外正确性问题。

验证：`npm test` 通过（16 个测试文件、54 个测试）；`./node_modules/.bin/tsc -b --pretty false --noEmit` 通过；`git diff --check` 通过。未执行会产出 `dist/` 的 build，以遵守“除审查文档外不改文件”。
