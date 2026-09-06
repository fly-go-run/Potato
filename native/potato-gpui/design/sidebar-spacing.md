# 侧栏图文间距与底部品牌

2026-09-06，根据用户提供的两张侧栏截图，并打开已安装 `/Applications/Potato.app` 实际对照。

原版 `app/src/components/layout/Sidebar.tsx` 的导航使用 16px 图标、8px 间距、15px/20px 文字、12px 外侧和按钮内侧留白。GPUI 原实现使用小按钮默认内部 4px 间距；仅修改按钮外层 gap 不会覆盖这个内部容器。

本次将五个导航入口改用明确的图标/文字行，保持原点击事件和可访问名称；采用原版尺寸，行高 36px、行间距 2px。会话标题和会话文字左侧对齐导航图标列。底部左右各保留 32px 设置与主题按钮，中间 Potato 普通文字水平居中；文字及留白不可点击。

核验：原生 Release 窗口 1080×760 实际截图 `verification/sidebar-spacing.png` 与 `verification/sidebar-spacing-hover.png`。点击 Potato 不打开设置；齿轮能打开设置，关闭后搜索入口能打开搜索框，悬浮背景仅属于对应按钮。编译、Release 构建、安装包签名和 DMG 校验通过；包内程序隔离启动检查 `ok: true`。没有为这些布局调整新增镜像实现的测试。
