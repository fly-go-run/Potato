# Rust 实际 API 配置与测试

已通过 Rust 核心的 `import_legacy_settings` 将本机旧版配置导入 GPUI 默认数据目录
`~/.potato/native-v1`（本轮从先前的 Application Support 目录迁移，旧目录保留备份）。
导入 3 个模型服务商和豆包语音配置，旧版数据保持不变；凭据由原生存储重新加密。
默认模型沿用 `sub2api / gpt-5.6-sol`，语音类型为 `doubao_asr`。

真实网络测试使用独立临时数据库、非私密短句与合成音频：

| 服务 | 结果 |
| --- | --- |
| DeepSeek Chat / deepseek-v4-flash | 通过，流式回复、历史落盘、会话回到 idle |
| DeepSeek Responses / deepseek-v4-flash | 通过，流式回复、历史落盘、会话回到 idle |
| Sub2API / gpt-5.4-mini | 通过，约 9.36 秒 |
| Sub2API / gpt-5.6-sol（默认模型） | 通过，约 3.14 秒，6 个文本 delta |
| 豆包 ASR | 通过，10 个 partial 和 final；识别出“你好，这是土豆客户端的语音输入测试。” |

导入的加密与重复执行回归测试通过。可复跑入口为
`native/potato-core/examples/service_acceptance.rs`，新增 `--import-only` 模式可单独持久化连接，
不会发起模型调用。普通运行仍使用临时数据库，并覆盖两种 DeepSeek 协议、Sub2API 和豆包。

已打开本地打包的 Potato GPUI 应用供体验。以上测试验证当前源码的 Rust 服务链路，
不代表打包应用窗口聊天、麦克风采集、输入法交互已经完成端到端验收。
