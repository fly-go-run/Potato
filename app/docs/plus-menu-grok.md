# 「+」菜单

同意 Claude 三项，减无可减。`<input type="file">` 无 accept，`addImages` 已按 `image/` 出预览，单列「上传图片」是伪分类。插件在 SkillsView 安装，`/` 弹层只滤 `skill.enabled`，菜单塞「插件」没有调用对象。截屏无 API；`onPaste` 已收剪贴板文件，不必再教。会话尚无文件时「引用」仍插入 `@`，空态文案已在 TriggerPopover，正好当教学。

`detectTrigger`（`composerTrigger.ts:15–31`）不是行首：`/` `@` 须在文首或空白后，光标停在 token 内。`a/b`、`me@qq` 不触发。已有文字时不能裸插 `/`——`hello/` 前字符非空白，弹层不出现。插入点前非空白则先补空格再写符号，插在光标处，不改原文、不整段替换。关菜单后焦点必须抢回 textarea 再 `syncTrigger`：Radix 默认还焦给「+」，textarea `onBlur` 会清 trigger。`applyTrigger` 不用改。`@` 同理。

点击直接开菜单，不藏长按/右键。本轮刚删提示行，发现性要落到「+」上；长按在桌面无惯例，右键等于没入口。上传多一击由 ChatView 拖拽和粘贴兜底。不要分裂按钮。上传列第一项，`fileInputRef.click()` 原样复用。按钮 title 从「添加附件」改成「添加」。

视觉同族不同宽：审批是 `min-w-64` 两行 hint 的档位选择，「+」是三条动作，套同一宽度会空、套节头会重。共用 `qp-pop` / raised / line / shadow-md，宽收到 `min-w-44`，单行，右侧只放弱化 `@` `/`，不要节头和副文案。
