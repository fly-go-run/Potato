# 原生客户端实机与服务验收（2026-09-06）

## 已完成：真实服务

使用 `~/.potato` 与 `~/.potato.secret`，通过 Rust 核心现有导入接口导入临时加密数据库。
导入 3 个服务商和豆包连接；测试结束自动移除临时数据库。未修改旧版数据，也未输出密钥。

| 验收项 | 结果 | 范围 |
| --- | --- | --- |
| sub2api | 通过 | 使用现有 Chat Completions 配置和 gpt-5.4-mini；约 5322 ms 完成，收到 6 个文本 delta；中文回复包含“验收通过”，历史持久化且状态为 idle |
| 豆包 ASR | 通过 | 将 3.768 秒、16 kHz、单声道、16-bit PCM 合成测试音频通过真实 WSS 服务识别；收到 10 个 partial 和 final，文本为“你好，这是土豆客户端的语音输入测试。” |
| 旧连接导入 | 通过 | 3 个服务商、语音连接成功导入，原数据未变 |

可复跑入口：[service_acceptance.rs](../potato-core/examples/service_acceptance.rs)。此例子必须显式运行，
普通 `cargo test` 不会发真实服务请求。只使用非私密测试短句和音频；语音输入文件最多 30 秒。

```sh
cargo run --manifest-path native/potato-core/Cargo.toml --example service_acceptance --   "$HOME/.potato" "$HOME/.potato.secret" gpt-5.4-mini /absolute/path/speech.pcm
```

本次测试暴露并修复了真实 WSS TLS 初始化 panic：reqwest / MCP 依赖统一后同时启用 Rustls
的 ring 和 aws-lc-rs，直接 WebSocket 客户端无法自动选择。Runtime 初始化现在安装明确的
ring provider；宿主已配置 provider 时沿用宿主设置。证书与主机名校验保持启用。
新增回归测试直接构建 WebSocket 使用的默认 TLS 配置，避免仅靠本地 HTTP mock 漏测。

## 部分完成：真实输入法窗口

最新代码构建的独立 `.app` 已打开，临时数据目录与正式数据隔离。
CUA 坐标点击持续返回 `noWindowsAvailable`；后来借助新版 `Cmd+N` 对编辑框的聚焦，
成功逐键输入，而非粘贴中文或合成 IME Commit。

实际观察到：

- 当前系统为豆包输入法（com.bytedance.inputmethod.doubaoime）。逐键输入 `nihao`，
  编辑框出现带下划线的 `ni'hao` 组合文本。
- 输入法原生 AX 候选面板显示“你好、👋、ヾ(=^▽^=)ノ、你号、拟好、你还”。
- Right 将选中项从“你好”移到“👋”，Left 切回“你好”，AX selected 状态均已核对。
- 连续按 Enter 后曾观察到原始拼音 `nihao` 留在草稿；不能据此证明中文候选已上屏。
  验收应用未配置聊天模型，也不能只用“没有新会话”断言没有触发 Submit。
- Shift+Enter 后观察到多行草稿，但期间存在输入源/焦点变化，完整中文输入组合仍未验收通过。

控制工具多次报告用户焦点变化；读取系统输入源菜单超时。当前能确认组合文本及候选切换，
不能把上屏和防误发送归因或判定为应用正常/异常。需要人工保持窗口焦点，分别完成：
拼音输入、候选切换、Enter 确认不发送、Shift+Enter 换行。

## 尚未完成

- 豆包麦克风：本次服务测试使用合成 PCM，没有验证系统麦克风授权、采集或 UI 填回草稿。
- Windows：当前无已确认可访问的 Windows 测试桌面；现有仓库 Windows CI 面向 Tauri，
  不能作为本原生客户端的 Windows 或 IME 验收证据。

回归结果：57 项 UI 测试、49 项核心测试通过，两 crate 的严格 Clippy 通过。
真实服务通过不代表上述窗口与平台验收通过。
