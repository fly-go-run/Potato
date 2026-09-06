# Potato GPUI 原生客户端

当前 Rust 前端迁移方向：GPUI Kit 0.6.0（gpui-component）+ 进程内 potato-core。
不启动 Python 服务或 WebView。原 React/Tauri 客户端作为视觉参照保留，Iced 版本作为迁移参考保留。

设计以 `app/src/styles/tokens.css` 为准：中性灰、系统字体、浅色侧栏、轻边框、统一圆角与克制阴影。
界面图标取自原版 Lucide，macOS 包使用原来的 `console/src-tauri/icons/icon.icns`。

## 构建与运行

需要 Rust 1.96.1 和 Python 3.11+（仅打包时使用）。macOS 需要 15+、完整 Xcode 和 Metal 编译工具；
Windows 需要 MSVC C++ Build Tools、Windows SDK（含 `fxc.exe`）和 NSIS。
GitHub Actions 会准备这些工具，不需要本地 Windows 机器。

```sh
cargo +1.96.1 test --locked --manifest-path native/potato-gpui/Cargo.toml
cargo +1.96.1 build --locked --release --manifest-path native/potato-gpui/Cargo.toml
python3 native/potato-gpui/package.py
```

产物位于 `native/potato-gpui/dist/`。macOS 生成 `.app`、可拖入 Applications 的 DMG 和 ZIP；
Windows 生成当前用户安装 EXE（开始菜单、桌面快捷方式、系统卸载入口）和便携 ZIP。
安装器不需要管理员权限；卸载保留用户数据。每个下载文件附带 SHA-256 校验文件。
macOS 仅 ad-hoc 签名，未做 Apple 公证；Windows 未做代码签名，系统可能显示信任提示。
Linux 打包尚未接入。Windows 真机的 GPU、输入法、麦克风仍需验收。

## 从 GitHub Actions 获取安装包

使用 [Native GPUI Build and Release](../../.github/workflows/gpui-release.yml)，
不要使用旧 Tauri 的 `Desktop Build` 或 Iced 的 `Rust UI preview`。

1. 将完整的 GPUI 源码、`Cargo.lock`、依赖的 `potato-core` 改动，以及本工作流提交并推送到 GitHub。
   GPUI 还引用 `native/potato-ui/src/stream.rs` 和仓库里的提示词资源，不能只上传本目录。
   新工作流需先进入默认分支，才能在 Actions 页面出现手动运行入口。
   首次验证也可以推送到 `codex/native-ci/...` 分支，直接触发构建，无需先合入默认分支。
2. 在 Actions → **Native GPUI Build and Release** → **Run workflow** 选择构建分支。
3. 在该次运行的 **Artifacts** 中下载对应文件。GitHub 会在实际安装包外再套一层 artifact ZIP，先解压它。

| Artifact | 实际安装/使用文件 |
| --- | --- |
| `Potato-GPUI-Windows-x86_64` | `Potato-GPUI-0.1.0-Windows-x86_64-setup.exe`；另含 `-portable.zip` |
| `Potato-GPUI-macOS-arm64` | Apple Silicon 的 `Potato-GPUI-0.1.0-macOS-arm64.dmg`；另含 ZIP |
| `Potato-GPUI-macOS-x86_64` | Intel 的 `Potato-GPUI-0.1.0-macOS-x86_64.dmg`；另含 ZIP |

表中版本来自本目录 `Cargo.toml`。手动运行、`codex/native-ci/...` 分支推送和相关 PR
都构建三个平台并保留安装包 30 天，
不会创建 Release。PR 还会针对共享核心、流适配器、提示词与图标变化触发。

CI 使用明确的 runner：`windows-2022`、`macos-15`（ARM64）、`macos-15-intel`；
固定 Rust 1.96.1，并用 `--locked` 构建。Windows 显式查找 GPUI 的 release 着色器编译器，
静态链接 CRT，并检查是否意外依赖 Visual C++ Redistributable DLL。

检查包括 Clippy、核心测试、GPUI 测试、release 编译、校验和和最终包验证：

- macOS：解压 ZIP、挂载 DMG，检查架构和签名，并分别启动包内程序初始化 SQLite/读取设置。
- Windows：解压便携包并启动，静默安装后启动，覆盖升级后再次启动，然后卸载并检查注册表清理及数据保留。
- 启动探针使用独立临时数据目录和工作目录，不需要服务端、模型密钥或显示器。
  这是核心启动验收，不代表 GPU 渲染、真实输入法和麦克风已经通过验收。
- 运行失败可下载 `GPUI-verification-*` 日志（保留 14 天）。检查失败的平台不上传可分发安装包。

本地复跑打包验收（以 macOS ARM64 为例，Python 必须是 3.11+）：

```sh
python3 native/potato-gpui/verify_package.py --target aarch64-apple-darwin
```

Windows 的 `verify_package.py` 会实际安装/卸载，仅应在干净的 CI 或测试机运行；发现已有安装会拒绝覆盖。

## 发布到 GitHub Releases

将 `Cargo.toml` 的版本与 `Cargo.lock` 更新后，为对应提交创建 `native-v<版本>` 标签并推送，例如
`native-v0.1.0`。标签版本必须精确匹配 `Cargo.toml`；三个平台全部通过后，工作流校验六个下载文件及校验和，
上传到同名 Release 草稿，最后发布为 **Prerelease**，不设为仓库 Latest。
同一标签重跑可以补齐草稿，但不能覆盖已经公开发布的版本。

这是一条独立的 GPUI 预览发布通道，不依赖 Tauri 更新签名或 OSS 密钥。
现有正式 Tauri 发布入口和更新源仍保留，GPUI 尚未提供自动更新。

默认使用 `~/.potato/native-v1`：沿用 Potato 的 `~/.potato` 数据入口，Rust 数据库、加密密钥和工作区放在其子目录。
安装到 Applications 或直接运行使用同一目录。此前预览版的 Application Support 目录不会自动搬迁。
可通过 `POTATO_NATIVE_DATA_DIR` 指定独立验收目录。不会自动导入原 Tauri 的个人配置；设置 → 数据中显式导入。
同一数据目录只运行一个原生客户端，避免多个调度器同时执行任务。

## 实现与验收边界

已接入真实后端：聊天流式输出与停止、附件、项目与权限选择、模型选择、会话管理、Markdown、逐次审批与提问、语音输入、任务模板及编辑、技能与记忆文档、七组设置页面。
已有草稿保护、过期文档保存冲突拒绝、单次任务类型保留、连接测试与保存分离。

这仍是迁移中的客户端，不能将“能编译”当作与原版完全一致。
详细已验收页面与剩余差异见 [REVIEW.md](REVIEW.md)。

## 体验检查补充

本轮修复与功能差距见 [2026-09-06 体验检查](UX_AUDIT_2026-09-06.md)。包含实际窗口截图、草稿/编辑器状态回归、深色对比及设置保存问题。
