#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(dirname "$here")
out=$here/dist

want=$(sed -n 's/^wasm-bindgen = "=\(.*\)"$/\1/p' "$root/Cargo.toml")
[ -n "$want" ] || { echo "Cargo.toml no longer pins wasm-bindgen exactly" >&2; exit 1; }
got=$(wasm-bindgen --version 2>/dev/null | cut -d' ' -f2 || true)
if [ "$got" != "$want" ]; then
  echo "wasm-bindgen $want required, found ${got:-none}; cargo install --locked wasm-bindgen-cli --version $want" >&2
  exit 1
fi

cd "$root"
cargo build --release --target wasm32-unknown-unknown
rm -rf "$out"
wasm-bindgen --target web --no-typescript --remove-name-section \
  --out-dir "$out" --out-name perch \
  "${CARGO_TARGET_DIR:-$root/target}/wasm32-unknown-unknown/release/perch.wasm"
cp "$here/index.html" "$out/index.html"
echo "web/dist: $(du -sh "$out" | cut -f1)"
