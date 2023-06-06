#!/usr/bin/env bash

cargo +nightly fmt || exit 1

# Check first to avoid lengthy compilation if later stages are bound to fail
touch wasm/store-v1.wasm
touch wasm/store-v2.wasm
cargo check -p mb-store-v1 || exit 1
cargo check -p mb-store-v2 || exit 1
cargo check -p mb-factory-v1 || exit 1
cargo check -p mb-factory-v2 || exit 1
cargo check -p mb-legacy-market || exit 1
cargo check -p mb-interop-market || exit 1

cargo clippy -- -D warnings || exit 1

build() {
  cargo "$1" || return 1
  mv "wasm/$1.wasm" "wasm/$1-raw.wasm"
  wasm-opt "wasm/$1-raw.wasm" -Oz -o "wasm/$1.wasm"
}

build store-v1 || exit 1
build store-v2 || exit 1
build factory-v1 || exit 1
build factory-v2 || exit 1
build legacy-market || exit 1
build interop-market || exit 1
