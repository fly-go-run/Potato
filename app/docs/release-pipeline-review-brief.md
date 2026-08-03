# 任务:review 桌面版自有分发/自动更新管线

你是严苛的发布工程审查者。只审查,**不修改任何文件**。

## 审查对象

1. `scripts/pack-tauri/release_desktop.sh`(新)— 发版脚本:macos 构建+暂存 / windows 摄取 CI 产物 / publish 聚合清单并 rsync / status。
2. `scripts/pack-tauri/RELEASE.md`(新)— 流程文档。
3. `console/src-tauri/tauri.conf.json` 的 `plugins.updater` 变更:pubkey 换为自有 minisign 公钥,endpoint 换为 `https://chat.recodex.top/potato-updates/metadata/qwenpaw-tauri-latest.json`。
4. 配套基础设施(描述,验证其合理性):
   - VPS(Ubuntu/Caddy):`/srv/potato-updates/{metadata,artifacts}`;Caddy 站点 chat.recodex.top 内新增 `handle_path /potato-updates/* { root * /srv/potato-updates; file_server }`,置于原有 catch-all `handle` 之前;经 Cloudflare 代理;已实测 HTTPS 可达。
   - 私钥 `~/.tauri/potato-updater.key`(无口令)。
   - 本次 macOS 档案因构建脚本跳过 updater 步,由手工 `tar -czf Potato.app.tar.gz Potato.app` + `tauri signer sign` 补齐,再 `--skip-build` stage(key-id 校验通过);脚本已修为传 `TAURI_SIGNING_PRIVATE_KEY` 内容,后续构建应自动产出。
   - 既有工具 `generate_update_manifest.py`(stage/manifest)与 CI `build-tauri-windows` 不在本次改动内,但契约相关。

## 重点挑战

1. **更新安全**:无口令私钥的风险边界;pubkey/endpoint 替换是否完整(有没有别处残留上游 endpoint/pubkey,如 tauri.version.conf.json 生成逻辑、CI vars);Cloudflare 缓存 metadata JSON 导致回滚/发布延迟(是否需要 Cache-Control/绕过)。
2. **脚本正确性**:publish 的 rsync 排除逻辑(`--exclude '*.json'` 与 sidecar/.sig 的实际分发)、清单最后落地的原子性声明是否成立(rsync 非原子写;CF 缓存)、多平台 sidecar 聚合时版本不一致的场景(mac 是 2.0.1、windows 是 2.0.2 时会怎样)、`--skip-build` 语义、status 404 时的表现。
3. **手工档案的等价性**:手工 tar 的目录结构/权限与 tauri build 产出的 .app.tar.gz 是否等价(updater 解包路径预期),符号链接处理(tar 默认保留 symlink,mac .app 内有 Frameworks symlink?gzip tar 是否会破坏签名后的 bundle)。
4. **Windows 路径**:摄取 CI 产物模式对 `*-setup.exe`+`.sig` 的假设与 CI 实际产物名的匹配;NSIS passive 安装模式与 updater 的兼容。
5. **文档遗漏**:RELEASE.md 里会让三个月后的用户踩坑的空白。

## 输出

P0/P1/P2,每条:位置 + 触发场景 + 一句话修法。最后总评:这套管线能不能长期用。
