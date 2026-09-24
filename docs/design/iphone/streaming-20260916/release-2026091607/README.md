# 流式输出优化发布 2026091607

**0.2.2 (2026091607) 已发布至既有 TestFlight「个人内测」组**。2026-09-16T22:47:23+08:00 回读确认 `IN_BETA_TESTING`、`testing: true`，中文说明一致。可在 TestFlight → Potato Remote → 更新。

## 范围与验证

- 云端任务可恢复 SSE 订阅、iPhone 32 ms 平滑显示、正文/游标一致保存与 Markdown 缓存。保留 1606 的亮色/暗色/自动及设置下拉关闭。
- iOS 全量单元测试 164/164 通过；补充停止订阅和未落盘重启后，CloudReplyTests 12/12 通过。原生聊天发送/停止/重试/版本/历史 UI 回归通过。Worker 全量 91/91、类型检查和 dry run 通过。详细证据见 [实现记录](../implementation.md)。
- 真机 Release 签名归档成功：`native/potato-ios/build/distribution/PotatoMobile-0.2.2-2026091607.xcarchive`。
- [源码摘要](source-sha256.json) 129 个输入，上传前重新计算一致。本轮仅增加构建号，复用同源码的测试证据；未覆盖其他工作树修改。

## 云端上线

`potato-iphone-api` 版本 `51e229f1-6acf-4cf6-b620-98a831882d61`，已回读确认 100% 流量；前版 `0a601f87-f1b3-4a0c-a0dc-365aef1ac97a`。保留线上变量与密钥，无数据库迁移。见 [部署记录](worker-deployment.json)。

使用既有 QA 云端会话和合成中文消息测试：第一段正文在 running 状态收到，主动断开订阅后按游标 2 续接，最终 complete，256 个事件连续、413 字符完整并包含终止标记；原分页末尾返回零事件，未认证订阅返回 401。见 [线上验证](worker-live-verification.json)。一次请求的首段到达为订阅开始后 956 ms，仅记录本次联调，不代表真机或整体延迟指标。

## Apple 分发

Apple 构建 ID：`8f689613-fde0-4435-816f-9f2cd4176e6d`。已确认 `VALID`、`IN_BETA_TESTING`、`inGroup: true`、`testing: true`、`notesVerified: true`。见 [分发回读](distribution.json)、[上传结果](upload.json)及[中文说明](notes.txt)。

日志：`/tmp/potato-release-2026091607-archive.log`、`/tmp/potato-release-2026091607-upload.log`；详细上传日志在归档旁 `.xcarchive.export/upload.log`。一次性上传标记 `.xcarchive.upload-attempt.json` 防止重复上传。
