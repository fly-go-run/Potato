# 远程对话改版 · 0.2.2 (2026091608)

**0.2.2 (2026091608) 已发布至既有 TestFlight「个人内测」组**。2026-09-16T23:27:51+08:00 回读确认 `IN_BETA_TESTING`、`testing: true`、中文说明一致。可在 TestFlight → Potato Remote → 更新。

## 构建内容

- 远程回复按轮整合，正文连续阅读，思考与工具收至同一个过程入口；完整记录可展开。
- 复制、选择和分享针对整轮正文，只显示一组回复操作。
- 顶部标题与电脑信息合并，输入区尺寸、玻璃效果与普通对话一致，复用原生 ComposerTextInput。
- 点击/拖动收键盘保留草稿；大字号断线提示增加独立可读背景。
- 按用户补充意见移除“本轮任务已完成”和就绪回执，仅保留运行、等待、停止与异常状态。

## 范围与验证

与已发布 2026091607 的 129 个输入摘要对比，仅 RemoteMessageView.swift、RemoteView.swift 及 3 个相关测试文件变化；构建号同步修改为 2026091608。其他已发布 iOS 功能保持当前内容，未随同部署桌面或 Worker。[变更文件](changes-from-2026091607.json)。

- 全量原生 iOS 单元测试 **167 通过，0 失败、0 跳过**，见 [测试摘要](unit-tests.json)。
- 复用同一功能源码的最终亮色 3 条及暗色 3 条 UI 回归，包含完整回复、过程详情、键盘草稿、大字号断线、状态转换和审批停止。导航与重启草稿亦通过，见 [实现与截图](../README.md)。
- 真机 Release 签名归档成功，上传前重新计算 [129 个源码输入摘要](source-sha256.json) 全部一致。
- 归档：`native/potato-ios/build/distribution/PotatoMobile-0.2.2-2026091608.xcarchive`。
- 单元测试结果包：`/tmp/PotatoRelease2026091608Unit.xcresult`。
- 归档/测试/上传主日志：`/tmp/potato-release-2026091608-{archive,tests,upload}.log`。导出上传详细日志位于归档旁 `.xcarchive.export/upload.log`。

本轮不创建测试组、邀请新测试者或提交外部/App Store 审核。真实 iPhone 操作仍需安装后验收。

## Apple 状态

Apple 构建 ID：`0dec6f88-dec8-47a7-b723-10ba80a525b8`。状态 `VALID`、`IN_BETA_TESTING`、`inGroup: true`、`testing: true`、`notesVerified: true`。见 [分发回读](distribution.json)、[上传回执](upload.json)和[中文说明](notes.txt)。
