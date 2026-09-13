# Android 0.2.0 验收记录

日期：2026-09-13。本轮以冻结的 iOS 0.2.2（2026091304）为对照，修改范围为 `native/potato-android/`。旧版记录保留在 [parity/acceptance-0.1.0.md](parity/acceptance-0.1.0.md)，旧 `verification/` 目录属于 0.1.0，不能当作本次截图。

## 检查结果

| 检查 | 结果 | 本轮证据 |
| --- | --- | --- |
| iOS 对照 UI 测试 | 12/12 通过：模型/回复 3，文稿/附件/侧栏/语音/记忆 7，远程 2 | `parity/verification/ios-*-test-summary.txt`、`parity/baseline-ios/` |
| Android ARM64 / API 35 设备测试 | 32/32 通过，0 跳过 | `parity/verification/android15-tests.xml`、`final-tests.log` |
| JVM 协议测试 | 13/13 通过 | `parity/verification/protocol-tests.xml` |
| Release lint | 0 error、85 warnings | `parity/verification/lint-release.txt` |
| 签名与发布构建 | 通过；RSA 3072 / APK v2，沿用 0.1.0 证书 | `parity/verification/package.log` |
| 对齐/本地库检查 | zipalign 16 KB 检查通过；无 `.so` 和签名材料 | `package.py`、发布日志 |
| 真正旧版覆盖升级 | 安装 0.1.0，写入合成对话和草稿，强制停止，`install -r` 覆盖为 0.2.0；启动后两者均保留 | `parity/verification/upgrade.json`、升级前后 XML 与 `after-upgrade.png` |
| 视觉检查 | 19 张 Android 最终 UI 截图；9 组 iOS / Android 对照 | [详细比较](parity/README.md)、[并排截图](parity/comparison.html) |

Android 设备为独立 PotatoAndroid35 ARM64 模拟器，Android 15 / API 35、1080 × 2400、420 dpi。iOS 使用独立 iPhone 17 / iOS 26.3 模拟器。没有连接 OPPO Find X8s+ 真机。

## 验证内容

测试覆盖页面和状态：三层模型、同模型重选保留思考、重答取消不改草稿、版本切换和检索来源身份、文稿编辑取消/历史恢复、首次示例、设置取消、12 条历史侧栏、记忆入口、四图附件删除/发送/图库、两页 PDF 渲染、大字体长输入、单台配对移除及凭据隔离。

实际 HTTPS/WSS 夹具覆盖流式中文、取消保留部分内容、重试参数、同步完成文字及附件排除、同步版本回执和 409 冲突、关闭检索后的排除清理、语音空最终结果不抹掉部分文字且不自动发送、远程丢失回执沿用操作编号、在途新草稿不被旧回执清空、思考/工具/回复/失败状态分类。

JVM 测试覆盖 SSE 中文和 emoji、CRLF、多行帧、错误和不完整结束、请求上限、HTTPS 地址、重定向令牌隔离及实际 HTTP 分块。

PDF 测试曾误匹配到主窗口；改为等待 PDF 预览对话框后，页码、翻页和关闭断言通过。最终 32 项完整设备测试全部通过，没有删去失败断言。保留的 lint warnings 包括动态加载图标导致的未引用资源、文案国际化、依赖升级提示及触摸无障碍建议，不存在 lint error。

## 最终文件

- APK：[Potato-Android-0.2.0.apk](dist/Potato-Android-0.2.0.apk)，19,185,295 bytes。
- 应用 ID：`top.recodex.potato`；versionCode：`26091302`。
- APK SHA-256：`c2e5355d48b3bdc8e345f84e53c612fba5233779a74740a3a2096fa602a24a17`。
- 证书 SHA-256：`8d2c7407e97d4c76dcdf2967f04a2cdc111b1b16e9c960596f188de5d2655d7a`。
- `parity/verification/artifact.json` 记录本轮 APK 与主源码摘要。

只需要把 APK 传到手机安装；不需要卸载 0.1.0。同签名覆盖安装已在模拟器保留数据。签名密钥仍只在忽略的本地 `.signing/` 中，不随安装包分发。

## 边界

应用内对齐结果和系统差异见 [详细比较](parity/README.md)。字体、键盘、状态栏、系统分享/选择器和权限界面保留 Android 形态；拖动动画和原生控件度量仍有平台差异，不宣称逐像素一致。

本轮未用个人账号验收 Cloudflare 登录、真实远程电脑控制/账号撤销、豆包服务、E2B 或国内移动网络。实际 OPPO 的 ColorOS 安装检查、麦克风质量与系统朗读仍需真机确认。模拟器与合成服务通过不能代替这些结果。
