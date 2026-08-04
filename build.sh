#!/usr/bin/env bash
set -euo pipefail

DEST="${1:-$HOME/.local/share/zellij/plugins}"

cargo build -p tab-notes --release --target wasm32-wasip1
mkdir -p "$DEST"
cp target/wasm32-wasip1/release/tab-notes.wasm "$DEST/tab-notes.wasm"
echo "installed $DEST/tab-notes.wasm"
