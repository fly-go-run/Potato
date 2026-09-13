# 原生电脑操作

GPUI 通过进程内 potato-core 启动包内 `cua-driver-rs 0.24.0` 子进程。
没有 Python/Node 应用运行时，也不连接用户安装的公共 Cua daemon。

## 使用

设置 → 能力 → 电脑操作，打开“允许助手操作其他应用”。点击“检测权限”
会惰性启动私有驱动，只检查状态，不弹系统授权窗口。

macOS 请在辅助功能/屏幕录制设置中为实际运行的 Potato 应用授权；修改权限后
退出并重新打开 Potato。开发验收包身份为 `dev.potato.gpui-review`，发布包为
`dev.potato.gpui`。从终端直接启动裸二进制不能作为发布包 TCC 归属的验收证据。
Windows 显示 UIA 可用性及进程完整性等级，不会自动提权。

当前提供应用列表、无障碍树观察、点击、设值、键盘输入、按键、滚动和拖动。
操作经过 Potato 现有审批策略；观察令牌绑定会话，每次操作消耗一次，之后必须
重新观察。终端、Potato 和系统设置等受保护应用不能通过该入口操作。

本阶段截图关闭。自绘界面、没有可用 AX/UIA 快照的窗口可能无法操作；驱动报告
`ax_window_unresolved` 等降级状态时明确拒绝输入，不切换前台或猜测元素。
像素坐标以驱动窗口截图坐标系定义，尚未验收视觉坐标操作和拖动。

关闭功能会停止驱动并清空观察；正常退出也会清理驱动。取消时会终止当前电脑
操作并使已有观察失效。驱动退出、超时或结果不明时不自动重放输入。

## 开发与打包

```sh
python3.12 native/potato-gpui/stage_driver.py
cargo +1.96.1 build --locked --manifest-path native/potato-gpui/Cargo.toml
python3.12 native/potato-gpui/package.py --debug
```

debug 裸程序可从 `native/potato-gpui/target/computer-driver` 加载；安装包只从
自身目录加载。macOS 路径为 `Contents/Resources/computer-driver`，Windows 为
EXE 同级的 `computer-driver`。NSIS 安装与卸载同时处理驱动和 VERSION。

`package.py` 自动下载驱动，现有 GPUI CI 调用无需额外安装 Node。可用
`--driver-archive /path/to/official-archive` 复用本地下载，仍强制校验固定 SHA-256。
`stage_driver.py --archive ...` 也支持该方式。来源及摘要固定在 stage_driver.py。
macOS 官方 universal standalone 包的原始签名在本机验证失败，因此对已通过
官方归档摘要验证的 helper 重签名，校验后再签外层应用，遵循当前 ad-hoc 发布方式。

版本来源：[0.24.0 release](https://github.com/trycua/cua/releases/tag/cua-driver-rs-v0.24.0)。
该 monorepo 的 GitHub Pre-release 标签不表示普通 SemVer 是 nightly。
宿主规则：[官方嵌入说明](https://github.com/trycua/cua/blob/cua-driver-rs-v0.24.0/libs/cua-driver/rust/Skills/cua-driver/EMBEDDING.md)。

## 协议与回归

- `status` 返回纯文本，不能按 JSON 的 `running` 字段判断就绪；使用只读
  `call check_permissions`。macOS 参数为 `prompt:false, probe_direct_capture:false`；
  Windows 只接受空参数，返回 `uia`、`integrity_level` 等字段。
- `drag` 不携带 `snapshot_id`；`scroll.amount` 上限为 50。
- 同等可见性下优先有标题的窗口，再按前后顺序选择，避免选中无标题辅助窗口。
- AX 降级且没有快照时撤销 session，并返回明确错误。

```sh
cargo +1.96.1 test --locked --manifest-path native/potato-core/Cargo.toml --lib computer
cargo +1.96.1 test --locked --manifest-path native/potato-gpui/Cargo.toml --bin potato-gpui -- --test-threads=1
```

真实驱动测试默认 ignored，需有桌面会话和对应授权。先打开计算器可做只读检查：

```sh
POTATO_TEST_COMPUTER_DRIVER="$PWD/native/potato-gpui/target/computer-driver/cua-driver" \
POTATO_TEST_COMPUTER_APP=com.apple.calculator \
cargo +1.96.1 test --locked --manifest-path native/potato-core/Cargo.toml --lib live_driver_observation -- --ignored --nocapture
```

运行 `python3 native/potato-core/tests/fixtures/computer/build_fixture.py` 构建独立
应用，打开其输出的 `.app` 路径后再测试。

设值与点击只允许使用仓库内 `tests/fixtures/computer/Fixture.swift` 对应的
`dev.cua.native-fixture` 独立应用，同时设置 `POTATO_TEST_COMPUTER_ACTIONS=1`。
测试前应重新启动夹具，计数从 0 开始；夹具显式提供 AXWindows，避免 Stage Manager
隐藏窗口时应用默认 AXWindows 为空。不要对运行中的夹具覆盖或重签名二进制。

2026-09-09 验收：macOS ARM64 真实驱动读取计算器 149 个元素；独立夹具设值、
后台点击及两次新观察回读通过。该测试由测试进程启动驱动，不能等同于 GPUI
宿主授权的完整实机验收。GPUI 能力页已检查新增控件；打包后的隔离启动探针
确认驱动可用且版本为 0.24.0。Windows 实机和截图/视觉坐标操作尚未验收。
