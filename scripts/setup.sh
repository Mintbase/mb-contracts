#!/usr/bin/env bash

mkdir wasm || exit 1

(
  cd tests || exit 1
  npm ci || exit 1
  mkdir downloads || exit 1
) || exit 1

# Check that cargo is installed + wasm

# Check that wasm-opt is installed
