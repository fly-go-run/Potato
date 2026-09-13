# iPhone 云端模型与邮箱授权

2026-09-13 桌面接入更新：模型同步脚本已改为始终保留现有邮箱白名单，以下「每次重设所有者」仅描述首次部署的旧行为。新的独立邮箱验证码入口与管理说明见[桌面云端模型](../../../architecture/cloud-desktop/README.md)。

后续更新：目录已精简为 DeepSeek V4.1 Flash 与 GPT-5.6，当前思考档位、生成探测及新 Worker 版本见 [模型能力验证](../model-capabilities-20260913/README.md)。下方模型数量和 Apple 构建状态记录首次云端账号上线；新的客户端修改已随 0.2.2 (2026091303) 上传 Apple，分发状态见最新记录。

2026-09-13：原生 SwiftUI 客户端增加「设置 → 云端模型」。用户通过 Cloudflare Access 登录后，自动读取云端目录及默认模型；不需要填写模型密钥或接口，不依赖桌面电脑在线。模型选择继续按会话保存，切换服务不清除草稿和历史。

## 权限

- 首次发布仅允许已核对的所有者邮箱 `pal_xu@163.com`。Cloudflare 现有 Access 登录策略未放宽。
- 云端使用独立的 `cloud` 会话角色，Keychain 和偏好项与远程电脑会话分开。不能用它列出电脑、注册电脑、发起远程任务或撤销电脑。
- Access JWT 校验后的邮箱随应用会话保存；每个云端请求重新验证会话有效期、撤销状态、角色和当前 `CLOUD_ALLOWED_EMAILS`。
- 旧的固定设备令牌、远程控制的 `phone` / `host` 会话不能调用启用账号模式的模型、模型目录、语音或沙箱接口。
- 旧应用会话未保存已验证邮箱时，需要重新登录。退出云端仅撤销该云端会话，不退出远程电脑账号。
- 授权变更阻止后续请求，不主动中断已经开始的流式生成。Cloudflare Access 的登录策略与 Worker 邮箱白名单均应在增加其他用户时明确更新；不要用添加宽泛 Include 登录方式的方式绕过邮箱限制。

最初联调因合并登录会同时授予远程电脑权限，被自动审批拒绝，未点击成功。随后实际增加独立角色与服务端拒绝规则，重新部署后才完成仅限云端的登录。不是仅修改确认文案。

## 配置来源与部署

`native/potato-worker/scripts/cloud-config.mjs` 从原生 `~/.potato/native-v1/potato.sqlite3` 的一致性快照读取 DeepSeek、sub2api、模型配置和已登录桌面所有者邮箱。密钥使用原生 master key 在内存中解密，不输出到终端或源码。排除图片生成、音频/实时协议及桌面审批别名，这些不能当作聊天模型转发。

在 `native/potato-worker` 执行：

```sh
node scripts/sync-cloud-models.mjs                 # 仅显示脱敏配置摘要
node scripts/sync-cloud-models.mjs --probe         # 查询供应商目录，过滤已下线条目
node scripts/sync-cloud-models.mjs --probe --apply # 明确发布：只授权现有所有者
node scripts/sync-cloud-models.mjs --probe --apply --models-only # 更新目录与代码，保留现有邮箱授权
```

发布使用 `wrangler deploy --keep-vars --secrets-file`，将以下三个 Secret 与代码一起上传，保留豆包、Exa、E2B 和原有远程绑定及密钥：

- `CLOUD_PROVIDERS`：多个供应商的接口、密钥、模型和默认模型；只在 Worker 解析。
- `CLOUD_ALLOWED_EMAILS`：逗号分隔的完整邮箱，空列表拒绝访问。上述同步脚本每次明确重设为桌面所有者，适用于当前仅所有者部署。
- `CLOUD_AUTH_REQUIRED=true`：即使模型 Secret 意外缺失也不退回固定设备令牌模式。

请求使用 `provider/model` 公共 ID，Worker 按目录选择对应地址和密钥，发给上游时去掉 provider 前缀。搜索后的续答保持同一供应商。模型目录只返回名称和能力，不返回供应商密钥或接口。

首次上线时的目录查询：DeepSeek 本地临时模型已不在上游目录中，当时交集为 `deepseek-v4-pro`，sub2api 交集为 14 个聊天模型；当时只实测 DeepSeek V4 Pro 与 GPT-5.6。现已由 `cloud-model-policy.mjs` 明确限定手机目录为 `deepseek-flash` 和 `gpt-5.6`，独立于桌面所选模型，实际档位验证见顶部链接。

## 验证与发布状态

- Worker：42 项测试、TypeScript 检查、Wrangler 类型生成与 dry run 通过。包含模型路由/密钥隔离、搜索续答、固定令牌拒绝、伪造邮箱拒绝、撤销、缺少配置拒绝，以及真实签名 JWT + SQLite Durable Object 的独立角色权限测试。
- iOS：103 项单元测试和云端入口 UI 回归通过；覆盖 Keychain 引用、凭据不写入 JSON、端点编辑不泄露会话、退出时异步目录请求不能恢复旧连接、草稿与重启恢复。
- 真实浏览器：仅限云端角色的登录成功，确认页不授予远程电脑访问权限。
- 真实 iPhone 17 模拟器：登录后自动目录、DeepSeek 回复 `POTATO_CLOUD_OK` 与重启恢复通过。结果 `/tmp/potato-cloud-live-v4.xcresult`；截图见 `qa/live/`。前几轮测试断言错误及被拒绝的合并登录不计为通过。
- 真实 sub2api：iOS 模拟器切换 GPT-5.6，经线上 Worker 收到 `POTATO_SUB2API_OK`；随后退出云端回到登录页，通过 `/tmp/potato-cloud-sub2api-v2.xcresult`。截图见 `qa/sub2api/`。
- 线上 Worker 版本：`08d8e5e6-c8df-48b3-a236-60b6cc223a17`。两个域名健康检查为 200，未登录目录请求为 401。此前版本 `69312217-b291-4870-8142-499eb836b996`；中间版本曾使用合并角色，已被独立权限版本替代。
- iPhone 新归档：`native/potato-ios/build/distribution/PotatoMobile-0.2.2-2026091302.xcarchive`，Release 构建、签名、仅 iPhone 设备类型、iOS 17 最低版本和隐私清单核对通过。2026-09-13 15:12 经 Xcode Organizer 成功上传 Apple；15:17 确认状态 Testing，已加入现有「个人内测」组（1 位测试者），有效期 90 天，中文测试说明已保存。Apple 构建 ID `9baa5570-14d4-41db-bbfd-28cedbed9628`。手机通过 TestFlight → Potato Remote → 更新。

真实联调只使用合成文本，没有发送用户历史和附件。本轮未重新做豆包麦克风、真实沙箱文件、真机蜂窝网络或 iOS 17 实机验收。
