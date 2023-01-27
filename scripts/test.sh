#!/usr/bin/env bash

# This needs to be done because it sometimes keeps running in the background
kill_sandbox() {
  killall near-sandbox >/dev/null 2>&1
  pkill near-sandbox >/dev/null 2>&1
}

kill_sandbox

# Limit to 6 parallel tests to prevent hiccups with the key store
(cd tests && npm test -- -c 6 --fail-fast "$@") || {
  kill_sandbox
  echo "Testing failed"
  exit 1
}

kill_sandbox
