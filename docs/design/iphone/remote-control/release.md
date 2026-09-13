# iPhone 远程控制个人内测发布

2026-09-12 已部署 Cloudflare 中继并通过真实账号、原生 iPhone 模拟器与隔离 Rust 核心的外网联调。Potato Remote 0.2.0 (2026091202) 已完成 TestFlight 个人内测分发，后续用户已确认真机安装并登录，本机 Mac 也已安装并关联。最新回复样式修复版为 0.2.1 (2026091203)，已进入个人内测且 Apple 后台显示已安装；蜂窝网络验收及 App Store 正式发布尚未完成。

## 已发布

- 服务：`https://potato-remote.recodex.top`，Worker 为 `potato-iphone-api`；当前版本 `69312217-b291-4870-8142-499eb836b996`。
- 原有 `potato-iphone-api.pal-xu.workers.dev` 及聊天、语音、搜索、E2B 配置保留，部署使用 `--keep-vars`，没有轮换既有 Secret。
- Access 团队 `https://liuxu-cf.cloudflareaccess.com`，应用 **Potato Remote Login**，只保护 `/v1/remote/auth/authorize`。只允许所有者邮箱，Cloudflare IdP 限定账号成员；既有 `video-restore` 应用保持不变。
- `RemoteDevice`、`RemoteAccount`、`RemoteLogin` 三个 SQLite Durable Object 已上线；两端默认远程地址已更新。

## 真实验证结果

- 新域名与原有域名 `/health` 均返回 200；未登录的账号列表、聊天接口、备用域名登录确认均返回 401；受保护登录路径正常跳转 Access。
- 电脑和 iPhone 原生模拟器分别发起登录，Chrome 使用真实 Cloudflare 身份核对验证码并确认，客户端成功关联同一账号。
- `/tmp/potato-live-account-ui-v2.xcresult`：完整远程任务测试通过，覆盖设备发现、新建任务、连续对话、回答提问、实际批准 `echo POTATO_APPROVED`、停止任务。链路为 SwiftUI → 线上 Worker/DO/WSS → 临时 Rust 核心；模型是合成服务，不代表真实模型质量或真机蜂窝网络验收。
- `/tmp/potato-live-account-cleanup.xcresult`：电脑退出后从手机设备列表消失、手机退出登录测试通过。临时宿主进程已退出，未启用日常桌面工作区的远程访问。
- Worker 27 项测试与 TypeScript 检查通过；原生核心 6 项本地测试通过。Mac release 构建、iOS 模拟器构建通过；Mac ZIP/DMG 解包、原生二进制与随包驱动自检通过。
- 线上验收截图保存在 [implemented/live](implemented/live)，文件摘要及更早本地回归结果见 [verification.json](verification.json)。

本次真实验证修复了两个问题：登录页 `no-referrer` 使浏览器表单提交的 Origin 变为 null，改用 `same-origin` 并继续拒绝空 Origin/跨站提交；长会话键盘变化触发 SwiftUI 懒布局循环，消息窗口上限 120 条，改为完整布局后全流程通过。初次失败没有计为通过证据。[浏览器来源策略依据](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Referrer-Policy)

## 安装与后续验收

Mac Apple Silicon 包在 `native/potato-gpui/dist/iphone-remote-beta-20260912/`：

- `Potato GPUI.app`
- `Potato-GPUI-0.1.0-macOS-arm64.dmg`
- `Potato-GPUI-0.1.0-macOS-arm64.zip`

包使用本地 ad-hoc 签名，未做 Developer ID 公证；没有替换 `/Applications` 中的现有应用。安装后在设置 → 能力 → iPhone 远程控制中登录，并主动开启远程访问。iPhone 使用同一账号登录即可关联。

用户可在 iPhone 安装 TestFlight 并通过邀请邮件安装本次版本，无需连接 Mac；已完成分发签名。尚待用户接受邀请、安装，验证 Wi-Fi/蜂窝网络、休眠唤醒与断线恢复。完整 VoiceOver、大字号、iOS 17 和远程语音真机录音也未验收。中继使用 HTTPS/WSS，但不是端到端加密，更多边界见 [实现说明](README.md)。

## 故障恢复

部署前版本为 `2cfc1662-2d0b-475e-8cbc-8e5a717b73d2`，配置、源码、部署记录备份在 `/tmp/potato-worker-before-remote-20260912/`，该临时路径不作为长期备份。

首次远程发布引入 Durable Object 类迁移，不能假定可直接回滚到迁移前版本。需要暂停远程入口时，将 `REMOTE_CONTROL_ENABLED` 改为 `false`，保留当前代码、DO 类和绑定重新部署；只阻止新远程请求，不会自动取消电脑已运行的任务。恢复时设回 `true`。不要运行会轮换既有客户端凭据的旧部署脚本。[Cloudflare 回滚限制](https://developers.cloudflare.com/workers/versions-and-deployments/rollbacks/)

部署前 OAuth API 返回空 Access 列表只是权限可见性结果；浏览器后来证实已有团队和应用，[预检历史记录](release-preflight.json) 已纠正这一解释。

## TestFlight 分发

2026-09-12 已创建 Potato Remote（Apple ID `6811367042`），0.2.0 (2026091202) 于北京时间 22:16 上传成功，Apple 已完成处理。个人内测组显示 1 位测试者、1 个构建，构建 **Testing**、有效期 90 天，账号所有者 **Invited**。不自动分发未来构建，未建立公开外部测试链接。安装步骤、签名产物及摘要见 [iPhone 分发状态](../../../../native/potato-ios/DISTRIBUTION.md)。

## 日常 Mac 安装与关联

2026-09-12 22:33，用户确认 iPhone 已安装并登录后，已将个人内测 Mac 包安装至 `/Applications/Potato.app`。旧应用保留在 `/Applications/Potato-before-remote-20260912-223135.app`；安装包签名校验通过，沿用原生日常数据目录，已有会话正常显示。通过浏览器核对登录码并完成 Cloudflare 账号登录，按用户远程使用要求开启远程访问。Mac 设置显示“已连接，等待手机操作”，设备名称“我的电脑”。未更改电脑操作权限或系统休眠设置；真机远程任务仍待用户操作验证。

## 0.2.1 Remote 回复样式更新

2026-09-12 23:29:47（北京时间）上传 0.2.1 (2026091203) 成功，Apple 处理完成并加入既有个人内测组，构建为 **Testing**、有效期 90 天，中文测试说明已保存。测试者页显示 iPhone 已安装该版本。用户气泡靠右，思考/执行过程默认折叠，最终答案增加复制、选择与系统分享，代码块边界更清晰。3 项单元测试、1 项原生 UI 测试通过，并逐张检查六张截图；[修复证据与限制](reply-fix-20260912/README.md)。本次只更新 iPhone，既有 Mac 登录与 Worker 保持兼容。
