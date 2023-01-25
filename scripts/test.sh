#!/usr/bin/env bash

scripts/build.sh || exit 1
cargo clippy -- -D warnings || exit 1

# This needs to be done because it sometimes keeps running in the background
kill_the_damn_sandbox() {
  killall near-sandbox >/dev/null 2>&1
  pkill near-sandbox >/dev/null 2>&1
}

kill_the_damn_sandbox

# Limit to 6 parallel tests to prevent hiccups with the key store
(cd tests && npm test -- -c 6 --fail-fast) || {
  kill_the_damn_sandbox
  echo "Testing failed"
  exit 1
}

kill_the_damn_sandbox
