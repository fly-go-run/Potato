# iOS 模型面板缓存与关闭交互

2026-09-14，已发布至 TestFlight：**0.2.2 (2026091401)**，既有「个人内测」组状态 **Testing**，中文说明已读回核验。

原问题：目录已持久化，但 LocalModelPicker.onAppear 无条件请求目录，同时显示加载卡片。首页叉号与已有系统下拉关闭重复。

现在启动和回到前台检查缓存，6 小时内复用，过期后台更新；面板也使用同一策略。并发读取合并为同一请求，关闭面板不取消更新；服务或凭据变化后的旧响应不会安装。缓存刷新失败保留目录；只有无缓存时展示读取状态，“更多模型”保留强制刷新。首页去掉叉号，子页保留返回与完成，并提供辅助功能 escape 操作。

## 验证

Xcode 原生构建与 iPhone 17 / iOS 26.3 模拟器：13 项 LocalModelTests、9 项 LocalModelUITests 全部通过。覆盖缓存跨重启、强制刷新、过期更新、失败保留、连接变化、启动与面板合并请求，以及重复打开、下拉关闭、草稿保留、思考档位、重新回答及大字号。

测试命令：

```sh
xcodebuild -project native/potato-ios/PotatoMobile.xcodeproj -scheme PotatoMobile -destination 'platform=iOS Simulator,id=BC20CB5A-FAC0-4A46-8E85-C58C35337B22' -derivedDataPath /tmp/potato-model-cache-build -parallel-testing-enabled NO -only-testing:PotatoMobileTests/LocalModelTests -only-testing:PotatoMobileUITests/LocalModelUITests test
```

结果：`/tmp/potato-model-cache-build/Logs/Test/Test-PotatoMobile-2026.09.14_09-13-10-+0800.xcresult`；日志：`/tmp/potato-model-cache-tests-final.log`。

[模型面板截图](model-picker.png) 已目视检查：无叉号和加载卡片，模型、思考及更多模型完整可见。

发布前全量 iOS 单元测试 129 项通过。不代表全端体验审查完成；未进行真机或 VoiceOver 人工验收。

## 发布

10:26:33（北京时间）通过 `potato-ios-release` 命令行脚本上传，Apple 处理完成后经 API 分发并核验 `IN_BETA_TESTING`。构建 ID `c9defc18-3589-4e4c-99ff-cc45ab81cc2d`，状态见 [distribution.json](distribution.json)。归档位于 `native/potato-ios/build/distribution/PotatoMobile-0.2.2-2026091401.xcarchive`，上传日志位于相邻 `.xcarchive.export/upload.log`。

首次配置完成 Developer API 密钥、本机分发证书及配套描述文件。普通发布使用手动签名和 API，无需浏览器。两次导出失败均发生在上传前，失败记录已保留；最终上传成功一次。本次仅发布 iOS。
