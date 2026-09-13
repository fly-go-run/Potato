# Android 0.1.0 验收记录

日期：2026-09-13。范围仅限新增 `native/potato-android`。实际设备为独立的 ARM64 Android 15 / API 35 模拟器（1080 × 2400，420 dpi），并非 OPPO 真机。

## 自动与安装验证

| 检查 | 结果 | 证据 |
| --- | --- | --- |
| JVM 协议测试 | 13/13 通过 | `verification/protocol-tests.xml` |
| Android 原生测试 | 16/16 通过 | `verification/android15-tests.xml` |
| Release lint | 0 error；保留库版本、第三方库及文案等 warnings | `verification/lint-release.txt` |
| Release APK 构建与签名 | 通过；RSA 3072 / APK Signature Scheme v2 | `verification/package.log` |
| APK 对齐、本地库检查 | zipalign 通过；没有 `.so` 文件 | `verification/package.log`、`package.py` |
| 最终 APK 安装与启动 | 通过 | `verification/reinstall.json` |
| 强制停止、同签名覆盖安装、历史保留 | 通过；本地合成会话仍在 | `verification/after-reinstall.png` |
| 视觉检查 | 首页、键盘弹出时输入区可见；最终版麦克风图标改为原生绘制 | `verification/home.png`、`verification/keyboard.png` |

协议测试包括：中文/emoji、CRLF、多行 SSE、公开思考及搜索帧、断线缺少 `[DONE]`、长度上限、错误帧、过大输入、HTTPS 路径、重定向不转发令牌、真实 HTTP 字节分段与取消。

原生测试包括：首页/侧栏、长草稿恢复和会话隔离、实际附件复制及发送内容、Keystore 加密与云端令牌地址隔离、损坏文件不覆盖、部分回复恢复、分支/文稿版本、模型目录迁移、Markdown 表格。独立 HTTPS fixture 验证了可见中文流式、停止保留部分内容和草稿、重试沿用原模型与思考档位并保留旧回复，以及远程丢失回执后沿用原操作编号、在途新增草稿不会被回执清空。

前一轮三个流式 UI 测试因测试夹具在主线程做反向 DNS 失败，移到测试线程后通过；最终完整 16 项原生回归全部通过。未通过修改 Android 网络策略或放宽生产证书校验来绕过测试。

## 最终安装文件

- `dist/Potato-Android-0.1.0.apk`，18,965,882 bytes。
- 应用 ID：`top.recodex.potato`，versionCode：`26091301`。
- APK SHA-256：`0fc4b37d0c28589742cd05ec85b48aeaeaee51300692d1d30fc766206af88ee9`。
- 签名证书 SHA-256：`8d2c7407e97d4c76dcdf2967f04a2cdc111b1b16e9c960596f188de5d2655d7a`。
- `verification/artifact.json` 还记录本轮主源码摘要；APK 和源码可对应核对。

签名材料仅存在被 gitignore 排除的本地 `.signing/`，未写入 APK 或文档。APK 未包含用户会话、供应商密钥或个人凭据；默认本地体验。正式 APK 中 `debuggable` 为默认 false，仅申请联网、按需麦克风及 AndroidX 内部非导出接收权限，无通讯录/位置/全盘存储权限。

## 尚未验证的使用环境

没有连接 OPPO Find X8s+，因此没有声称真机、ColorOS 安装扫描、麦克风采集质量、中文 TTS、国内移动网络已验收。真实 Cloudflare 账号登录、豆包服务、E2B 服务、真实远程电脑的全链路也未用个人账号测试；本轮依据已存在的 iOS/Worker 协议实现并用合成 fixture 验证传输。

没有修改线上 Worker、Cloudflare 允许名单或桌面程序，没有自动授权新邮箱。首次使用需登录已经授权的邮箱；新邮箱需由服务所有者增加授权。

与 iOS 的交互实现有明确差异：Android 使用原生对话框/系统文件预览，不提供 iOS 同款滑动侧栏和图库左右翻页；应用进入后台暂停手机侧流式生成与录音，不提供后台保活/推送。远程任务留在电脑继续执行。本地历史没有跨设备同步，卸载会删除；覆盖安装保留。
