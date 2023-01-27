# #!/usr/bin/env bash

[[ -d wasm ]] || mkdir wasm || exit 1

(
  cd tests || exit 1
  npm ci || exit 1
  [[ -d downloads ]] || mkdir downloads || exit 1
) || exit 1
echo ">>>> Test setup successful"

# Check that cargo has all necessary components
rustup target add wasm32-unknown-unknown || exit 1
rustup toolchain install stable || exit 1
rustup toolchain install nightly || exit 1
rustup component add rustfmt || exit 1
rustup component add clippy || exit 1
echo ">>>> Cargo setup successful"

# Check that wasm-opt is installed
if ! command -v wasm-opt &>/dev/null; then
  echo ">>>> wasm-opt is missing, but required to optimize wasm blobs"
  exit 1
fi
