#!/usr/bin/env bash

cargo +nightly fmt || exit 1

# Check first to avoid lengthy compilation if later stages are bound to fail
touch wasm/store.wasm
cargo check -p mb-store || exit 1
cargo check -p mb-factory || exit 1
cargo check -p mb-legacy-market || exit 1
cargo check -p mb-interop-market || exit 1

cargo clippy -- -D warnings || exit 1

build() {
  cargo "$1" || return 1
  mv "wasm/$1.wasm" "wasm/$1-raw.wasm"
  wasm-opt "wasm/$1-raw.wasm" -Oz -o "wasm/$1.wasm"
}

build store || exit 1
build factory || exit 1
build legacy-market || exit 1
build interop-market || exit 1
