# 文件与改动右侧栏

入口：聊天右上角的右侧栏图标。聊天正文中的本地文件链接也会打开右栏。

首页分三个区：

- **会话编辑记录**：成功的 write/edit/append 调用，按路径合并；数字为累计编辑片段行数，明确不代表文件净改动。详情保留逐次替换片段，写入操作不虚构旧文件内容。
- **交付文件**：显式发送或助手回复中的本地 Markdown 文件链接；去重，排除图片语法、代码块中的伪链接以及没有交付的临时写入。支持 file/sandbox URI、绝对路径，以及已知编辑文件的相对链接。
- **工作区当前改动**：独立 Git 快照，包含所有来源的修改。已暂存/未暂存分别展示；支持未跟踪、删除、二进制及无初始提交的仓库。重命名按删除/新增列出。合并冲突引导查看当前文件，不伪造普通 diff。

预览支持 Markdown、带行号和语法高亮的 UTF-8 文本/代码、PNG/JPEG/WebP/GIF。Office、PDF、其他二进制/大文件通过系统应用打开；SVG 暂作为文本源文件展示。Markdown 中的本地相对图片尚未接入内嵌加载。

文件详情有返回、文件管理器定位、系统打开、刷新；会话/交付行可定位来源消息。Git 详情可切换到文件当前内容。右栏首页/详情分别记忆宽度，窄窗口限制面板宽度以保留聊天空间，返回时保留列表滚动位置。切换会话/项目使旧后台请求失效；主题变化重新匹配代码高亮。

## 实现边界

文件和 Git 在后台读取。文本读取上限 2 MB，图片上限 20 MB，高亮上限 300 KB，最多展示 1500 行且单行限制 4000 字符。Git 查询限制输出大小和执行时间，禁用外部 diff、textconv 和 fsmonitor；没有 Git 写操作。撤销改动不在本批范围。

工作区列表在打开、手动刷新和会话结束后刷新，不运行文件系统监听。文件详情为读取时快照，可手动刷新。正在生成的普通文本不会逐 token 重新解析整段 Markdown 产物链接。

## 验证

```sh
cargo +1.96.1 test --locked --manifest-path native/potato-gpui/Cargo.toml side_panel -- --test-threads=1
cargo +1.96.1 clippy --locked --manifest-path native/potato-gpui/Cargo.toml --all-targets -- -D warnings
cargo +1.96.1 build --locked --manifest-path native/potato-gpui/Cargo.toml
```

侧栏测试覆盖：真实临时 Git 仓库中 staged/unstaged/untracked/deleted 文件及刷新时状态变化，路径/链接处理，失败调用与去重，跨用户轮次工具配对，大文件/二进制/长中文行降级，会话/项目切换，真实 GPUI 鼠标拖拽与窄窗口，以及宽度保存后重新打开数据库。

macOS 独立测试窗口已目视确认：首页三个分区、返回列表、双分区 Git diff、代码行号与高亮、Markdown 标题/列表/表格。CUA 截图存在刷新延迟，通过窗口 zoom 刷新后检查；直接坐标拖拽遇到 noWindowsAvailable，因此拖拽依据 GPUI 鼠标事件测试，不宣称桌面指针验收已完成。Windows 桌面未验收。

可通过 debug-only 的 `POTATO_GPUI_REVIEW_FIXTURE` 重放。除已有 history 外，fixture 支持 `files_open`、`project_path`，仍要求显式 `POTATO_NATIVE_DATA_DIR`。不要用个人数据目录运行 fixture。
