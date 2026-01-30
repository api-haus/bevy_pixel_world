#!/bin/bash
set -euo pipefail
DIST="${1:-crates/game/dist}"
R2="https://bevy-pixel-world-assets.yura415.workers.dev/game.wasm"
WASM=$(basename "$DIST"/*_bg.wasm)
JS=$(grep -oP "from '/\K[^']+\.js" "$DIST/index.html")

cat > "$DIST/init.js" << INIT
import init, * as bindings from '/$JS';
const r = await fetch('$R2'), t = +r.headers.get('content-length')||0, rd = r.body.getReader();
const chunks = []; let l = 0;
while(true) { const {done,value} = await rd.read(); if(done)break; chunks.push(value); l+=value.length; if(t&&window.updateLoadingProgress)window.updateLoadingProgress(l,t); }
const b = new Uint8Array(l); let o=0; for(const c of chunks){b.set(c,o);o+=c.length;}
const wasm = await init({module_or_path:b}); if(window.hideLoading)window.hideLoading();
window.wasmBindings = bindings; dispatchEvent(new CustomEvent("TrunkApplicationStarted",{detail:{wasm}}));
INIT

sed -i "/<script type=\"module\">/,/<\/script>/c\\<script type=\"module\" src=\"/init.js\"></script>" "$DIST/index.html"
sed -i "s|href=\"/$WASM\"|href=\"$R2\"|g" "$DIST/index.html"
rm "$DIST"/*_bg.wasm
echo "Patched: $R2"
