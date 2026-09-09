#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if [[ "$(wasm-bindgen --version)" != "wasm-bindgen 0.2.127" ]]; then
    echo 'Use nix develop .#web, or install wasm-bindgen-cli 0.2.127.' >&2
    exit 1
fi

paint_target_dir="${CARGO_TARGET_DIR:-target}"
paint_web_dir="$paint_target_dir/web"

cargo build --locked --release --lib --target wasm32-unknown-unknown \
    --target-dir "$paint_target_dir"
mkdir -p "$paint_web_dir/pkg"
wasm-bindgen --target web --out-dir "$paint_web_dir/pkg" --out-name paint_10 \
    "$paint_target_dir/wasm32-unknown-unknown/release/paint_10.wasm"
cp web/index.html "$paint_web_dir/index.html"
cp assets/paint-10.svg assets/paint-10.png assets/paint-10.ico "$paint_web_dir/"
cp assets/fonts/DejaVu-LICENSE.txt "$paint_web_dir/"
mkdir -p "$paint_web_dir/licenses"
cp LICENSE assets/fonts/DejaVu-LICENSE.txt assets/licenses/*.txt "$paint_web_dir/licenses/"

printf 'Built %s. Serve it with:\n' "$paint_web_dir"
printf 'python3 -m http.server 8080 --bind 127.0.0.1 --directory %q\n' "$paint_web_dir"
