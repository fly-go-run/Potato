# Potato Windows 命令沙箱

2026-09-10。原生 `potato-core` 的 Windows 专用依赖，无需管理员安装，不依赖已安装的 Codex CLI。

## 源码依据

参考官方 `openai/codex` 的固定提交 `2cbbf0c9b542a36a1c3284b5e804917635b6f666`，不是最新 HEAD 声明：

- [`token.rs`](https://github.com/openai/codex/blob/2cbbf0c9b542a36a1c3284b5e804917635b6f666/codex-rs/windows-sandbox-rs/src/token.rs)：受限令牌和能力 SID。
- [`legacy.rs`](https://github.com/openai/codex/blob/2cbbf0c9b542a36a1c3284b5e804917635b6f666/codex-rs/windows-sandbox-rs/src/unified_exec/backends/legacy.rs)：明确指出 `WRITE_RESTRICTED` 不保证读隔离，拒绝 deny-read。
- [`process.rs`](https://github.com/openai/codex/blob/2cbbf0c9b542a36a1c3284b5e804917635b6f666/codex-rs/windows-sandbox-rs/src/process.rs)：标准流句柄白名单、Unicode 环境、Job Object 原子挂接。
- [`elevated.rs`](https://github.com/openai/codex/blob/2cbbf0c9b542a36a1c3284b5e804917635b6f666/codex-rs/windows-sandbox-rs/src/unified_exec/backends/elevated.rs)：独立账号路线；本次没有移植账号配置、管理员服务、UAC 和防火墙设施。

Potato 参考这些编排原则，选择 **Less Privileged AppContainer（LPAC）** 作为不同的 Windows 进程后端，依照 Microsoft 的 [AppContainer 启动说明](https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer)。不宣称与 Codex 或 macOS 的权限完全等价。

## 实际权限

- 项目普通文件只读，任务 scratch 与独立 AppContainer profile 可写。选择 `workspace-write` 时，Shell 的 `effective_file_mode` 仍明确为 `read-only`。正常编辑继续使用原生文件工具；Shell 直接写项目要经过单次宿主执行审批。
- 每次创建随机 profile/SID，不复用。DACL 只给本次 SID 逐项添加读取许可和写入拒绝，不授予 Everyone 或 ALL APPLICATION PACKAGES。秘密名称（含大小写、Win32 尾随点/空格）和项目内目录拒绝逐项拒绝读写。
- LPAC 的系统基线仍可读，另外添加 `registryRead` 和 `lpacInstrumentation` 供 PowerShell 运行时及 ETW 日志初始化。不能承诺隐藏系统已向 LPAC 公开的数据。未给普通项目外文件及 Potato 私有数据授予本次 SID 的访问权；Windows 测试检查默认 ACL 下的实际拒绝。
- 默认没有网络能力。获准联网只增加 `internetClient`，不加 LAN 能力，不设 localhost 豁免。其他网络需求继续诊断，必要时审查宿主执行。
- 项目和 scratch 最多 20000 个已有条目。遇到硬链接、符号链接、junction/reparse point、不受支持的 DACL、非绝对/非 Unicode 路径或超限时停止受限启动。项目之外的显式目录拒绝尚不支持，核心也不会用宿主执行绕过这些拒绝。

## 生命周期与恢复

1. 固定探针真实检查 PowerShell 启动、项目读取、项目写入被拒绝、scratch 可写；运行超时 5 秒。失败就报告不可用。
2. 系统 API 定位 PowerShell，命令以 UTF-16 Base64 传递并配置 UTF-8 输出。显式环境白名单不继承凭据、SSH socket 或代理；`USERPROFILE` 指向任务 scratch。仅补入 AppContainer 创建所需的宿主 `LOCALAPPDATA` 路径，不因此授予该目录的文件访问权。
3. 原生 `CreateProcessW` 原子附加 LPAC、安全能力、三个标准流句柄和 Job Object，先挂起并验证实际 AppContainer token 与 `WIN://NOALLAPPPKG=1` 安全属性，再恢复线程；拒绝宿主令牌、缺失或异常属性。Job 最多 64 个进程，不允许脱离。
4. 取消、超时、父 future 丢弃及 Shell 退出均终止整个 Job，包括后代。阻塞准备结束后再次检查取消，避免取消后才运行用户指令。输出沿用核心的流式归档和大小限制。
5. 所有后代退出后，通过固定文件句柄撤回本次 SID 的 ACE，再删除 profile。保留其他任务/用户的当前 ACE；应用内锁与 Windows 命名 Mutex 串行化 Potato 实例的修改。其他宿主软件不参加这个互斥协议。任务执行期间，已有项目条目不能被删除或替换。
6. 清理同时移除 allow 和 deny ACE，不能依赖 `REVOKE_ACCESS` 删除 deny，见 [Microsoft ACCESS_MODE](https://learn.microsoft.com/en-us/windows/win32/api/accctrl/ne-accctrl-access_mode)。清理失败保留诊断、阻止盲目重放；未能确认所有后代退出时保留权限记录，关闭 Job 再次触发内核清理。

应用崩溃可能留下随机 SID 的 ACL/profile 残留；从不自动复用该身份，目前没有跨重启回收器。scratch 在整个逻辑 job 结束时删除；重试会检查并保留其中的已有结果。

独立模型审批、人工卡片、一次重试、复合命令诊断、后台续跑、三次恢复预算和取消/撤销守卫沿用核心实现。新增 Windows 启动 NTSTATUS、PowerShell 权限错误及网络拒绝识别；错误文本不能产生授权。宿主重试使用当前账号权限，不等于管理员提权。

## 验证状态

开发机是 macOS。Windows crate 与完整核心通过 `x86_64-pc-windows-gnu` 交叉编译/全目标 Clippy；不能代替 Windows 实机验收。

Windows CI 已加入真实 API 测试，不做 mock 或静默跳过：

```powershell
cargo +1.96.1 clippy --locked --manifest-path native/potato-core/windows-sandbox/Cargo.toml --all-targets -- -D warnings
cargo +1.96.1 test --locked --manifest-path native/potato-core/windows-sandbox/Cargo.toml
cargo +1.96.1 test --locked --manifest-path native/potato-core/Cargo.toml --lib windows
```

覆盖探针、只读项目、秘密/私有文件、默认禁网、联网重试保留 scratch、硬链接/junction 拒绝、恢复线程前取消、后代清理、并发任务 ACL 撤回，以及核心双流大输出归档、超时和取消。网络测试不依赖公网，不把公网连通性当成本地通过条件。CI 文件已修改，本轮未推送或触发远程 CI，Windows 运行时结果仍待执行。
