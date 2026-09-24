# TestFlight 0.2.2（2026091601）

2026-09-16。**已发布至既有「个人内测」组，可在 TestFlight → Potato Remote → 更新。** App ID 6811367042，Bundle ID com.potato.iphone.prototype。北京时间 11:16:10 上传成功，11:20 确认 Testing，中文更新说明读回一致。

## 本次内容

- iOS 26+ 聊天控制层采用系统默认 Liquid Glass，去掉第二轮添加的细轮廓与投影遮罩。
- 对话正文延伸到浮动导航、输入框和底部安全区后方；最后一条消息和操作仍可完整滚动到可见区域。
- 修复侧栏展开后的直角阴影残留、关闭后的按钮响应，补充当前会话选中状态。
- 调整输入区间距、小屏大字号控件，并同步远程对话输入区材质与浮动布局。

## 源码与验证

发布当前工作树，未修改桌面或 Worker。[源码文件摘要](source-manifest.json)记录 Git 基点和构建输入 SHA-256；归档后已核对全部摘要一致。仅递增构建号，营销版本保持 0.2.2。

- 发布前全量 iOS 单元测试：129 / 129 通过，结果 `/tmp/potato-release-unit-2026091601.xcresult`，日志 `/tmp/potato-release-unit-2026091601.log`。
- 同一界面源码的 GlassChromeTests：5 / 5 通过，结果 `/tmp/potato-glass-native-default.xcresult`；另有此前阅读、输入、侧栏、语音和远程草稿回归，详见 [验证记录](../verification.md)。
- 真机 Release 归档成功，日志 `/tmp/potato-archive-2026091601.log`。Bundle ID、版本、iPhone-only、隐私清单、签名与非豁免加密声明已检查，见 [归档检查](archive-check.json)。
- 构建环境 Xcode 26.3 / iOS 26.2 SDK；界面截图来自 iOS 26.3 模拟器。未进行 iOS 27 真机、真实麦克风网络链路或完整 VoiceOver 人工验收。

## 产物与状态

归档：`native/potato-ios/build/distribution/PotatoMobile-0.2.2-2026091601.xcarchive`。

更新说明：[notes.txt](notes.txt)。上传结果：[upload.json](upload.json)。最终状态：[status.json](status.json)。分发读回结果：[distribution.json](distribution.json)。

Apple 构建 ID：`8e51c492-178b-43e8-8e94-cad9fddb6bca`。读回结果：processingState 为 VALID、internalBuildState 为 IN_BETA_TESTING、inGroup / testing / notesVerified 均为 true，未过期。

使用现有本机分发证书与描述文件，限制内部测试。没有创建测试组、邀请测试者、启用公开链接或提交外部/正式审核。
