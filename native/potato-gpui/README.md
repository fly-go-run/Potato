# Potato GPUI 原生客户端

> 维护范围：本客户端与 `potato-core` 是唯一维护目标，旧实现逐步废弃。见[原生架构与退役原则](../../docs/architecture/native-only.md)。

当前 Rust 前端迁移方向：GPUI Kit 0.6.0（gpui-component）+ 进程内 potato-core。
不启动 Python 服务或 WebView。原 React/Tauri 客户端作为视觉参照保留，Iced 版本作为迁移参考保留。

设计以 `app/src/styles/tokens.css` 为准：中性灰、系统字体、浅色侧栏、轻边框、统一圆角与克制阴影。
界面图标取自原版 Lucide，macOS 包使用原来的 `console/src-tauri/icons/icon.icns`。

## 邮箱登录与云端模型

设置 → 模型与服务商 → 云端模型 → 使用邮箱登录。受邀用户只需邮箱验证码，无需 Cloudflare 账户或手填 API Key。白名单在 Cloudflare 后台随时修改，安装包不包含密钥；配置入口、协议与验证边界见[桌面云端模型说明](../../docs/architecture/cloud-desktop/README.md)。

## 构建与运行

需要 Rust 1.96.1 和 Python 3.11+（仅打包时使用）。macOS 需要 15+、完整 Xcode 和 Metal 编译工具；
Windows 需要 MSVC C++ Build Tools、Windows SDK（含 `fxc.exe`）和 NSIS，运行系统为 Windows 10/11 x86_64。
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

表中版本来自本目录 `Cargo.toml`。手动运行、`main`/`master` 或 `codex/native-ci/...` 分支推送和相关 PR
都构建三个平台并保留安装包 30 天，
不会创建 Release。PR 还会针对共享核心、流适配器、提示词与图标变化触发。

CI 使用明确的 runner：`windows-2022`、`macos-15`（ARM64）、`macos-15-intel`；
固定 Rust 1.96.1，并用 `--locked` 构建。Windows 显式查找 GPUI 的 release 着色器编译器，
静态链接 CRT，并检查是否意外依赖 Visual C++ Redistributable DLL。

检查包括 Clippy、核心测试、GPUI 测试、release 编译、校验和和最终包验证：

- macOS：解压 ZIP、挂载 DMG，检查架构和签名，并分别启动包内程序初始化 SQLite/读取设置。
- Windows：检查主程序和电脑驱动的运行库依赖，解压便携包并启动；静默安装到包含中文、空格的自定义目录后启动，检查卸载命令，验证主程序或驱动被占用时升级/卸载不修改文件；不指定目录覆盖升级后再次启动，最后卸载并检查注册表清理及数据保留。
- 启动探针使用独立临时数据目录和工作目录，不需要服务端、模型密钥或显示器。
  这是核心启动验收，不代表 GPU 渲染、真实输入法和麦克风已经通过验收。
- 运行失败可下载 `GPUI-verification-*` 日志（保留 14 天）。检查失败的平台不上传可分发安装包。

本地复跑打包验收（以 macOS ARM64 为例，Python 必须是 3.11+）：

```sh
python3 native/potato-gpui/verify_package.py --target aarch64-apple-darwin
```

Windows 的 `verify_package.py` 会实际安装/卸载，仅应在干净的 CI 或测试机运行；发现任一 32/64 位安装注册表项（含自定义路径）或默认安装目录会拒绝执行。
安装器编译把 NSIS 警告作为错误，避免生成带有无效卸载命令的安装包。

## 发布到 GitHub Releases

将 `Cargo.toml` 的版本与 `Cargo.lock` 更新后，为对应提交创建 `native-v<版本>` 标签并推送，例如
`native-v0.1.0`。标签版本必须精确匹配 `Cargo.toml`；三个平台全部通过后，工作流校验六个下载文件及校验和，
上传到同名 Release 草稿，再下载回读核对全部文件后发布为 **Prerelease**，不设为仓库 Latest。
文件名、版本、架构、数量及 SHA-256 必须全部匹配。混入旧版本、多余文件或校验失败都会停止发布。
同一标签重跑可以补齐草稿，但不能覆盖已经公开发布的版本；网络/认证错误也不会被误判为“尚未创建 Release”。

三个平台 artifact 解压到同一个干净目录后，可以先在本地检查。目录中只放六个安装/便携文件及六个 `.sha256` 文件：

```sh
python3 native/potato-gpui/release.py --packages /path/to/packages --tag native-v0.1.0
```

以上命令只检查本地文件。需要实际上传发布时，在已认证的 GitHub CLI 环境追加
`--repo fly-go-run/Potato --publish`。Actions 标签发布调用同一个脚本。
发布脚本不能替代 Windows 实机安装验证；本地手动发布只应使用该工作流验证通过的 artifacts。

跨平台打包/发布回归检查：

```sh
python3 -m unittest discover -s native/potato-gpui -p test_packaging.py -v
```

2026-09-12 的 Windows 检查、修复与验证边界见 [Windows 检查记录](WINDOWS_REVIEW.md)。

这是一条独立的 GPUI 预览发布通道，不依赖 Tauri 更新签名或 OSS 密钥。
现有正式 Tauri 发布入口和更新源仍保留，GPUI 尚未提供自动更新。

默认使用 `~/.potato/native-v1`：沿用 Potato 的 `~/.potato` 数据入口，Rust 数据库、加密密钥和工作区放在其子目录。
安装到 Applications 或直接运行使用同一目录。此前预览版的 Application Support 目录不会自动搬迁。
正常启动会自动读取旧 `.potato`/`.potato.secret` 的兼容服务商配置一次，沿用已有原生配置与可用模型选择；旧文件保持不变。模型密钥支持原有进程环境变量、原生数据目录 `.env` 和上级 `~/.potato/.env`，再回退到加密配置。没有可用本机配置时，邮箱登录后使用云端默认模型。手动选择云端模型仍然有效，本机接口请求失败不会自动把同一任务转发云端。
可通过 `POTATO_NATIVE_DATA_DIR` 指定独立验收目录，此时不自动导入个人旧配置；设置 → 数据仍可显式导入。仅 API Key 而没有可用服务地址和模型的自定义服务商不算完整配置。旧协议尚未支持或配置损坏时会提示手动导入，不阻止云端登录。
同一数据目录只运行一个原生客户端，避免多个调度器同时执行任务。

## 实现与验收边界

已接入真实后端：聊天流式输出与停止、附件、项目与权限选择、模型选择、会话管理、Markdown、模型自动审批、目录授权、逐次审批与提问、语音输入、任务模板及编辑、技能与记忆文档、七组设置页面。
已有草稿保护、过期文档保存冲突拒绝、单次任务类型保留、连接测试与保存分离。

这仍是迁移中的客户端，不能将“能编译”当作与原版完全一致。
详细已验收页面与剩余差异见 [REVIEW.md](REVIEW.md)。

## 体验检查补充

本轮修复与功能差距见 [2026-09-06 体验检查](UX_AUDIT_2026-09-06.md)。包含实际窗口截图、草稿/编辑器状态回归、深色对比及设置保存问题。

模型自动审批的设置与可复现验收见 [自动审批说明](design/auto-approval/README.md)。

macOS Shell 已接入 Seatbelt，Windows 已接入 LPAC，均沿用前后台自动审批和受限后恢复。Windows 第一版的 Shell 为项目只读、临时目录可写，需要 Shell 写项目时申请单次宿主权限；文件工具仍按原规则编辑。Windows 真实隔离测试已加入 CI，本轮尚未实机执行。范围与验证边界见 [沙箱实现记录](../../docs/architecture/sandbox-implementation.md) 和 [Windows 后端说明](../potato-core/windows-sandbox/README.md)。

联网搜索默认“自动选择”：配置 Exa 密钥时优先直接调用 Exa，其次为模型内置搜索，最后为 Tavily。显式选择的搜索方式不被覆盖。`EXA_API_KEY` 按进程环境变量、原生数据目录 `.env`、默认 `~/.potato/.env` 的顺序读取；不会读取工作项目的 `.env`。设置接口仅返回是否配置和实际搜索后端，不返回密钥。

## 技能包与 MCP 服务

侧栏「技能」支持 ZIP 技能包导入，以及 MCP 服务配置、工具发现和逐工具启停。脚本及资源的使用方式、连接生命周期和验收范围见[原生扩展说明](../../docs/design/extensions/README.md)。

## 文件与改动右侧栏

聊天右上角新增右侧栏入口，支持会话编辑记录、交付文件、独立工作区 Git 差异、文件预览、来源定位及宽度记忆。支持范围和验收记录见 [右侧栏说明](design/right-panel/README.md)。

电脑操作已接入包内 Cua Rust 驱动 0.24.0。入口为设置 → 能力 → 电脑操作。
本阶段使用无障碍树和后台输入，截图尚未接入；权限、打包与验收边界见 [电脑操作说明](COMPUTER_USE.md)。

## iPhone 远程控制

原生手机可通过中继继续桌面项目与会话、回应审批/提问并停止任务。新增 Cloudflare 同账号关联，桌面登录后需明确开启远程访问。中继与 Access 登录已部署，iPhone 已启用 TestFlight 个人内测。验证、配置与限制见 [远程控制说明](../../docs/design/iphone/remote-control/README.md)。
