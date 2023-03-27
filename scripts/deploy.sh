#!/usr/bin/env bash

[[ -n "$1" ]] && NETWORK="$1"

case "$NETWORK" in
mainnet)
  FACTORY="mintbase1.near"
  OLD_MARKET="market.mintbase1.near"
  NEW_MARKET="simple.market.mintbase1.near"
  ;;
testnet)
  FACTORY="mintspace2.testnet"
  OLD_MARKET="market.mintspace2.testnet"
  NEW_MARKET="market-v2-beta.mintspace2.testnet"
  ;;
*)
  echo "Network must be either mainnet or testnet"
  exit 1
  ;;
esac

NEAR_ENV="$NETWORK" near deploy "$OLD_MARKET" wasm/legacy-market.wasm
NEAR_ENV="$NETWORK" near deploy "$NEW_MARKET" wasm/interop-market.wasm

# TODO: deploy factory and smart contracts
