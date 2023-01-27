# Mintbase Smart Contracts

## Setup

In order to make full use of this repo, you will need:

- A rust installation with `cargo`, stable and nightly toolchains and the
  `wasm32-unknown-unknown` target to compile the smart contracts
- `wasm-opt` to post-process smart contracts. This tool is provided by the
  `binaryen` port
- NodeJS and NPM for testing

The script `scripts/setup.sh` should assist to get the right setup, and also
creates some necessary folders.

## Scripts

- `scripts/setup.sh`: Assists in setting up the repo and makes sure
  prerequisites are met.
- `scripts/build.sh`: Checks and builds all smart contracts
- `scripts/test.sh`: Tests the built smart contracts

## Components

### Store

Located in `mb-store`, this is the Mintbase NFT contract. It holds all
information regarding tokens, their metadata, ownership, and royalties.

### Factory

Located in `mb-factory`, this allows deploying a Mintbase Store.

### Legacy market

Located in `mb-legacy-market`, this smart contract uses NFT approvals to create
listings and processes offers. You can create simple sale or rolling auction
listings, but this market is restricted to stringified integers as token IDs.
Due to a large number of listings, storage migrations are deemed as not
feasible. The eventual goal is a replacement with the interop market for simple
sales (see below), and a not-yet-written interop market for rolling auctions.

### Interop market

Located in `mb-interop-market`, this smart contract uses NFT approvals to create
listings and processes offers. You can only create simple sale listings, but
this market does not make any assumptions about the token ID format. This market
allows using an `affiliate_id` on offers, providing a business model and
revenue-sharing between Mintbase and developers using the Mintbase
infrastructure.
