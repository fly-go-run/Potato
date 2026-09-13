# 0.2.2 (2026091304) 语音启动优化发布

本版针对 iPhone 点击语音后必须等待网络连接才能开始说话的问题。麦克风启动后即可录音，内存缓冲保留连接期间的开头音频，服务就绪后顺序上传。慢连接、取消、提前结束及最终转写均有边界处理。iPhone 仍经过 Cloudflare Worker 到豆包，本次不改变中转路线。

- 与 2026091303 的生产源码差异仅为 SpeechService.swift、VoiceInput.swift、VoiceComposer.swift 和版本号。
- 15 项单元测试及 2 条语音发送/取消编辑 UI 回归通过；慢连接用例验证连接前录制 8 秒后完整上传。
- Release 归档和签名检查通过；仅 iPhone、最低 iOS 17、隐私清单及调试音频排除检查通过。
- 归档：`native/potato-ios/build/distribution/PotatoMobile-0.2.2-2026091304.xcarchive`。
- 更新说明：`native/potato-ios/testflight-voice-notes.zh-Hans.txt`。
- 发布进度：2026-09-13 17:57（北京时间）Xcode Organizer 确认上传成功；Apple 处理 Complete，已加入既有「个人内测」组，组内构建状态 **Testing**、有效期 90 天，1 位现有测试者；中文更新说明显示 Saved。Apple 构建 ID `fcbd1a04-b3d4-4627-9c26-fe7df2aac6eb`。手机可通过 TestFlight → Potato Remote → 更新。

验证见 [release-check.json](release-check.json)、[源文件指纹](release-source-sha256.json)、[测试日志](tests.log) 和 [归档日志](archive.log)。尚未验收用户真机麦克风和国内网络延迟。

[App Store Connect 构建详情](https://appstoreconnect.apple.com/teams/2b4712b4-b6bc-437e-a154-eb5d72362d9a/apps/6811367042/testflight/ios/fcbd1a04-b3d4-4627-9c26-fe7df2aac6eb)。本次未改变组成员或未来构建自动分发设置，未提交 App Store 正式发布。
