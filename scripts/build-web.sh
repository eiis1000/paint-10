#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(wasm-bindgen --version)" != "wasm-bindgen 0.2.127" ]]; then
  echo 'Use nix develop .#web, or install wasm-bindgen-cli 0.2.127.' >&2
  exit 1
fi
cargo build --locked --release --lib --target wasm32-unknown-unknown
mkdir -p target/web/pkg
wasm-bindgen --target web --out-dir target/web/pkg --out-name paint_10 \
  target/wasm32-unknown-unknown/release/paint_10.wasm
cp web/index.html target/web/index.html
cp assets/paint-10.svg assets/paint-10.png assets/paint-10.ico target/web/
echo 'Built target/web. Serve it with: python3 -m http.server 8080 --bind 127.0.0.1 --directory target/web'
