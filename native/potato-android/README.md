# Potato Android

以 iOS 0.2.2（2026091304）的页面、功能与状态为基准逐项对齐的 Android 原生客户端。详细差异和对照证据见 [parity/README.md](parity/README.md)。使用 Java 17、Android Views 与 Android Keystore；没有 WebView、Python 运行时或 Google Play 服务依赖。沿用 iOS 的 Potato Worker、Cloudflare 设备登录、豆包语音及远程电脑协议。此目录是本次明确要求新增的移动端实现，未修改桌面 GPUI、potato-core、iOS 或线上 Worker。

## 给手机安装

安装文件：`dist/Potato-Android-0.2.0.apk`，附 `.apk.sha256` 校验文件。

1. 把 APK 传到 OPPO 手机，在文件管理中打开。
2. ColorOS 提示时，允许这次使用的文件管理器或聊天应用安装未知来源应用，按系统提示完成安装。系统可能会扫描安装包。
3. 打开 Potato → 左上角侧栏 → 设置 → 登录 Cloudflare，使用服务所有者已授权的邮箱。核对验证码，在浏览器确认后返回 Potato。
4. 也可以填写自己的完整 HTTPS Chat Completions 地址、模型名与令牌。设置中的连接测试只发送一条合成短消息，不上传个人历史。

云端模型与远程电脑分别登录。女友的邮箱如果尚未在现有服务的允许名单中，需要服务所有者先授权；安装包没有内置任何人的账号、设备会话或模型密钥。云端访问能力和网络可达性沿用现有 iOS 服务。

支持 Android 9 / API 28 及以上，target SDK 35，compile SDK 36。APK 没有本地 `.so` 库，覆盖 ARM64 等 Android 运行环境，没有第三方本地库的 16 KB 内存页兼容问题。OPPO Find X8s+ 是目标使用设备，本轮实际验证使用独立的 ARM64 Android 15 模拟器，尚未连接 OPPO 真机。

## 已接入的功能

- 与 iOS 同款首次示例文稿、简洁首页、暖色背景与原版 Potato 图标。可滑动侧栏，全部历史搜索、置顶、重命名、最近删除恢复。
- 流式正文和公开思考、Markdown 表格和代码、停止、断线部分内容保留、重启恢复；重试保留历史回复版本，较早消息可分支，换模型重答不修改输入区选择。
- 云端设备登录、自定义 HTTPS 接口、动态模型列表、服务声明的思考档位、连接测试。凭据用 Android Keystore 的 AES-GCM 密钥加密，按服务地址隔离，禁止凭据随 HTTP 重定向发送。
- 系统文件选择器、多图、JPEG 压缩和 EXIF 方向、文本/PDF 提取、原生预览/分享。最多四个附件，单文件 10 MB，图片最长 1600 px / 600 KB，完整请求最多 4 MiB。扫描 PDF 明确提示无文字。PDF 最多 200 页，提取文字最多 20 万字符。
- 聊天内工作文稿面板、展开/收起、顶部取消/保存编辑、清单勾选、版本预览与恢复、Markdown 文件分享、回复复制/文字选择/系统朗读。
- 豆包语音：Android AudioRecord 16 kHz 单声道 PCM，连接期间最多缓存 640 KB 开头音频，按序发送，60 秒上限。主聊天可编辑、发送或取消；离开前台保留可用草稿。远程语音仅转成草稿，核对后手动发送。
- Exa 自动搜索进度与来源随回复保存；Python 代码块可提交云端计算，选定附件随请求发送，结果/文件随回复保存和分享。
- 远程电脑账号关联或配对码、项目/会话列表、任务观察、模型选择、发送、单次审批、提问回答及绑定具体运行的停止。待确认指令保留目标电脑和原操作编号，重试查询原回执。
- 记忆与历史：跨对话检索开关、自动记忆、同步冲突保护、按会话排除、手工编辑/忘记记忆与来源定位。
- Android 15 全面屏状态栏/手势区避让、输入法避让、长输入展开、系统字号和按钮无障碍标签。

应用转入后台时停止录音并保留可用文字，远程电脑任务继续运行，返回前台重新读取状态。手机侧普通流式请求不会因打开分享面板主动停止，但不提供后台保活或推送。对话与附件保存在本机，卸载会删除；同签名覆盖升级保留数据。主动开启“记忆与历史”后，完成的消息文字按 iOS 同一协议同步到云端，附件、示例与排除的对话不参与。

## 构建

依赖 JDK 17、Android SDK platform 36 / build-tools 35.0.0、Gradle 8.14.3（wrapper 附有分发包 SHA-256）。

```sh
cd native/potato-android
export JAVA_HOME=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home
export ANDROID_HOME="$HOME/Library/Android/sdk"
./gradlew :app:assembleDebug :app:testDebugUnitTest
./gradlew :app:connectedDebugAndroidTest
python3 package.py
```

`package.py` 依次运行协议单元测试、Release lint、Release 构建、APK 签名验证和 zipalign 检查。首次运行生成私有 `.signing/potato-release.jks` 与权限为 600 的凭据文件；以后复用同一签名。**请离线备份整个 `.signing/` 目录，后续更新必须使用此密钥。** 密钥、构建缓存和 APK 均被本目录 `.gitignore` 排除，不得提交签名材料。分发时只需要 `.apk`，无需分享源码或密钥。

## 验证边界

本轮验证记录见 [ACCEPTANCE.md](ACCEPTANCE.md)。测试流量使用合成数据和独立 HTTPS fixture，不使用个人账号或真实电脑任务。真实 Cloudflare 邮箱登录、国内移动网络、OPPO 麦克风/系统朗读、远程电脑和 E2B 端到端仍需在使用者账号与手机上验收。应用内已对齐侧栏、模型三层面板、回复操作与版本、文稿、图库翻页、PDF 预览和语音交互。状态栏、键盘、系统文件选择器、分享、授权及系统朗读保留 Android 自身实现；模拟器验证不代表已在 OPPO 真机验证。

Android 系统 API 参考：[全面屏与窗口避让](https://developer.android.com/develop/ui/views/layout/edge-to-edge)、[Keystore 加密](https://developer.android.com/privacy-and-security/cryptography)。
