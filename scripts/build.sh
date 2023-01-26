#!/usr/bin/env bash

cargo +nightly fmt || exit 1

# Check first to avoid lengthy compilation if later stages are bound to fail
touch wasm/store.wasm
cargo check -p mb-store || exit 1
cargo check -p mb-factory || exit 1
cargo check -p mb-legacy-market || exit 1
cargo check -p mb-interop-market || exit 1

cargo store || exit 1
cargo factory || exit 1
cargo legacy-market || exit 1
cargo interop-market || exit 1
