# 任务:用你的 cloudflare-api MCP 完成 R2 分发桶设置

你装有 Cloudflare 官方 MCP 插件(cloudflare-api,OAuth 已授权)。用它的 search()/execute() 调 Cloudflare API 完成以下操作。**只做列出的操作,不要动账号下任何既有资源(zone 设置、DNS 既有记录、Workers 等)。**

背景:为桌面应用 Potato 的自动更新分发建对象存储。域名 zone: recodex.top。

## 步骤

1. 【验证】确认 MCP 可用:取账号信息(account id、账号名),确认 R2 是否已开通(list buckets 能成功即已开通;若报未开通/未订阅,直接停止并报告,不要尝试开通)。
2. 【建桶】创建 R2 bucket:名称 `potato-updates`,位置 hint APAC(如支持)。已存在则跳过。
3. 【自定义域】把 bucket 绑定自定义域 `dl.recodex.top`(zone recodex.top,R2 custom domain API 会自动建 DNS 记录)。已存在则跳过。报告绑定后的状态(是否 active)。
4. 【上传凭据】创建一个 Account API Token,权限仅 `Workers R2 Storage:Edit`(scope 到该账号),名称 `potato-updates-uploader`,不设 IP 限制,永不过期。输出 token 值与 token id(后续本机用它派生 S3 凭据走 rclone 上传)。若你的 OAuth 权限不足以创建 token,如实报告,不要用变通手段。
5. 【公共访问】确认 bucket 经自定义域的公共读取已启用(custom domain 默认公开读)。

## 输出格式

逐步骤:成功/失败/跳过 + 关键返回字段(account_id、bucket 名、domain 状态、token id 与 token 值)。失败时给出 API 错误原文。最后一行:R2_READY=yes/no。
