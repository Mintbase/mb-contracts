#!/usr/bin/env bash

read -r -d '' MINTARGS <<EOF
{
  "owner_id": "yadda.testnet",
  "metadata": {
    "reference": "8Z7OVdQ9jVfdkaGPlP8jW4ReXHQxAXDXsYUXEEKpGM8"
  },
  "num_to_mint": 1,
  "royalty_args": {
    "split_between": {
      "tifrel.testnet": 3334,
      "benipsen.testnet": 3333,
      "whatever123.testnet": 3333
    },
    "percentage": 3600
  },
  "split_owners": {
    "tifrel.testnet": 1600,
    "benipsen.testnet": 1600,
    "whatever123.testnet": 1600,
    "yadda.testnet": 5200
  }
}
EOF
# 10590_000000_000000_000000 -> 10.59 milliNear

read -r -d '' MINTARGS <<EOF
{
  "owner_id": "yadda.testnet",
  "metadata": {
    "reference": "8Z7OVdQ9jVfdkaGPlP8jW4ReXHQxAXDXsYUXEEKpGM8"
  },
  "num_to_mint": 1
}
EOF
# 5790_000000_000000_000000 -> 5.8 mNEAR
# token costs: 360 bytes -> 3.6 mNEAR
# metadata: 2.2 mNEAR -> 220 bytes

NEAR_ENV=testnet near call testmintingdepsoit.mintspace2.testnet \
  nft_batch_mint "$MINTARGS" --deposit 0.001 --accountId yadda.testnet
