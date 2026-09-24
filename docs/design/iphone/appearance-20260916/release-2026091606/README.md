# 外观选择发布 2026091606

目标：0.2.2（2026091606），既有 TestFlight 个人内测组。Apple 已确认 **IN_BETA_TESTING**，`testing: true`，已加入既有个人内测组，中文说明读回一致。可在 TestFlight → Potato Remote → 更新。

## 范围与验证

本次新增亮色、暗色、自动三种外观，以及原生客户端主要页面的明暗配色适配。预览、保存、取消恢复及旧版设置兼容见 [功能与截图](../README.md)。当前工作树其他改动保留，桌面和 Worker 不随本次发布。

发布前比对 127 个源码、资源及测试输入，与最后通过验证的源码一致，仅随后递增构建号。复用 157 项完整 iOS 单元测试及暗色下 4 项、亮色下 2 项原生 UI 回归结果。

- 源码摘要：[source-sha256.json](source-sha256.json)。
- 更新说明：[notes.txt](notes.txt)。
- 归档：`native/potato-ios/build/distribution/PotatoMobile-0.2.2-2026091606.xcarchive`。
- 归档日志：`/tmp/potato-release-2026091606-archive.log`。
- 真机及 iOS 17 外观尚未人工验收。

## 上传与分发

Release 真机签名归档、上传均成功，安装包核对版本正确且不再包含强制亮色配置。上传前再次核对 127 个源码、资源和测试文件摘要一致。

Apple 构建 ID：`5b051e6e-bdcc-4ab0-a905-22f4d2000d7c`。

[上传结果](upload.json) · [分发读回结果](distribution.json)。上传日志：`native/potato-ios/build/distribution/PotatoMobile-0.2.2-2026091606.xcarchive.export/upload.log`。归档旁 `.upload-attempt.json` 保留唯一上传尝试记录。
