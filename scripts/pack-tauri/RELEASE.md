# 桌面版发布与自动更新(自有分发)

## 架构

```
构建(本地 mac / CI windows)
  → tauri 构建时用私钥签名更新档案(minisign)
  → generate_update_manifest.py stage 暂存到 dist/updates/
  → release_desktop.sh publish 聚合 latest.json + rsync 到 VPS
  → 应用内 tauri-plugin-updater 轮询
     https://chat.recodex.top/potato-updates/metadata/qwenpaw-tauri-latest.json
  → 发现新版本 → 下载(经 Cloudflare)→ Windows passive 静默安装 / macOS 替换
```

- **产物(大文件)**:Cloudflare R2 桶 `potato-updates`,自定义域
  `https://dl.recodex.top`(公开读,零出站流量费)。本机 rclone remote 名
  `r2`(凭据在 `~/.config/rclone/rclone.conf`,bucket 级 token)。
- **清单(发版开关)**:VPS(vps-aozhou)`/srv/potato-updates/metadata`,
  Caddy 在 `chat.recodex.top` 站点内 `handle_path /potato-updates/*` 提供,
  metadata 显式 `Cache-Control: no-store` → 发布/停发即时生效。
  配置备份在 `/etc/caddy/Caddyfile.bak-potato`。
- **签名密钥**:`~/.tauri/potato-updater.key`(私钥,无口令)+ `.pub`。
  公钥已写入 `console/src-tauri/tauri.conf.json` 的 `plugins.updater.pubkey`,
  endpoint 指向上面的 metadata URL。

## ⚠️ 私钥备份

`~/.tauri/potato-updater.key` **丢了就永远无法给已装出去的客户端推更新**
(只能让用户重新手动安装)。请立刻备份到密码管理器/异地。不要提交进 git。

## 日常发版

```bash
# 1. 改版本号(唯一版本源: src/qwenpaw/__version__.py;pyproject 动态引用它)
# 2. 构建并暂存各平台
./scripts/pack-tauri/release_desktop.sh macos
gh run download <run-id> -n <windows-artifact> -D /tmp/win-artifacts
./scripts/pack-tauri/release_desktop.sh windows /tmp/win-artifacts
# 3. 发布(生成清单 + 推送;清单最后落地,天然是"发布开关")
./scripts/pack-tauri/release_desktop.sh publish
# 查看线上版本
./scripts/pack-tauri/release_desktop.sh status
```

**尽量两个平台同版本一起发**:清单缺某平台 target 时,该平台客户端每次
检查更新会报 `TargetsNotFound` 错误(不是安静的"无更新")。单平台发布
仅限紧急修复,并接受另一平台短暂报错。publish 会强制校验暂存产物文件名
与当前版本一致,防止混发旧构建。

Windows 产物注意:CI 可能把 `-setup.exe` 与 `.sig` 分成多个 artifact,
**全部下载到同一目录**再执行 `windows <dir>`;脚本会递归拷平并要求恰好
一个安装器。

## Windows 构建(CI)

`.github/workflows/desktop-build.yml` 的 `build-tauri-windows` 任务
(windows-latest + 自动验证 + 截图)。启用前需在 GitHub 仓库配置:

- Secret `TAURI_SIGNING_PRIVATE_KEY`:`~/.tauri/potato-updater.key` 的内容
- Var `TAURI_UPDATER_ENDPOINTS`:
  `https://chat.recodex.top/potato-updates/metadata/qwenpaw-tauri-latest.json`
- ⚠️ 检查并**删除/更新旧的 `TAURI_UPDATER_PUBKEY` var**(如存在):CI 的
  env 会覆盖 tauri.conf.json,残留上游公钥会让 CI 包收不到你的更新
- Secret `QWENPAW_DASHSCOPE_API_KEY`(verify 步骤要用;不配则跳过验证步骤)

注意:CI 构建来自仓库内容,**app/ 前端与所有改动必须先 commit + push**。

## 首次交付(给家人,零配置开箱即用)

```bash
# 1. 生成预配置文件(密钥只进这份文件,不进仓库/CI)
python3 scripts/pack-tauri/make_provision.py \
  --provider-id sub2api --base-url https://sub2api.recodex.top/v1 \
  --api-key <给家人单独开的key> \
  --model gpt-5.6 --model gpt-5.6-luna --active-model gpt-5.6 \
  --output dist/provision.json
# 2. 和安装器一起打包
cd dist && zip Potato-安装包.zip Potato-<ver>-Windows-setup.exe provision.json
```

对方:解压 → 双击安装器 → 打开即用(NSIS hook 自动收取同目录的
provision.json,后端首启走设置页同款代码路径应用并本机加密,归档为
provision.applied.json 防重复;实现见 `src/qwenpaw/app/provisioning.py`)。

注意:
- Defender SmartScreen 会拦未签名安装器:「更多信息 → 仍要运行」,教一次
- WebView2 由安装器静默自动装;之后所有更新走应用内自动更新,无需再发包
- 建议在 sub2api 网关给家人单独开 key(可限额、可单独吊销)

## 回滚

updater 只接受 `远端版本 > 本地版本`,**把清单改回旧版本号不会让已更新
的客户端降级**(只能阻止未更新的客户端继续升级)。真正回滚 = checkout
旧代码 → 把 `__version__.py` 提到一个**更高**的版本号 → 正常发版。
`artifacts/` 保留历史产物便于排查。

## 安全注意

- 私钥 `~/.tauri/potato-updater.key` 权限保持 600,无口令——本机失陷即
  具备恶意更新能力;备份放密码管理器,不进 git、不进网盘明文
- macOS 目前是 ad-hoc 签名,只适合自用/熟人分发;若要对外分发需
  Developer ID + notarization(记入 backlog)
