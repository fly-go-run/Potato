# 快捷附件面板 · 0.2.2（2026091901）

2026-09-19：云端已部署；iOS **0.2.2（2026091901）** 已分发至既有 TestFlight「个人内测」组，确认 `IN_BETA_TESTING`、`testing: true`，中文更新说明读回一致。Apple 构建 ID `ac8eea2a-1380-4114-8f61-82161ee4e1e1`。可在 TestFlight → Potato Remote → 更新。

新增最近照片快捷添加面板，照片和文件共用 20 个名额，移除底部操作说明。与上一已发布 iOS 构建 2026091703 的输入摘要比较，本版新增/变更均为附件面板、上限、权限说明、相关测试与项目构建配置；原有未提交功能保持此前发布状态。

云端基于已上线版本 8d8d69b8-6452-4c49-9ae9-4173cea1c8c0 的源码摘要隔离构建，只调整 src/index.ts 图片数量校验。新版 9e1a65a0-89d2-4c81-baaf-16e718fbc780 回读确认 100% 流量。没有发布工作树中尚未上线的远程排队改动，不变更线上绑定、密钥与迁移。

## 验证与产物

- 196 项 iOS 全量单元测试通过；本轮 4 项附件 UI 场景分批通过。
- 隔离 Worker 92 项回归、TypeScript 检查和 dry run 通过；线上 /health 正常。
- 自动审批拒绝额外真实模型探针读取并解密本地 QA 会话凭据，因此该探针未执行。20 张完整转发及 21 张拒绝由接口回归覆盖，没有声称真实模型联调通过。
- Release 真机签名归档成功，版本、iPhone-only、照片/相机用途说明及隐私清单已核对。
- 归档：`native/potato-ios/build/distribution/PotatoMobile-0.2.2-2026091901.xcarchive`。
- iOS 日志：`/tmp/potato-release-2026091901-tests.log`、`/tmp/potato-release-2026091901-archive.log`、`/tmp/potato-release-2026091901-upload.log`。
- Worker 日志：`/tmp/potato-attachments-worker-release-tests.log`、`/tmp/potato-attachments-worker-dry-run.log`、`/tmp/potato-attachments-worker-deploy.log`。

[源码摘要](source-sha256.json) · [相对上一构建变化](changes-from-2026091703.json) · [归档检查](archive-check.json) · [测试摘要](tests.json) · [云端部署](worker-deployment.json)

[内测分发回读](distribution.json) · [上传结果](upload.json) · [中文更新说明](notes.txt)
