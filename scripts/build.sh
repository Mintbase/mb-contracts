#!/usr/bin/env bash

cargo +nightly fmt || exit 1

# Check first to avoid lengthy compilation if later stages are bound to fail
touch wasm/mb-nft-v1.wasm
touch wasm/mb-nft-v2.wasm
cargo clippy -p mb-sdk -- -D warnings || exit 1
cargo clippy -p mb-nft-v1 -- -D warnings || exit 1
cargo clippy -p mb-nft-v2 -- -D warnings -A clippy::too_many_arguments || exit 1
cargo clippy -p mb-factory-v1 -- -D warnings || exit 1
cargo clippy -p mb-factory-v2 -- -D warnings || exit 1
cargo clippy -p mb-legacy-market -- -D warnings || exit 1
cargo clippy -p mb-interop-market -- -D warnings || exit 1

build() {
  cargo "$1" || return 1
  mv "wasm/$1.wasm" "wasm/$1-raw.wasm"
  wasm-opt "wasm/$1-raw.wasm" -Oz -o "wasm/$1.wasm"
}

build mb-nft-v1 || exit 1
build mb-nft-v2 || exit 1
build factory-v1 || exit 1
build factory-v2 || exit 1
build legacy-market || exit 1
build interop-market || exit 1
