#!/bin/bash
set -euo pipefail
DIST="${1:-crates/game/dist}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GAME_DIR="$(dirname "$SCRIPT_DIR")/crates/game"

COMMIT_HASH="${COMMIT_HASH:-$(git rev-parse --short=8 HEAD 2>/dev/null || echo 'dev')}"
R2="${R2_WASM_URL:-https://bevy-pixel-world-assets.yura415.workers.dev/game.wasm}?v=${COMMIT_HASH:0:8}"
WASM=$(basename "$DIST"/*_bg.wasm)
JS=$(grep -oP "from '/\K[^']+\.js" "$DIST/index.html")

# Generate init.js from template
sed -e "s|__JS_FILE__|$JS|g" \
    -e "s|__R2_URL__|$R2|g" \
    "$GAME_DIR/init.js.template" > "$DIST/init.js"

# Inject commit hash into service worker
sed -i "s/__VERSION__/$COMMIT_HASH/g" "$DIST/sw.js"

# Patch index.html
sed -i "/<script type=\"module\">/,/<\/script>/c\\<script type=\"module\" src=\"/init.js\"></script>" "$DIST/index.html"
sed -i "s|href=\"/$WASM\"|href=\"$R2\"|g" "$DIST/index.html"

rm "$DIST"/*_bg.wasm
echo "Patched: $R2 (version: $COMMIT_HASH)"
