#!/usr/bin/env bash
# Builds the browser player into crates/kairos-player/pkg (ES module for `wasm-bindgen --target web`).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
profile="${PROFILE:-release}"
version="$(grep -A1 'name = "wasm-bindgen"' "$root/Cargo.lock" | sed -n 's/version = "\(.*\)"/\1/p')"

# `cargo install` puts wasm-bindgen here, which is not on PATH for every rustup setup.
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"

rustup target add wasm32-unknown-unknown >/dev/null
if ! wasm-bindgen --version 2>/dev/null | grep -q "$version"; then
  cargo install wasm-bindgen-cli --version "$version" --locked
fi

flag=$([ "$profile" = release ] && echo --release || echo)
cargo build -p kairos-player --target wasm32-unknown-unknown $flag --manifest-path "$root/Cargo.toml"
dir=$([ "$profile" = release ] && echo release || echo debug)
wasm-bindgen --target web --out-dir "$root/crates/kairos-player/pkg" \
  "$root/target/wasm32-unknown-unknown/$dir/kairos_player.wasm"
echo "built $root/crates/kairos-player/pkg"
