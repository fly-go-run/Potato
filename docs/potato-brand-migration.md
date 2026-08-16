# Potato 品牌迁移审计

## 审计基线

迁移前对 Git 跟踪的文本与路径进行了全库扫描：

- 1,094 个文本文件包含 `QwenPaw/qwenpaw/QWENPAW`，约 15,040 处命中。
- 951 个跟踪路径包含 `qwenpaw`，其中 823 个位于原 Python 包 `src/qwenpaw`。
- 扫描范围覆盖 Python、前后端、Tauri、插件、安装脚本、CI/CD、Docker、网站、测试、文档和静态资源。

## 已迁移为 Potato

| 区域 | 当前规范 |
| --- | --- |
| Python 发行包与源码 | PyPI 发行名保持 `qwenpaw`；实现包为 `src/potato`，主入口为 `python -m potato` |
| CLI | 主命令 `potato`；`qwenpaw`、`copaw` 仅作为兼容入口 |
| Python 环境变量 | `POTATO_*` |
| 用户数据目录 | 新安装默认 `~/.potato` |
| 消息元数据 | `potato_tag`、`potato_turn_usage` |
| 本地模型 Provider | `potato-local` |
| 插件版本字段与 entry point | `potato_version`、`potato.doctor` |
| 浏览器存储 | `potato_*`、`potato.*` |
| 桌面端 | `potato-desktop`、`potato-backend`；bundle identifier 保持旧值以支持原位升级 |
| Pet 插件 | `potato-pet`、`potato_pet_desktop` |
| 构建与发布 | wheel 与 Docker 的外部发行身份保持兼容；安装器、可执行文件和新更新元数据使用 Potato 名称 |
| 网站与文档 | Potato 名称、仓库链接、页面/博客 slug、品牌素材名 |
| GitHub 仓库链接 | `https://github.com/fly-go-run/Potato` |

## 升级兼容策略

新版本统一写入 Potato 名称，同时继续读取以下旧数据，避免升级后配置、密钥或运行状态“消失”：

- Python import/CLI：`qwenpaw` namespace、`python -m qwenpaw`、`qwenpaw` 命令。
- 环境变量：`QWENPAW_*`，以及更早的 `COPAW_*`。
- 工作目录：已有 `~/.qwenpaw` 或 `~/.copaw` 会被复用；显式 `POTATO_*` 优先。
- 系统钥匙串：按 `potato`、`qwenpaw`、`copaw` 顺序读取。
- Provider 与插件：旧 `qwenpaw-local`、旧插件版本字段、旧 doctor entry point。
- 备份：接受旧 `qwenpaw_version` 字段，并可验证旧字段参与计算的签名。
- 前端状态：迁移旧 localStorage/sessionStorage key 与项目会话前缀。
- 项目运行状态：读取旧 `.qwenpaw`/`.copaw` 的 policy、loop state、fork worktree、registry、scope 和集成项目指针。
- 桌面/Pet：识别旧工作目录、日志、偏好和 Pet home。
- 桌面更新：新旧 manifest URL 发布同一份签名制品；保留旧 bundle identifier 作为稳定安装身份。
- 进程与卸载：更新/关停时识别 `potato`、`qwenpaw`、`copaw` 三代 CLI 包装进程；卸载时清理三代安装器写入的 PATH 标记。
- Python 更新：继续发布到 PyPI 的 `qwenpaw` 项目，安装后提供 `potato` 主命令和 `potato` Python 包。
- Docker 更新：继续发布 `agentscope/qwenpaw` 镜像；Compose 的 service/container 与卷身份保持 `qwenpaw`/`qwenpaw-*`，避免原位升级时遗留旧容器、端口冲突或已有数据不可见。
- CloudPaw/iac-code：继续写入 iac-code 已发布的 `llm_source: qwenpaw` 协议值，并把 Potato 当前 `SECRET_DIR` 通过其兼容环境键传给子进程。

## 刻意保留的旧名称

以下命中不应机械替换：

- `QwenPaw-Flash`：这是已发布的外部模型系列及 ModelScope/Hugging Face 仓库 ID；改名会使下载地址失效。
- `AgentScope/QwenPaw`：现有 ModelScope Studio 外部目标，确认新 Studio 存在前不能替换。
- `src/qwenpaw`、`QWENPAW_*`、`.qwenpaw`、`qwenpaw_version` 等：只出现在兼容层、迁移逻辑和相应测试中。
- `io.agentscope.qwenpaw.desktop` 与 `qwenpaw-tauri-latest.json`：已安装桌面客户端无法修改的稳定身份/更新桥，不是用户可见品牌。
- PyPI `qwenpaw`、Docker `agentscope/qwenpaw`、`qwenpaw.agentscope.io`/`download.qwenpaw.agentscope.io`、OSS bucket `qwenpaw-download`，以及 Compose 的 service/container/卷身份：已经存在的外部发布和运行身份；切换会导致包名冲突、链接或下载失败、旧容器遗留或已有数据不可见。
- 历史 release notes、评审记录、参考截图路径：保留当时名称以免篡改历史证据。
- `LICENSE` 中的旧作者归属：保留原始版权署名。

## 发布前需要外部确认

以下新的 Potato 基础设施尚未启用，创建并验证前不能切换：

- `potato.agentscope.io` 产品站点与 `download.potato.agentscope.io` 下载域名。
- 当前更新存储中需要新增并验证 `potato-tauri-latest.json`；旧 `qwenpaw-tauri-latest.json` 必须继续保留。
- 首个 Potato bridge release 必须同时上传 `potato-tauri-latest.json` 与 `qwenpaw-tauri-latest.json`，并用现有 updater 私钥签名。
- ModelScope Studio 是否要继续使用 `AgentScope/QwenPaw`，或另建 Potato 目标后再切换。

本次已核验并决定不切换：PyPI 的 `potato` 名称已属于无关项目；Docker Hub 尚无 `agentscope/potato`；`potato.agentscope.io` 与 `download.potato.agentscope.io` 当前均不可解析。因此它们不作为本次迁移的发布目标。

另外，当前源码版本为 `2.0.5`，而 PyPI 的 `qwenpaw` 已发布到 `2.1.0`。正式发布 Potato 兼容版本前必须先选择一个更高且未占用的版本号；本次审计不擅自决定发布版本。

## 本机生成物

旧 `.venv`、`build/`、`dist/`、Tauri `target/`、PyInstaller 目录及历史截图中仍可能存在 QwenPaw 文件名。这些不是当前源码，且可能包含用户保留的历史制品，因此本次没有批量删除。干净环境重新安装/构建后，Python wheel 的发行文件名仍以 `qwenpaw` 开头（外部兼容身份），实现仅包含 `potato/**` 与 `qwenpaw/__init__.py`、`qwenpaw/__main__.py` 两个兼容文件，不应包含旧实现或 `.pyc`。

## 本次验证

- Python 品牌迁移、备份、密钥、provider、fork、policy 和 loop 定向测试通过。
- App 测试 279 个通过，Console 测试 12 个通过。
- App、Console/Tauri bootstrap、Website 生产构建通过。
- Rust `cargo check` 通过，库测试 21 个通过。
- `potato` 与兼容的 `qwenpaw` CLI 冒烟测试通过。
- Python 语法扫描 700 个文件通过；安装脚本 shell 语法检查通过。
- `qwenpaw-2.0.5` sdist/wheel 已完成本地构建检查：metadata 发行名为 `qwenpaw`，主实现为 `potato/**`，同时提供 `potato`、`qwenpaw`、`copaw` 三个 CLI；旧 namespace 仅有两个兼容文件，且归档内无 `.pyc`。
- `potato update` 已固定查询和升级 PyPI 的 `qwenpaw` 发行项目；相关更新、安装、兼容 namespace、旧进程关停、卸载 PATH、插件目录与 CloudPaw 外部协议定向测试 100 个通过。
- 完整 Python 集成套件的沙箱运行有 5,934 个测试通过，但 localhost 绑定被沙箱禁止；允许绑定端口后补跑到 6% 时有 452 个通过、6 个 ACP/cron workspace-reload 时序失败，因全套预计超过一小时而中止。两个代表失败独立复跑仍表现为 cron 在 workspace reload 时被取消，未发现与品牌字段或路径有关的调用栈。
