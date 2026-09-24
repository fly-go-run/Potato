# 0.2.2 (2026091603) 发布记录

已于 2026-09-16 14:05（北京时间）确认分发到既有 TestFlight「个人内测」组。状态 `VALID / IN_BETA_TESTING`，`testing: true`，中文说明读回一致。Apple 构建 ID `73f5b7a9-944b-4090-8aeb-db4affdb428a`。

更新入口：TestFlight → Potato Remote → 更新。

本次保留聊天主线思考摘要，点击直接查看完整记录；工具入口独立。工具详情、图片结果与文件卡片沿用本轮 Claude 参考改版。累计思考事件没有拆分成虚构时间线。

发布当前 iOS 工作树，包含此前 2026091601/1602 已上线的系统玻璃样式、侧栏和键盘交互改动。未部署 GPUI、potato-core 或 Worker。本轮 Worker 的可选动作标题和 PPTX MIME 扩展仍未发布；旧服务没有动作标题时，客户端使用兼容标题。

源码摘要见 source-sha256.json。DEBUG 展示样例不进入 Release。

- 原生测试结果：/tmp/potato-release-1603.xcresult
- 测试日志：/tmp/potato-release-1603-tests.log
- 归档日志：/tmp/potato-release-1603-archive.log
- 归档：native/potato-ios/build/distribution/PotatoMobile-0.2.2-2026091603.xcarchive
- 中文更新说明：notes.txt

## 发布验证

- 全量 iOS 单元测试 136 项通过。
- UI 回归 9 条实际执行通过：思考摘要直接打开全文、完成及重启恢复、停止及断流、搜索期间暂停与恢复、工具执行和文件恢复、PPT/图片预览、大字号、远程思考/工具/回复切换。
- Release 真机签名归档成功；仅 iPhone，隐私清单齐全，DEBUG activity-preview 入口不在可执行文件中。
- source-sha256.json 与归档时当前源码读回一致。
- 主线截图确认思考摘要独立可见，工具计数不包含思考。
