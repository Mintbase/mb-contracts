#!/usr/bin/env bash

# This needs to be done because it sometimes keeps running in the background
kill_sandbox() {
  killall near-sandbox >/dev/null 2>&1
  pkill near-sandbox >/dev/null 2>&1
}

kill_sandbox

# Make sure node version selected using `n` is being used
export PATH="/usr/local/bin:$PATH"
(
  cd tests/node_modules/near-workspaces/node_modules || exit 1
  [[ -e near-sandbox.bak ]] || mv near-sandbox near-sandbox.bak
  ln -sF ../../near-sandbox ./
) || exit 1

# Limit to 6 parallel tests to prevent hiccups with the key store
# FIXME: reactivate
# (cd tests && MB_VERSION=v1 npm test -- -c 6 --fail-fast "$@") || {
#   kill_sandbox
#   echo "Testing failed (v1)"
#   exit 1
# }

(cd tests && MB_VERSION=v2 npm test -- -c 6 --fail-fast "$@") || {
  kill_sandbox
  echo "Testing failed (v2)"
  exit 1
}

kill_sandbox
