#!/usr/bin/env bash

near delete sailgp.yadda.testnet yadda.testnet
near create-account sailgp.yadda.testnet \
    --masterAccount yadda.testnet \
    --initialBalance 4 \
    --publicKey ed25519:GCbrk27ceALxAuJAHuFWR3HXoYmjTauZe8EXSHDmoepX

near deploy sailgp.yadda.testnet wasm/mb-nft-v2.wasm \
    --initFunction new \
    --initArgs '{"metadata":{"spec":"nft-1.0.0","name":"Testing shtuff for SailGP","symbol":"SAILGP"},"owner_id":"yadda.testnet"}'
