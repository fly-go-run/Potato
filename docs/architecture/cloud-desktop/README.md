# 桌面邮箱登录与云端模型

Windows/macOS 当前维护的 GPUI + potato-core 客户端复用 Potato Worker 的模型配置。安装包只包含服务地址，不包含任何供应商密钥、设备令牌或用户白名单。用户在「设置 → 模型与服务商 → 云端模型 → 使用邮箱登录」输入受邀邮箱和邮件验证码，在浏览器核对桌面确认码，返回客户端后自动获取模型。

## 随时修改邮箱白名单

白名单是 Cloudflare 的运行配置，不写进客户端或源代码。增删邮箱不需要重新发安装包。

1. 打开 [Potato cloud email allowlist 策略](https://dash.cloudflare.com/672ee45aa5baa156e0e966c9765e0bc1/one/access-controls/policies/863109ea-5e6b-426b-8744-ee1ca86b123c/edit)，修改「包含 → 电子邮件」，保存。保留精确邮箱规则，不增加 Everyone 或把「邮箱验证码登录方式」单独作为包含规则。
2. Cloudflare → Workers 和 Pages → `potato-iphone-api` → 设置 → 变量和机密，编辑 `CLOUD_ALLOWED_EMAILS`。值为完整邮箱列表，用英文逗号分隔。初始仅保留所有者；新增时保留已有邮箱。当前此值为加密 Secret，页面不能显示旧值，编辑时需要填写完整的新列表。
3. 新增邮箱时先更新 Worker 名单，再更新 Access 名单；撤销时先从 Worker 名单删除，再从 Access 删除。Worker 每次模型请求都检查当前名单，已有会话也不能绕过。已开始的回复不会因名单变化而主动中断。

[邮箱登录应用](https://dash.cloudflare.com/672ee45aa5baa156e0e966c9765e0bc1/one/access-controls/apps/self-hosted/cf9d0af2-e9ec-450e-84d5-90c5486fc3da/edit)只使用 One-time PIN，保护 `potato-remote.recodex.top/v1/cloud/auth/authorize`。用户无需注册 Cloudflare，也无需成为管理账户成员。原 `Potato Remote Login` 应用与其策略保持独立。

当前获准邮箱共享云端模型目录；尚未实现按邮箱分配不同模型或独立账单额度。模型费用计入 Worker 使用的供应商账户。

## 更新模型

在 `native/potato-worker` 执行 `node scripts/sync-cloud-models.mjs --probe --apply`。同步脚本只更新模型配置和强制账号鉴权开关，保留邮箱名单与其他服务配置；`--models-only` 作为兼容选项保留，所有模型更新均不再覆盖邮箱授权。

普通代码发布必须使用 `wrangler deploy --keep-vars`，保留后台管理的运行变量和 Secret。`CLOUD_PROVIDERS` 包含供应商地址、密钥、模型和默认模型，只在 Worker 使用；客户端只读取公开目录。

## 协议与本机权限

- 新入口：`/v1/cloud/auth/start`、`poll`、`authorize`，退出为 `/v1/cloud/account/logout`。使用独立 Access audience `CLOUD_ACCESS_AUD`；邮箱入口不能签发 host/phone 会话，不能接受远程登录请求的确认码。应用会话仍绑定经签名验证的 issuer + subject。
- 模型目录为 `/v1/models`；桌面聊天为 `/v1/desktop/chat/completions`，仅接受获准的 `cloud` 会话。保留模型工具定义、工具调用、工具结果、公开思考和 usage 流。Worker 不执行桌面工具，也不插入 iPhone 的搜索循环；文件、命令和审批继续在用户本机执行。
- iOS 原 `/v1/chat/completions`、语音、搜索和沙箱行为保持原协议。
- 本地云端会话与远程控制分开存放，凭据使用原生数据库密钥加密。`/api/models` 和设置接口只返回脱敏信息。托管服务商的地址、密钥和目录不能通过普通模型编辑接口覆盖；用户仍可选择模型和已声明支持的思考档位。
- 正常启动自动导入旧 `.potato`/`.potato.secret` 的兼容配置一次，保留原生端已配置的服务商和可用选择。密钥顺序为进程环境变量 → 原生数据目录 `.env` → 上级 `.potato/.env` → 加密配置；支持原来的 `<服务商>_API_KEY`、`POTATO_<服务商>_API_KEY`、`<服务商>_OPENAI`/`_CLAUDE` 别名。不会读取聊天项目内的 `.env`，云端会话也不会使用这些本机密钥。
- 模型选择顺序为现有可用本机选择（或用户手动选中的云端模型）→ 已配置的本机服务商模型 → 已登录云端账号的默认模型。自动回退到云端后若补回本机配置，会重新优先本机。只有模型名称而缺少密钥/有效地址的空配置不会挡住云端回退；手动选择云端后仍保留该选择。本机服务调用失败不会自动向云端重发任务。退出清除云端凭据及云端选择，保留聊天和自定义服务商；有本机配置时可继续使用本机模型。登录取消或退出会使较早的异步结果失效。
- `POTATO_NATIVE_DATA_DIR` 指定的独立环境不自动读取个人旧配置，仍可通过设置 → 数据显式导入。自动导入不覆盖原生密钥，不修改旧文件；成功后不重复导入，避免把用户已删除的旧服务商重新加回。损坏的旧配置会显示提示，允许继续云端登录。

## 验证记录

2026-09-13：Worker 53 项测试、类型检查与 dry run 通过；包括独立 audience、云端与远程角色隔离、工具往返、未授权和撤销、上游密钥隔离。Rust 核心完整回归通过，新增 3 项云端登录/持久化/退出竞态测试。GPUI 原有 65 项测试通过，已在独立临时数据目录启动 macOS GPUI，检查邮箱登录入口和确认码显示。

Worker 发布前版本 `b5a3fe31-3921-40d5-ac0d-11c9220019ce`；邮箱入口与桌面网关部署版本 `d54f067f-61bb-4593-9ab8-5c52dbe7fdaf`。

真实验证已完成：所有者邮箱通过 One-time PIN 登录，浏览器显示登录成功，GPUI 设置显示已登录邮箱和两个云端模型，首次安装自动选择 DeepSeek V4.1 Flash。关闭测试客户端后，独立核心进程从同一验收目录恢复加密会话并刷新目录；DeepSeek V4.1 Flash 与 GPT-5.6 均完成真实工具往返：模型发起 `echo $((17 * 23))`，本机 Seatbelt 沙箱执行成功（退出码 0、输出 391），模型收到结果后回复 391。测试只使用合成任务。原生窗口自动化在后续聊天操作中出现快照不刷新/窗口不可用，完整聊天结果通过核心事件及持久化记录核实，不能当作完整 GUI 聊天验收。

自定义域名与 workers.dev 的 `/v1/models`、`/v1/desktop/chat/completions` 未登录访问均返回 403。新增核心 3 项测试与 GPUI 65 项测试在最终代码再次通过。

macOS ARM64 release 构建、打包及 `verify_package.py --target aarch64-apple-darwin` 通过；ZIP/DMG 均通过签名、架构、解包/挂载、独立数据目录启动与驱动探针。产物在 `native/potato-gpui/dist/`，仅 ad-hoc 签名，未做 Apple 公证。Windows 和 Intel macOS 共用接入代码，但本轮未生成安装包、未做实机验收；需运行原生 GitHub Actions 工作流后再分发对应平台产物。

可选线上复验使用 `native/potato-core/examples/cloud_smoke.rs`。先在临时数据目录的客户端完成邮箱登录并关闭客户端，再设置 `POTATO_CLOUD_QA_DIR` 为该目录，执行 `cargo +1.96.1 run --locked --manifest-path native/potato-core/Cargo.toml --example cloud_smoke`。可用 `POTATO_CLOUD_QA_MODEL` 指定公开模型 ID 或名称。该命令会产生真实模型调用费用，并验证本机命令成功及最终回复，默认测试不运行它。

2026-09-13 本机配置兼容补充：新增 3 项旧配置/环境密钥测试和 1 项本机优先/云端回退测试。核心共 239 项测试通过，11 项按原测试配置跳过；GPUI 65 项测试通过，核心和 GPUI 的 all-targets Clippy 检查通过。覆盖旧 `.env` 密钥重新加密、环境密钥更新、旧文件保持不变、原生配置保留、重启选择保留、损坏配置提示、独立目录隔离、空配置回退、补回本机配置及手动云端选择。

兼容补丁的最终 macOS ARM64 release 已重新打包，ZIP/DMG 的包验证和 SHA-256 均通过。正式二进制在独立测试目录启动，只有上级 `.potato/.env` 的合成 DeepSeek 密钥；启动后的数据库确认自动选中 `deepseek/deepseek-chat`，不存在云端登录会话。该检查未发送模型请求；窗口自动化仅取得缩略画面，模型选择结果以实际进程写入的配置为证据。
