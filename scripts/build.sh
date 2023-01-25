#!/usr/bin/env bash

cargo +nightly fmt || exit 1

# Check first to avoid lengthy compilation if later stages are bound to fail
touch wasm/store.wasm
cargo check -p mb-store
cargo check -p mb-factory
cargo check -p mb-legacy-market
cargo check -p mb-interop-market

cargo store || exit 1
cargo factory || exit 1
cargo legacy-market || exit 1
cargo interop-market || exit 1
