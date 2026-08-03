#!/usr/bin/env bash
# Build a full wheel package including the default web app.
# Run from repo root: bash scripts/wheel_build.sh
set -e

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

APP_DIR="$REPO_ROOT/app"
WEB_DEST="$REPO_ROOT/src/qwenpaw/console"

echo "[wheel_build] Building default web app..."
(cd "$APP_DIR" && npm ci)
(cd "$APP_DIR" && npm run build)

echo "[wheel_build] Copying app/dist/* -> src/qwenpaw/console/..."
rm -rf "$WEB_DEST"/*

mkdir -p "$WEB_DEST"
cp -R "$APP_DIR/dist/"* "$WEB_DEST/"

echo "[wheel_build] Bundling website docs into package..."
DOCS_SRC="$REPO_ROOT/website/public/docs"
DOCS_DEST="$REPO_ROOT/src/qwenpaw/docs"
rm -rf "$DOCS_DEST"
mkdir -p "$DOCS_DEST"
cp "$DOCS_SRC/"*.md "$DOCS_DEST/"

echo "[wheel_build] Building wheel + sdist..."
python3 -m pip install --quiet build
rm -rf dist/*
python3 -m build --outdir dist .

echo "[wheel_build] Done. Wheel(s) in: $REPO_ROOT/dist/"
