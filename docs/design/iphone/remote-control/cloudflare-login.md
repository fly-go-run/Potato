# Cloudflare 账号关联

用户选定：保持 ChatGPT 式侧栏与 Remote 页面；用同一账号关联手机和电脑，避免每台设备手工复制配对码。

## 方式

使用 Cloudflare Access 的 Cloudflare 身份提供方。官方支持以 Cloudflare 账号登录，并可限制为当前 Cloudflare 账户成员：
https://developers.cloudflare.com/cloudflare-one/integrations/identity-providers/cloudflare/

Worker 按官方方法验证 Access JWT 的 RS256 签名、issuer、audience、sub、iat 和 exp，关联键为 `SHA256(issuer + sub)`。邮箱仅用于展示，不决定授权：
https://developers.cloudflare.com/cloudflare-one/access-controls/applications/http-apps/authorization-cookie/validating-json/

Cloudflare 另有 OAuth/OIDC 公共客户端方案，官方支持原生客户端 Authorization Code + PKCE。当前实现选择 Access 作为个人使用的登录层，不索取用户 Cloudflare API 管理权限，也没有将第三方 OAuth 的设备授权接口当作已支持的能力：
https://developers.cloudflare.com/fundamentals/oauth/create-an-oauth-client/

## 用户流程

1. 电脑 Potato → 设置 → 能力 → iPhone 远程控制 → 登录 Cloudflare 账号。
2. 在系统浏览器完成 Cloudflare 登录，核对两端显示的验证码，确认登录。
3. 回到电脑，主动开启远程访问。登录本身不会开启本机控制。
4. iPhone 侧栏 → 远程 → 登录 Cloudflare 账号，完成相同账号的登录。
5. 手机自动列出账号下的电脑；在线电脑可以查看项目/会话、发起/继续任务、停止、审批和回答提问。
6. 手机的「管理电脑」可以撤销某台电脑，或退出当前手机；电脑端可独立关闭访问或退出并撤销本机。

验证码确认页属于 Potato 的原生应用浏览器交接，不是对 Cloudflare OAuth Device Authorization grant 的实现。应用凭据不出现在浏览器 URL；登录请求五分钟过期，应用会话最长三十天，可提前撤销。撤销记录阻止旧登录请求再次换回会话。

## 部署前需要配置

2026-09-12 已部署到 `https://potato-remote.recodex.top`。Access 团队为 `https://liuxu-cf.cloudflareaccess.com`，应用为 Potato Remote Login；仅允许所有者邮箱并使用限制账号成员的 Cloudflare IdP。配置及上线验收见 [发布记录](release.md)。缺少配置时服务明确拒绝登录。

- `REMOTE_PUBLIC_URL`：本次远程服务的 HTTPS 根地址，手机和电脑使用同一地址。
- `REMOTE_ACCESS_TEAM`：Access 团队地址，格式 `https://<team>.cloudflareaccess.com`。
- `REMOTE_ACCESS_AUD`：为该应用签发的 Access audience。
- Access 自托管应用保护 `<REMOTE_PUBLIC_URL>/v1/remote/auth/authorize`；启用 Cloudflare IdP，并为实际允许的用户设置策略。API 和 WebSocket 路径使用 Potato 自己的应用凭据，不应被交互式登录重定向覆盖。
- Worker 的 `REMOTE_ACCOUNTS`、`REMOTE_LOGINS` 与 `REMOTE_DEVICES` Durable Object 绑定及 SQLite 迁移已进入配置。部署时不能丢失既有聊天、语音、搜索和 E2B Secret。

不要把 Cloudflare account ID、邮箱、Access 的客户端可伪造请求头，或已通过的本地签名测试，直接当成真实身份认证。真实域名浏览器登录与原生两端关联结果单独记录在发布验收中。

## 已有验证

本地 Miniflare 执行真实 Worker、Durable Object、WebSocket 和 JWT 校验；仅签名身份的公钥获取由合成 IdP 替代。覆盖同账号设备目录、同邮箱不同身份隔离、手机/宿主角色限制、电脑撤销、手机退出、撤销后旧请求重放、错误签名、错误 audience、过期 JWT、跨站确认与 CSRF 重放。没有访问真实账户。

Rust 原生 API 的合成本机 HTTP 测试覆盖登录开始、轮询、注册、凭据加密落盘、登录默认不开远程、显式开启和退出清理。它验证桌面协议与保存方式，不代表 Cloudflare 真实浏览器登录已通过。

2026-09-12 追加真实云端验证：桌面核心和原生 iPhone 模拟器使用实际 Access 身份完成同账号关联，任务与退出流程通过；详见 [发布记录](release.md)。上面的合成本地测试说明仅描述各自证据范围。
