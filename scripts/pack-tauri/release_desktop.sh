#!/usr/bin/env bash
# 桌面版发布:构建/摄取产物 → 签名校验 → 生成更新清单 → 推送到自有分发服务器。
#
# 分发架构(R2 + VPS 混合):
#   大文件产物 → Cloudflare R2 桶 potato-updates,经 https://dl.recodex.top
#   对外(零出站流量费,rclone remote 名 "r2")。
#   更新清单 → g-vps /srv/potato-updates/metadata(Caddy no-store,
#   充当即时生效的"发版开关")。
#   应用内 tauri-plugin-updater 轮询 metadata/qwenpaw-tauri-latest.json。
#
# 用法:
#   release_desktop.sh macos            本地构建 macOS 并暂存产物
#   release_desktop.sh macos --skip-build   复用已有构建产物,只做暂存
#   release_desktop.sh windows <dir>    摄取 CI 下载的 Windows 产物目录
#                                       (需包含 *-setup.exe 与同名 .sig)
#   release_desktop.sh publish          聚合清单并推送到 VPS
#   release_desktop.sh status           查看线上当前版本
#
# 典型发版流:
#   ./scripts/pack-tauri/release_desktop.sh macos
#   gh run download <run-id> -n <windows-artifact> -D /tmp/win-artifacts
#   ./scripts/pack-tauri/release_desktop.sh windows /tmp/win-artifacts
#   ./scripts/pack-tauri/release_desktop.sh publish
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
STAGE_DIR="$REPO_ROOT/dist/updates"
REMOTE_HOST="g-vps"
REMOTE_ROOT="/srv/potato-updates"
R2_REMOTE="r2:potato-updates"
BASE_URL="https://dl.recodex.top/artifacts"
MANIFEST_NAME="qwenpaw-tauri-latest.json"
SIGNING_KEY="${TAURI_SIGNING_PRIVATE_KEY_PATH:-$HOME/.tauri/potato-updater.key}"
TAURI_CONF="$REPO_ROOT/console/src-tauri/tauri.conf.json"

version() {
  # 与构建脚本同源:src/qwenpaw/__version__.py
  python3 - "$REPO_ROOT/src/qwenpaw/__version__.py" <<'EOF'
import re, sys, pathlib
text = pathlib.Path(sys.argv[1]).read_text()
match = re.search(r'__version__\s*=\s*"([^"]+)"', text)
print(match.group(1) if match else "0.0.0")
EOF
}

require_key() {
  if [[ ! -f "$SIGNING_KEY" ]]; then
    echo "错误: 找不到更新签名私钥 $SIGNING_KEY" >&2
    echo "生成方式: console/node_modules/.bin/tauri signer generate -w ~/.tauri/potato-updater.key" >&2
    exit 1
  fi
}

mac_target() {
  # 按构建机架构推导 updater target,防止 Intel 机器发出 aarch64 条目。
  case "$(uname -m)" in
    arm64) echo "darwin-aarch64" ;;
    x86_64) echo "darwin-x86_64" ;;
    *) echo "错误: 未知 mac 架构 $(uname -m)" >&2; exit 1 ;;
  esac
}

stage_macos() {
  require_key
  local ver skip_build="${1:-}" target bundle_dir
  ver="$(version)"
  target="$(mac_target)"
  bundle_dir="$REPO_ROOT/console/src-tauri/target/release/bundle/macos"
  if [[ "$skip_build" != "--skip-build" ]]; then
    echo "==> 构建 macOS 桌面包"
    (cd "$REPO_ROOT/app" && npm run build && rm -f dist/__qa.html)
    TAURI_SIGNING_PRIVATE_KEY="$(cat "$SIGNING_KEY")" \
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" \
      bash "$REPO_ROOT/scripts/pack-tauri/build_macos_pyinstaller.sh"
  fi
  # 构建脚本的最终递归重签发生在 tauri 打包之后,tauri 阶段产出的
  # tar 里是重签前的 app。这里一律以"最终签名后的 .app"重新打包
  # 并重新签名,保证更新档案与磁盘上验证过的 bundle 逐字节一致。
  echo "==> 以最终签名状态重打更新档案"
  rm -f "$bundle_dir/Potato.app.tar.gz" "$bundle_dir/Potato.app.tar.gz.sig"
  (cd "$bundle_dir" && tar -czf Potato.app.tar.gz Potato.app)
  TAURI_SIGNING_PRIVATE_KEY="$(cat "$SIGNING_KEY")" \
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" \
    "$REPO_ROOT/console/node_modules/.bin/tauri" signer sign \
    "$bundle_dir/Potato.app.tar.gz" >/dev/null
  mkdir -p "$STAGE_DIR"
  echo "==> 暂存 macOS 更新档案 (v$ver, $target)"
  python3 "$REPO_ROOT/scripts/pack-tauri/generate_update_manifest.py" stage \
    --bundle-dir "$bundle_dir" \
    --pattern '*.app.tar.gz' \
    --target "$target" \
    --output "$STAGE_DIR/Potato-$ver-macos-$(uname -m).app.tar.gz" \
    --pubkey-config "$TAURI_CONF"
  echo "OK: $STAGE_DIR"
}

stage_windows() {
  local dir="${1:?用法: release_desktop.sh windows <CI 产物目录>}"
  local ver flat
  ver="$(version)"
  # CI 可能把 exe 与 .sig 分放子目录:拷平到临时目录,且要求恰好一个安装器,
  # 防止摄取到历史构建时静默选错文件。
  flat="$(mktemp -d)"
  find "$dir" -type f \( -name '*-setup.exe' -o -name '*-setup.exe.sig' \) \
    -exec cp {} "$flat/" \;
  local count
  count="$(find "$flat" -name '*-setup.exe' | wc -l | tr -d ' ')"
  if [[ "$count" != "1" ]]; then
    echo "错误: $dir 下找到 $count 个 *-setup.exe(期望恰好 1 个)" >&2
    exit 1
  fi
  mkdir -p "$STAGE_DIR"
  echo "==> 暂存 Windows 安装器 (v$ver) 自 $dir"
  python3 "$REPO_ROOT/scripts/pack-tauri/generate_update_manifest.py" stage \
    --bundle-dir "$flat" \
    --pattern '*-setup.exe' \
    --target windows-x86_64 \
    --output "$STAGE_DIR/Potato-$ver-Windows-setup.exe" \
    --pubkey-config "$TAURI_CONF"
  rm -rf "$flat"
  echo "OK: $STAGE_DIR"
}

publish() {
  local ver
  ver="$(version)"
  shopt -s nullglob
  # 只认平台 sidecar;上次 publish 留下的 latest manifest 不能混进输入。
  local sidecars=("$STAGE_DIR"/tauri-*-updater.json)
  if [[ ${#sidecars[@]} -eq 0 ]]; then
    echo "错误: $STAGE_DIR 里没有暂存的平台产物,先跑 macos/windows 子命令" >&2
    exit 1
  fi
  # 版本一致性:所有暂存产物文件名必须带当前版本号,防止混发旧构建。
  local artifact
  for artifact in "$STAGE_DIR"/Potato-*; do
    [[ "$artifact" == *.json ]] && continue
    if [[ "$(basename "$artifact")" != *"$ver"* ]]; then
      echo "错误: 暂存产物 $(basename "$artifact") 与当前版本 v$ver 不一致" >&2
      echo "清理 $STAGE_DIR 后重新 stage,或修正 src/qwenpaw/__version__.py" >&2
      exit 1
    fi
  done
  local metadata_args=()
  for sidecar in "${sidecars[@]}"; do
    metadata_args+=(--metadata "$sidecar")
  done
  echo "==> 生成清单 v$ver(平台: ${#sidecars[@]} 个)"
  python3 "$REPO_ROOT/scripts/pack-tauri/generate_update_manifest.py" manifest \
    --version "$ver" \
    --base-url "$BASE_URL" \
    "${metadata_args[@]}" \
    --output "$STAGE_DIR/$MANIFEST_NAME"

  echo "==> 推送产物到 R2 ($R2_REMOTE/artifacts)"
  # 先传产物再传清单:清单最后落地充当"发版开关"(metadata 在 VPS,
  # Caddy 已配 no-store,发布/停发即时生效)。
  rclone copy --s3-upload-cutoff 100M --s3-chunk-size 50M \
    --exclude "*.json" \
    "$STAGE_DIR/" "$R2_REMOTE/artifacts/"
  echo "==> 推送清单到 $REMOTE_HOST:$REMOTE_ROOT/metadata"
  local remote_tmp="/tmp/${MANIFEST_NAME}.${ver}.$$"
  command scp "$STAGE_DIR/$MANIFEST_NAME" "$REMOTE_HOST:$remote_tmp"
  command ssh "$REMOTE_HOST" \
    "sudo -n install -o root -g root -m 0644 '$remote_tmp' '$REMOTE_ROOT/metadata/$MANIFEST_NAME' && rm -f '$remote_tmp'"

  echo "==> 线上验证"
  curl -fsS "https://chat.recodex.top/potato-updates/metadata/$MANIFEST_NAME" \
    | python3 -m json.tool | head -20
  echo "发布完成: v$ver"
}

status() {
  local code
  code="$(curl -s -o /tmp/potato-manifest.json -w '%{http_code}' \
    "https://chat.recodex.top/potato-updates/metadata/$MANIFEST_NAME")"
  if [[ "$code" == "404" ]]; then
    echo "尚未发布任何版本(线上无清单)。"
  elif [[ "$code" == "200" ]]; then
    python3 -m json.tool /tmp/potato-manifest.json
  else
    echo "线上清单不可达: HTTP $code" >&2
    exit 1
  fi
}

case "${1:-}" in
  macos) stage_macos "${2:-}" ;;
  windows) stage_windows "${2:-}" ;;
  publish) publish ;;
  status) status ;;
  *)
    grep '^#' "$0" | sed 's/^# \{0,1\}//' | sed -n '2,20p'
    exit 1
    ;;
esac
