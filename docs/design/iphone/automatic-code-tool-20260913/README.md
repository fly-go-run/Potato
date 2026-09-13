# iOS 模型自主代码工具

2026-09-13，按用户明确要求修复 iOS 云端对话调用链。此前 E2B 只在手动代码块运行和历史检索内部使用，模型没有独立的代码执行工具；因此模型说自己只有联网搜索，并不意味着 E2B 服务没有部署。

## 当前行为

- `run_python` 与 `web_search`、已开启的历史/记忆工具一起提供给模型。模型可选择不调用、单独调用或组合调用，执行结果返回模型继续回答。
- iOS 云端账号的正常对话自动启用 Python，显示运行状态、代码与日志，并保存和预览产物；停止、失败、重启恢复和回复版本均覆盖。
- 历史检索继续遵守「记忆与历史」开关及允许范围；运行 Python 不会绕过历史授权或直接挂载账号历史数据库。第三方自定义模型地址不附加专有 sandbox 请求。
- 最新一条含附件消息中的文件可作为输入，最多四份、总计约 2 MB；超预算文件不阻断普通对话，会明确告知模型哪些文件未提供。
- 每次执行是独立 E2B 环境，无互联网；每轮最多三次 Python，单次运行最多 60 秒。错误可由模型修正后重试，每次发送完整脚本；不承诺跨调用保留变量或文件。
- 旧版 iOS 未发送 sandbox 能力标志时不启用新事件，避免破坏旧客户端流解析。全新安装仍展示本地周末计划示例，覆盖更新恢复数据。

## 验证

- Worker 60 项测试通过，TypeScript 检查及 Wrangler dry-run 通过。覆盖平级工具、按需调用、历史与代码组合、文件预算、错误重试、调用次数、取消及旧客户端兼容。
- iOS 118 项单元测试通过；已有两项 Recall UI 测试通过；新增两项代码执行 UI 测试在修正测试按钮定位后重跑通过。初次测试日志含定位失败，最终依据 `ios-ui-final.log`，并未将第一次整体失败写成通过。
- UI 验证运行状态 → 文件 → 重启 → 实际预览，以及运行中停止；截图已目视检查，见 `screenshots/`。尚未声称在用户 iPhone 真机完成验收。
- 线上真实 DeepSeek 自主选择两次 Python：读取合成 CSV，再计算 17+19+23=59 并生成 PNG 和 PDF。输出文件签名、流完成及临时账号会话撤销通过，详见 [online-check.json](online-check.json)。未上传用户真实文件或历史。
- Release 0.2.2 (2026091306) 归档、签名、隐私清单、iPhone-only 与调试夹具排除检查通过，见 [release-check.json](release-check.json)。源码快照见 `source-sha256.json`。

## TestFlight 发布

0.2.2 (2026091306) 于 20:41 通过 Xcode Organizer 上传 Apple，处理完成，已加入既有「个人内测」组，状态 **Testing**、有效期 90 天、1 位现有测试者。中文说明已保存，用户可在 TestFlight → Potato Remote → 更新。Apple 构建 ID `3f529c5b-fee8-469a-bbeb-1bfda95f4bdf`。

## 云端部署

- 已部署 Worker `1a91e9e5-9a5f-4659-9cc4-01e2f9925eb2`；域名 `potato-remote.recodex.top`。
- 上一版可回滚版本 `1e5b77af-be4e-4fd1-abf1-8b52ce3e4f63`（已发布的历史记忆版本）。部署使用 `--keep-vars` 保留现有配置。
- iOS 归档：`native/potato-ios/build/distribution/PotatoMobile-0.2.2-2026091306.xcarchive`。TestFlight 最终状态见 `distribution.json`。
