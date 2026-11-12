#!/usr/bin/env bash
# Install the official Linux CI binary without compiling the CLI and its deps.
set -euo pipefail

version=0.2.127
checksum=61d4a7dc85acfa0d2354ccc0b8361928c7e52a746d17f28ebaa795ed3dc1614a
if [[ "$(wasm-bindgen --version 2>/dev/null || true)" == "wasm-bindgen $version" ]]; then
    exit 0
fi

archive="wasm-bindgen-$version-x86_64-unknown-linux-musl"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
curl --fail --location --retry 3 \
    "https://github.com/wasm-bindgen/wasm-bindgen/releases/download/$version/$archive.tar.gz" \
    --output "$work/archive.tar.gz"
printf '%s  %s\n' "$checksum" "$work/archive.tar.gz" | sha256sum --check
tar -xzf "$work/archive.tar.gz" -C "$work"
install -Dm755 "$work/$archive/wasm-bindgen" "${CARGO_HOME:-$HOME/.cargo}/bin/wasm-bindgen"
"${CARGO_HOME:-$HOME/.cargo}/bin/wasm-bindgen" --version
