# Windows 原生版检查与打包发布修复

日期：2026-09-12。范围：`potato-gpui`、`potato-core`、原生 NSIS 安装器及 `gpui-release.yml`。
在已有未提交改动上检查和修复，没有将旧 Tauri、Iced 或 Python 客户端作为验收对象。

## 已修复

| 问题 | 修复及覆盖 |
| --- | --- |
| Windows 后台 Git、电脑操作驱动、MCP、获批宿主 Shell 启动控制台子进程时可能弹出黑窗 | 对这些进程设置 `CREATE_NO_WINDOW`；已有 LPAC 启动器保持其隐藏窗口设置 |
| 文件侧栏接受 `C:relative.txt`，解析依赖进程当前盘符目录；根相对路径与 file URI 缺少 Windows 用例 | 拒绝盘符相对路径，保留明确盘符、中文和空格，覆盖 `file:///C:/...`、项目盘符根相对路径、上级目录和远程 URI 拒绝 |
| 卸载注册表命令用了无效的 `$"` 转义，NSIS 产生警告仍继续出包 | 在单引号字符串中直接使用双引号；`makensis /WX` 将警告作为错误；安装验收检查两个卸载命令的完整值 |
| NSIS 进程默认 32 位注册表视图，可能无法恢复记录在 64 位视图中的自定义安装路径 | 在 `.onInit` 设置 64 位视图后显式读取 `InstallDir` |
| 只检查主程序覆盖错误，驱动被占用时可能出现部分升级；卸载失败后仍可能移除注册表入口 | 修改前检查两个 EXE 的占用，所有 payload 写入检查错误；卸载关键文件失败时保留注册表入口，返回失败 |
| 安装验证只检查默认目录，可能覆盖注册表指向的既有自定义安装 | 验证前拒绝任何 32/64 位安装或卸载注册表项，并拒绝默认安装目录；验收使用独立中文/空格路径 |
| 主程序和驱动的最终打包依赖检查不对称 | 包验收对两者执行 `dumpbin /dependents` 并检查 VC++ 运行库依赖 |
| 发布只检查扩展名数量，无法拒绝错误版本/架构同数量文件、混入文件；上传没有下载回读 | `release.py` 严格核对六个下载文件和六个校验文件，上传草稿后下载全部资产，比对本地与远端 SHA-256 后才公开 |
| Release 查询失败可能把网络/认证错误当作不存在 | API 查询成功后才判断是否存在；失败立即停止；拒绝覆盖公开版本，重跑仅补齐草稿 |
| 当前格式不符合 CI 的 `cargo fmt --check` | 对 GPUI 当前代码运行固定工具链格式化，保留已有功能改动；格式检查已通过 |
| 原生工作流只有专用分支自动构建入口 | 增加 main/master 原生相关变动自动构建；标签才发布，普通构建仍只上传 artifacts |

## 已执行验证

- macOS 开发机：核心测试 **219 通过、11 忽略**；GPUI 测试 **65 通过**。核心首次受当前工具沙箱禁止本机监听影响，复跑允许本机 fixture 服务后全部非忽略测试通过。
- Windows 专属沙箱：`x86_64-pc-windows-msvc` 的 `clippy --all-targets -- -D warnings` 通过；3 项平台无关编码/策略测试在 macOS 通过。
- 完整 core、GPUI 及其测试目标：`x86_64-pc-windows-gnu` 的 Clippy 检查通过。使用 Zig 交叉 C 编译器及 RC 协议适配器；这属于类型/编译检查，没有链接、运行 Windows 客户端。
- macOS GPUI：debug 和 release 的 `clippy --all-targets -- -D warnings` 通过；`cargo fmt --check`、`git diff --check` 通过。
- Python 打包发布回归：8 项通过，覆盖缺失、多余、错版本、损坏内容、错误校验文件名、CRLF 校验文件、自定义已有安装、公开版本拒绝覆盖、查询失败、上传损坏不公开、草稿恢复及发布顺序。
- NSIS 3.12：用临时测试 payload 实际编译安装脚本，启用 `-WX` 后成功且无警告。该 EXE 仅用于脚本编译验收，**不是可分发的 Potato 安装包**。
- 固定 Cua 0.24.0 Windows 驱动：官方下载包 SHA-256 与脚本固定值一致；PE 架构为 x86_64，导入表未发现 VC++ Redistributable 依赖。
- `actionlint 1.7.11`：原生工作流检查通过；Python 脚本语法检查通过。

## 尚未执行

没有 Windows 主机，未执行 MSVC release 最终链接、FXC 着色器编译、Windows 安装/升级/卸载、GPU 渲染、中文输入法和麦克风实机测试。
安装生命周期和占用回归已接入 Windows CI，不能把本地 NSIS 编译或交叉 Clippy 当作这些测试已通过。

检查时 GitHub 默认分支尚没有 `gpui-release.yml`，API 返回 workflow not found。
本次没有推送工作区的大量既有改动、创建标签、公开 Release 或发送文件给联系人。
完整原生源码及这些修复进入构建分支后，按 [README](README.md) 运行原生工作流；只有真实 Windows 验证通过的包才用于分发。

## 依据

- [Microsoft：进程创建标志及 CREATE_NO_WINDOW](https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags)
- [NSIS：File 错误标志语义](https://nsis.sourceforge.io/Reference/File)、[SetOverwrite](https://nsis.sourceforge.io/Reference/SetOverwrite)
- [GitHub 官方 runner 标签](https://github.com/actions/runner-images)：`macos-15` 为 ARM64，`macos-15-intel` 为 x64；保留现有明确架构矩阵。
