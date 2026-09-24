# 异步回复发布 2026091604

目标：0.2.2（2026091604），既有 TestFlight 个人内测组。已于 2026-09-16 19:21 UTC+08:00 确认 Apple 状态为 **IN_BETA_TESTING**，`testing: true`，已加入既有个人内测组，中文说明读回一致。可在 TestFlight → Potato Remote → 更新。

## 范围与验证

本次发布当前 SwiftUI 工作树：云端异步回复、发送/停止/游标恢复，以及已完成的资料库改版。先前已发布的工具过程界面继续保留。服务端同时包含工具动作标题和 PPTX MIME 支持。

- 发布前 155 项 iOS 单元测试全部通过；本轮开发另有 4 条原生聊天 UI 回归通过，资料库改版既有 8 条 UI 回归证据。
- Release 真机签名归档成功：`native/potato-ios/build/distribution/PotatoMobile-0.2.2-2026091604.xcarchive`。
- Worker 的 88 项测试、类型检查及 dry run 已通过。正式部署删除了当前免费套餐不支持的自定义 CPU 限额，保持平台默认；未升级套餐。
- 源码及测试输入摘要：[source-sha256.json](source-sha256.json)。上传前比对 127 个文件一致，保留原工作树，不提交或覆盖其他任务改动。

## 服务端已上线

Worker `potato-iphone-api`，新版本 `0a601f87-f1b3-4a0c-a0dc-365aef1ac97a` 已回读确认 100% 流量。原版本为 `1475115f-7a87-414f-b90c-0ea611047332`。

两个入口：`https://potato-remote.recodex.top`、`https://potato-iphone-api.pal-xu.workers.dev`。新增 `CHAT_JOBS`/`ChatJob` 和 `chat-job-v1` 迁移；`--keep-vars` 保留现有变量，未读取/重写 Secret 值。见 [部署记录](worker-deployment.json)。

真实云端验收使用既有 QA 账号和一条合成 DeepSeek 消息，携带当前客户端的沙箱能力字段，不发送个人聊天。客户端联调脚本第一次读取前遭遇 socket 断连；使用相同任务 ID 恢复时状态已 complete，26 个事件完整读回 `POTATO_ASYNC_OK`。同 ID 再提交仍为原已完成任务；不同内容被 409 拒绝；末尾游标读回零事件；停止先于发送时延迟请求仍为 stopped；未认证读取返回 401。见 [线上验证](worker-live-verification.json)。

这是实际云端接口断连恢复验证，加上 iOS 模拟器重启与弱网替身测试；尚未在用户 iPhone 蜂窝网络上实测。

## 客户端上传

Apple 构建 ID：`1135e5c3-a4ac-40bc-bef0-b71082240886`。更新说明：[notes.txt](notes.txt)。[上传结果](upload.json) · [分发回读结果](distribution.json)。

日志：`/tmp/potato-release-2026091604-tests.log`、`/tmp/potato-release-2026091604-archive.log`、`/tmp/potato-release-2026091604-upload.log`。归档旁的 `.upload-attempt.json` 记录唯一上传尝试。
