# 设置下拉关闭发布 2026091605

目标：0.2.2（2026091605），既有 TestFlight 个人内测组。已确认 Apple 状态为 **IN_BETA_TESTING**，`testing: true`，已加入既有个人内测组，中文更新说明读回一致。可在 TestFlight → Potato Remote → 更新。

## 范围与验证

对上一版 2026091604 的 127 个源码、资源及测试输入摘要进行比对，唯一业务代码差异是 `SettingsView`：移除 `interactiveDismissDisabled()`，增加 `presentationDragIndicator(.visible)`。反向替换这行后，LibraryView.swift 摘要与上一发布版本完全一致。保留工作树现有改动；本轮不部署桌面或 Worker。

- 设置页使用系统下拉关闭手势与顶部拖动条。关闭与“取消”一致，设置修改仍需点击“保存”。关闭页面时沿用已有连接测试取消逻辑。
- 155 项完整 iOS 单元测试通过，0 失败。
- 同一源码的 3 项原生设置 UI 回归通过：保存校验、取消后不保存连接配置、云端账号页面返回。
- 用户 iPhone 的下拉手势尚待更新后验收；前一轮模拟器手动操作工具返回无可用窗口，未将其算作手势通过证据。
- 源码输入：[source-sha256.json](source-sha256.json)。中文更新说明：[notes.txt](notes.txt)。

## 产物与日志

- 归档：`native/potato-ios/build/distribution/PotatoMobile-0.2.2-2026091605.xcarchive`。
- 全量单测：`/tmp/potato-release-2026091605-tests.log`。
- 设置 UI 回归：`/tmp/potato-settings-dismiss-tests.log`。
- 归档：`/tmp/potato-release-2026091605-archive.log`。
- Release 真机签名归档和上传成功。
- Apple 构建 ID：`120829d0-bb13-4127-8399-1d5e54992b6e`。
- [上传结果](upload.json) · [分发读回结果](distribution.json)。
- 上传日志：`native/potato-ios/build/distribution/PotatoMobile-0.2.2-2026091605.xcarchive.export/upload.log`。
- 上传前重新比对 127 个源码、资源与测试文件摘要全部一致。
