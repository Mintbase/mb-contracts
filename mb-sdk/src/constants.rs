use near_sdk::{
    borsh::{
        self,
        BorshDeserialize,
        BorshSerialize,
    },
    json_types::U128,
    Balance,
};

/// Current price for one byte of on-chain storage, denominated in yoctoNEAR.
pub const YOCTO_PER_BYTE: Balance = 10_000_000_000_000_000_000;

/// One yoctoNEAR, commonly required to verify that a method was called with a
/// full-access key.
pub const ONE_YOCTO: Balance = 1;

/// The argument for non-payable cross contract calls.
/// ref: https://github.com/near/core-contracts/blob/master/staking-pool/src/lib.rs#L26
pub const NO_DEPOSIT: Balance = 0;

/// Miniscule minting fee (1 milliNEAR) to allow tracking by DappRadar
pub const MINTING_FEE: Balance = 1_000_000_000_000_000_000_000;

/// Maximum number of tokens to be minted on unlocked (dynamic) metadata
pub const DYNAMIC_METADATA_MAX_TOKENS: u32 = 1000;

/// This module holds gas costs for common operations
pub mod gas {
    use near_sdk::Gas;

    const fn tgas(n: u64) -> Gas {
        Gas(n * 10u64.pow(12))
    }

    /// Gas requirements for resolving a payout struct.
    pub const PAYOUT_RESOLVE: Gas = tgas(30);

    /// Gas requirements for transferring an NFT and obtaining the payout.
    pub const NFT_TRANSFER_PAYOUT: Gas = tgas(15);

    /// Gas requirements for creating a store.
    pub const CREATE_STORE: Gas = tgas(65 + 5);

    /// Gas requirements for
    pub const ON_CREATE_CALLBACK: Gas = tgas(10);

    /// Gas requirements for
    pub const NFT_BATCH_APPROVE: Gas = tgas(100);

    // ref: https://github.com/near-apps/nft-market/blob/main/contracts/nft-simple/src/nft_core.rs
    /// Gas requirements for resolving a `nft_transfer_call` XCC
    pub const RESOLVE_TRANSFER: Gas = tgas(10);

    /// Gas requirements for `nft_transfer_call`
    pub const NFT_TRANSFER_CALL: Gas = tgas(35);

    /// Gas requirements for `nft_transfer_call`
    pub const NFT_ON_APPROVE: Gas = tgas(25);

    pub const FT_TRANSFER: Gas = tgas(15);
}

pub mod storage_bytes {
    use near_sdk::StorageUsage;
    // FIXME: make a difference between store v1 and store v2
    /// Storage bytes that a raw store occupies, about 350 KB.
    pub const STORE: StorageUsage = 400_000;

    /// Storage bytes for a maximum size token without any metadata and without
    /// any royalties.
    pub const TOKEN: StorageUsage = 360;

    /// Storage bytes for some common components:
    ///
    /// - a single royalty
    /// - a single approval
    /// - an entry in the `tokens_per_account` map
    /// - an entry in the `composables` map
    pub const COMMON: StorageUsage = 80;
}

pub mod storage_stake {
    use near_sdk::Balance;

    use super::YOCTO_PER_BYTE;

    const fn bytes_to_stake(bytes: u64) -> Balance {
        (bytes as Balance) * YOCTO_PER_BYTE
    }

    /// Storage stake required to deploy a store.
    pub const STORE: Balance = bytes_to_stake(super::storage_bytes::STORE);

    /// Storage stake required to hold a maximum size token without any metadata
    /// and without any royalties.
    pub const TOKEN: Balance = bytes_to_stake(super::storage_bytes::TOKEN);

    /// Storage stake required for some common components:
    ///
    /// - adding a single royalty
    /// - adding a single approval
    /// - adding a new entry to the `tokens_per_account` map
    /// - adding a new entry to the `composables` map
    pub const COMMON: Balance = bytes_to_stake(super::storage_bytes::COMMON);

    /// Require 0.1 NEAR of storage stake to remain unused.
    pub const CUSHION: Balance = 10u128.pow(23);
}

/// Royalty upper limit is 50%.
pub const ROYALTY_UPPER_LIMIT: u32 = 5000;

/// Maximum payout (royalties + splits) participants to process (NFT v1)
pub const MAX_LEN_PAYOUT: u32 = 50;

/// Maximum royalties participants to process (NFT v2)
pub const MAX_LEN_ROYALTIES: u32 = 25;

/// Maximum splits participants to process (NFT v2)
pub const MAX_LEN_SPLITS: u32 = 25;

/// Maximum allowed approvals per token to prevent panics on revoking all, most
/// notably during transfers.
pub const MAX_APPROVALS_PER_TOKEN: u64 = 100;

/// Minimum storage stake required to allow updates
pub const MINIMUM_FREE_STORAGE_STAKE: near_sdk::Balance = 50 * YOCTO_PER_BYTE;

#[derive(BorshDeserialize, BorshSerialize)]
pub struct StorageCosts {
    /// The Near-denominated price-per-byte of storage. As of April 2021, the
    /// price per bytes is set by default to 10^19, but this may change in the
    /// future, thus this future-proofing field.
    pub storage_price_per_byte: u128,
    /// 80 bytes as a Near price. Used for:
    /// - a single royalty
    /// - a single approval
    /// - adding a new entry to the `tokens_per_account` map
    /// - adding a new entry to the `composables` map
    pub common: u128,
    /// base cost of storing a single NFT
    pub token: u128,
}

impl StorageCosts {
    pub fn new(storage_price_per_byte: u128) -> Self {
        Self {
            storage_price_per_byte,
            // common: storage_price_per_byte * 80_u64 as u128,
            common: storage_stake::COMMON,
            // token: storage_price_per_byte * 360_u64 as u128,
            token: storage_stake::TOKEN,
        }
    }
}

#[derive(near_sdk::serde::Serialize)]
pub struct StorageCostsJson {
    /// The Near-denominated price-per-byte of storage. As of April 2021, the
    /// price per bytes is set by default to 10^19, but this may change in the
    /// future, thus this future-proofing field.
    pub storage_price_per_byte: U128,
    /// 80 bytes as a Near price. Used for:
    /// - a single royalty
    /// - a single approval
    /// - adding a new entry to the `tokens_per_account` map
    /// - adding a new entry to the `composables` map
    pub common: U128,
    /// base cost of storing a single NFT
    pub token: U128,
}

impl From<&StorageCosts> for StorageCostsJson {
    fn from(storage_costs: &StorageCosts) -> Self {
        StorageCostsJson {
            storage_price_per_byte: storage_costs.storage_price_per_byte.into(),
            common: storage_costs.common.into(),
            token: storage_costs.token.into(),
        }
    }
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct StorageCostsMarket {
    /// The Near-denominated price-per-byte of storage. As of April 2021, the
    /// price per bytes is set by default to 10^19, but this may change in the
    /// future, thus this future-proofing field.
    pub storage_price_per_byte: u128,
    pub list: u128,
}

impl StorageCostsMarket {
    pub fn new(storage_price_per_byte: u128) -> Self {
        Self {
            storage_price_per_byte,
            list: storage_stake::TOKEN,
        }
    }
}
