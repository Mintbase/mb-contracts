use std::{
    collections::HashMap,
    fmt,
};

use near_sdk::{
    borsh::{
        self,
        BorshDeserialize,
        BorshSerialize,
    },
    json_types::{
        Base64VecU8,
        U128,
    },
    serde::{
        ser::Serializer,
        Deserialize,
        Serialize,
    },
    AccountId,
    Balance,
};

use crate::utils::{
    SafeFraction,
    TokenKey,
};

// ------------------------ token and token metadata ------------------------ //
/// Supports NEP-171, 177, 178, 181. Ref:
/// https://github.com/near/NEPs/blob/master/specs/Standards/NonFungibleToken/Core.md
#[derive(Clone, BorshDeserialize, BorshSerialize, Deserialize, Serialize)]
pub struct Token {
    /// The id of this token on this `Store`. Not unique across `Store`s.
    /// `token_id`s count up from 0. Ref: https://github.com/near/NEPs/discussions/171
    pub id: u64,
    /// The current owner of this token. Either an account_id or a token_id (if composed).
    pub owner_id: Owner,
    /// Ref:
    /// https://github.com/near/NEPs/blob/master/specs/Standards/NonFungibleToken/ApprovalManagement.md
    /// Set of accounts that may transfer this token, other than the owner.
    pub approvals: HashMap<AccountId, u64>,
    /// The metadata content for this token is stored in the Contract
    /// `token_metadata` field, to avoid duplication of metadata across tokens.
    /// Use metadata_id to lookup the metadata. `Metadata`s is permanently set
    /// when the token is minted.
    pub metadata_id: u64,
    /// The Royalty for this token is stored in the Contract `token_royalty`
    /// field, to avoid duplication across tokens. Use royalty_id to lookup the
    /// royalty. `Royalty`s are permanently set when the token is minted.
    pub royalty_id: Option<u64>,
    /// Feature for owner of this token to split the token ownership accross
    /// several accounts.
    pub split_owners: Option<SplitOwners>,
    /// The account that minted this token.
    pub minter: AccountId,
    /// Non-nil if Token is loaned out. While token is loaned, disallow
    /// transfers, approvals, revokes, etc. for the token, except from the
    /// approved loan contract. Mark this field with the address of the loan
    /// contract. See neps::loan for more.
    pub loan: Option<Loan>,
    /// Composablility metrics for this token
    pub composable_stats: ComposableStats,
    /// If the token originated on another contract and was `nft_move`d to
    /// this contract, this field will be non-nil.
    pub origin_key: Option<TokenKey>,
}

impl Token {
    /// - `metadata` validation performed in `TokenMetadataArgs::new`
    /// - `royalty` validation performed in `Royalty::new`
    pub fn new(
        owner_id: AccountId,
        token_id: u64,
        metadata_id: u64,
        royalty_id: Option<u64>,
        split_owners: Option<SplitOwners>,
        minter: AccountId,
    ) -> Self {
        Self {
            owner_id: Owner::Account(owner_id),
            id: token_id,
            metadata_id,
            royalty_id,
            split_owners,
            approvals: HashMap::new(),
            minter,
            loan: None,
            composable_stats: ComposableStats::new(),
            origin_key: None,
        }
    }

    /// If the token is loaned, return the loaner as the owner.
    pub fn get_owner_or_loaner(&self) -> Owner {
        self.loan
            .as_ref()
            .map(|l| Owner::Account(l.holder.clone()))
            .unwrap_or_else(|| self.owner_id.clone())
    }

    pub fn is_pred_owner(&self) -> bool {
        self.is_owned_by(&near_sdk::env::predecessor_account_id())
    }

    pub fn is_owned_by(&self, account_id: &AccountId) -> bool {
        self.owner_id.to_string() == account_id.to_string()
    }

    pub fn is_loaned(&self) -> bool {
        self.loan.is_some()
    }

    pub fn id_tuple(&self) -> (u64, u64) {
        (self.metadata_id, self.id)
    }

    pub fn fmt_id(&self) -> String {
        format!("{}:{}", self.metadata_id, self.id)
    }
}

// Supports NEP-171, 177, 178, 181. Ref:
/// https://github.com/near/NEPs/blob/master/specs/Standards/NonFungibleToken/Core.md
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TokenCompliant {
    /// The id of this token on this `Store`. Not unique across `Store`s.
    /// `token_id`s count up from 0. Ref: https://github.com/near/NEPs/discussions/171
    pub token_id: String,
    /// The current owner of this token. Either an account_id or a token_id (if composed).
    pub owner_id: Owner,
    /// Ref:
    /// https://github.com/near/NEPs/blob/master/specs/Standards/NonFungibleToken/ApprovalManagement.md
    /// Set of accounts that may transfer this token, other than the owner.
    pub approved_account_ids: HashMap<AccountId, u64>,
    /// The metadata content for this token is stored in the Contract
    /// `token_metadata` field, to avoid duplication of metadata across tokens.
    /// Use metadata_id to lookup the metadata. `Metadata`s is permanently set
    /// when the token is minted.
    pub metadata: TokenMetadataCompliant,
    /// The Royalty for this token is stored in the Contract `token_royalty`
    /// field, to avoid duplication across tokens. Use royalty_id to lookup the
    /// royalty. `Royalty`s are permanently set when the token is minted.
    pub royalty: Option<Royalty>,
    /// Feature for owner of this token to split the token ownership accross
    /// several accounts.
    pub split_owners: Option<SplitOwners>,
    /// The account that minted this token.
    pub minter: AccountId,
    /// Non-nil if Token is loaned out. While token is loaned, disallow
    /// transfers, approvals, revokes, etc. for the token, except from the
    /// approved loan contract. Mark this field with the address of the loan
    /// contract. See neps::loan for more.
    pub loan: Option<Loan>,
    /// Composeablility metrics for this token
    pub composable_stats: ComposableStats,
    /// If the token originated on another contract and was `nft_move`d to
    /// this contract, this field will be non-nil.
    pub origin_key: Option<TokenKey>,
}

// -------- token metadata
// NON-COMPLIANT https://github.com/near/NEPs/blob/master/specs/Standards/NonFungibleToken/Metadata.md
/// ref:
/// https://github.com/near/NEPs/blob/master/specs/Standards/NonFungibleToken/Metadata.md
#[derive(
    Clone, Debug, Deserialize, Serialize, BorshDeserialize, BorshSerialize,
)]
pub struct TokenMetadata {
    /// the Title for this token. ex. "Arch Nemesis: Mail Carrier" or "Parcel 5055"
    pub title: Option<String>,
    /// free-form description of this token.
    pub description: Option<String>,
    /// URL to associated media, preferably to decentralized, content-addressed storage
    pub media: Option<String>,
    /// Base64-encoded sha256 hash of content referenced by the `media` field.
    /// Required if `media` is included.
    pub media_hash: Option<Base64VecU8>,
    /// number of copies of this set of metadata in existence when token was minted.
    pub copies: Option<u16>,
    /// ISO 8601 datetime when token expires.
    pub expires_at: Option<String>,
    /// ISO 8601 datetime when token starts being valid.
    pub starts_at: Option<String>,
    /// When token was last updated, Unix epoch in milliseconds
    pub extra: Option<String>,
    /// URL to an off-chain JSON file with more info. The Mintbase Indexer refers
    /// to this field as `thing_id` or sometimes, `meta_id`.
    pub reference: Option<String>,
    /// Base64-encoded sha256 hash of JSON from reference field. Required if
    /// `reference` is included.
    pub reference_hash: Option<Base64VecU8>,
}

// NON-COMPLIANT https://github.com/near/NEPs/blob/master/specs/Standards/NonFungibleToken/Metadata.md
/// ref:
/// https://github.com/near/NEPs/blob/master/specs/Standards/NonFungibleToken/Metadata.md
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TokenMetadataCompliant {
    /// the Title for this token. ex. "Arch Nemesis: Mail Carrier" or "Parcel 5055"
    pub title: Option<String>,
    /// free-form description of this token.
    pub description: Option<String>,
    /// URL to associated media, preferably to decentralized, content-addressed storage
    pub media: Option<String>,
    /// Base64-encoded sha256 hash of content referenced by the `media` field.
    /// Required if `media` is included.
    pub media_hash: Option<Base64VecU8>,
    /// number of copies of this set of metadata in existence when token was minted.
    pub copies: Option<u16>,
    /// When token was issued or minted, Unix epoch in milliseconds
    pub issued_at: Option<String>,
    /// ISO 8601 datetime when token expires.
    pub expires_at: Option<String>,
    /// ISO 8601 datetime when token starts being valid.
    pub starts_at: Option<String>,
    /// When token was last updated, Unix epoch in milliseconds
    pub updated_at: Option<String>,
    /// Brief description of what this thing is. Used by the mintbase indexer as "memo".
    pub extra: Option<String>,
    /// URL to an off-chain JSON file with more info. The Mintbase Indexer refers
    /// to this field as `thing_id` or sometimes, `meta_id`.
    pub reference: Option<String>,
    /// Base64-encoded sha256 hash of JSON from reference field. Required if
    /// `reference` is included.
    pub reference_hash: Option<Base64VecU8>,
}

impl From<TokenMetadata> for TokenMetadataCompliant {
    fn from(metadata: TokenMetadata) -> TokenMetadataCompliant {
        TokenMetadataCompliant {
            title: metadata.title,
            description: metadata.description,
            media: metadata.media,
            media_hash: metadata.media_hash,
            copies: metadata.copies,
            issued_at: None,
            expires_at: metadata.expires_at,
            starts_at: metadata.starts_at,
            updated_at: None,
            extra: metadata.extra,
            reference: metadata.reference,
            reference_hash: metadata.reference_hash,
        }
    }
}

/// Metadata and meta-metadata for tokens minted on store v2
#[derive(Clone, BorshDeserialize, BorshSerialize)]
pub struct MintingMetadata {
    /// Number of tokens minted on this metadata
    pub minted: u32,
    /// Number of tokens minted on this metadata
    pub burned: u32,
    /// Price required to mint on this metadata
    pub price: near_sdk::Balance,
    /// How the minting price is to be paid
    pub payment_method: MintingPayment,
    /// Maximum amount of tokens allowed to be minted, no restrictions if `None`
    pub max_supply: Option<u32>,
    /// Accounts allowed to mint on this metadata, no restrictions if `None`,
    /// the boolean is used to indicate if an account has already minted their
    /// token in case that `unique_minters` is true.
    pub allowlist: Option<Vec<(AccountId, bool)>>,
    /// Are the allowed accounts
    pub unique_minters: bool,
    /// Earliest possible timestamp to mint, no restrictions if `None`. Timestamp
    /// in number of non-leap nanoseconds since 1970-01-01 00:00:00 UTC.
    pub starts_at: Option<u64>,
    /// Latest possible timestamp to mint, no restrictions if `None`. Timestamp
    /// in number of non-leap nanoseconds since 1970-01-01 00:00:00 UTC.
    pub expires_at: Option<u64>,
    /// Creator of this metadata
    pub creator: AccountId,
    /// A locked metadata may not be updated. By default all metadata is
    /// locked. To enable dynamic NFTs metadata may be unlocked on mint.
    /// Locking metadata is irreversible.
    pub is_locked: bool,
    /// The actual metadata
    pub metadata: TokenMetadata,
}

#[derive(Clone, BorshDeserialize, BorshSerialize)]
pub enum MintingPayment {
    Near,
    Ft(near_sdk::AccountId),
}

impl MintingPayment {
    pub fn is_near(&self) -> bool {
        matches!(self, Self::Near)
    }

    pub fn get_ft_contract_id(&self) -> Option<&AccountId> {
        match self {
            Self::Near => None,
            Self::Ft(id) => Some(id),
        }
    }

    pub fn create_payment_promise(
        &self,
        receiver_id: AccountId,
        amount: Balance,
    ) -> near_sdk::Promise {
        match self {
            Self::Near => near_sdk::Promise::new(receiver_id).transfer(amount),
            Self::Ft(ft_contract_id) => {
                crate::interfaces::ext_ft::ext(ft_contract_id.to_owned())
                    .with_attached_deposit(1)
                    .ft_transfer(receiver_id, amount.into(), None)
            }
        }
    }
}

// -------- token owner
// This is mostly kept here to avoid storage migrations, but this should always
// be the `Account` variant.
#[derive(Deserialize, Clone, Debug, BorshDeserialize, BorshSerialize)]
pub enum Owner {
    /// Standard pattern: owned by a user.
    Account(AccountId),
    /// Compose pattern: owned by a token on this contract.
    TokenId(u64),
    /// Cross-compose pattern: owned by a token on another contract.
    CrossKey(crate::utils::TokenKey),
    /// Lock: temporarily locked until some callback returns.
    Lock(AccountId),
}

impl Serialize for Owner {
    fn serialize<S: Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{}", self))
    }
}

impl fmt::Display for Owner {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Owner::Account(s) => write!(f, "{}", s),
            Owner::TokenId(n) => write!(f, "{}", n),
            Owner::CrossKey(key) => write!(f, "{}", key),
            Owner::Lock(_) => panic!("locked"),
        }
    }
}

// -------- loan
// This is only kept here to avoid storage migrations, it is no longer used
#[derive(
    Clone, Debug, Deserialize, Serialize, BorshDeserialize, BorshSerialize,
)]
pub struct Loan {
    pub holder: AccountId,
    pub loan_contract: AccountId,
}

impl Loan {
    pub fn new(holder: AccountId, loan_contract: AccountId) -> Self {
        Self {
            holder,
            loan_contract,
        }
    }
}

// -------- composability
// This is only kept here to avoid storage migrations, it is no longer used
/// To enable recursive composeability, need to track:
/// 1. How many levels deep a token is recursively composed
/// 2. Whether and how many cross-contract children a token has.
///
/// Tracking depth limits potential bugs around recursive ownership
/// consuming excessive amounts of gas.
///
/// Tracking the number of cross-contract children a token has prevents
/// breaking of the Only-One-Cross-Linkage Invariant.
#[derive(
    Clone, Debug, Deserialize, Serialize, BorshDeserialize, BorshSerialize,
)]
pub struct ComposableStats {
    /// How deep this token is in a chain of composeability on THIS contract.
    /// If this token is cross-composed, it's depth will STILL be 0. `depth`
    /// equal to the parent's `depth`+1. If this is a top level token, this
    /// number is 0.
    pub local_depth: u8,
    /// How many cross contract children this token has, direct AND indirect.
    /// That is, any parent's `cross_contract_children` value equals the sum
    /// of of its children's values. If this number is non-zero, deny calls
    /// to `nft_cross_compose`.
    pub cross_contract_children: u8,
}

impl ComposableStats {
    pub(super) fn new() -> Self {
        Self {
            local_depth: 0,
            cross_contract_children: 0,
        }
    }
}

// ----------------------------- store metadata ----------------------------- //
#[derive(
    Clone, Debug, Deserialize, Serialize, BorshDeserialize, BorshSerialize,
)]
pub struct NFTContractMetadata {
    /// a version like "nft-1.0.0"
    pub spec: String,
    /// Subaccount of this `Store`. `Factory` is the super-account.
    pub name: String,
    /// Symbol of the Store. Up to 6 chars.
    pub symbol: String,
    /// a small image associated with this `Store`.
    pub icon: Option<String>,
    /// Centralized gateway known to have reliable access to decentralized storage
    /// assets referenced by `reference` or `media` URLs
    pub base_uri: Option<String>,
    /// URL to a JSON file with more info
    pub reference: Option<String>,
    /// Base64-encoded sha256 hash of the JSON file pointed at by the reference
    /// field. Required if `reference` is included.
    pub reference_hash: Option<Base64VecU8>,
}

impl Default for NFTContractMetadata {
    fn default() -> Self {
        Self {
            spec: "".to_string(),
            name: "".to_string(),
            symbol: "".to_string(),
            icon: None,
            base_uri: None,
            reference: None,
            reference_hash: None,
        }
    }
}

// /// ref:
// /// https://github.com/near/NEPs/blob/master/specs/Standards/NonFungibleToken/Metadata.md
// pub trait NonFungibleContractMetadata {
//     /// Get the metadata for this `Store`.
//     fn nft_metadata(&self) -> &NFTContractMetadata;
// }

// ------------------------ splits/royalties/payouts ------------------------ //

/// Whom to pay. Generated from `OwnershipFractions`.
#[derive(Serialize, Deserialize)]
pub struct Payout {
    pub payout: HashMap<AccountId, U128>,
}

pub type SplitBetweenUnparsed = HashMap<AccountId, u32>;
pub type SplitBetween = HashMap<near_sdk::AccountId, SafeFraction>;

/// A representation of the splitting of ownership of the Token. Percentages
/// must add to 1. On purchase of the `Token`, the value of the transaction
/// (minus royalty percentage) will be paid out to each account in `SplitOwners`
/// mapping. The `SplitOwner` field on the `Token` will be set to `None` after
/// each transfer of the token.
#[derive(
    Clone, Debug, Deserialize, Serialize, BorshDeserialize, BorshSerialize,
)]
pub struct SplitOwners {
    pub split_between: HashMap<AccountId, SafeFraction>,
}

impl SplitOwners {
    pub fn new(split_between: HashMap<near_sdk::AccountId, u32>) -> Self {
        crate::near_assert!(
            split_between.len() >= 2,
            "Requires at least two accounts to split revenue"
        );
        // validate args
        let mut sum: u32 = 0;
        let split_between: HashMap<AccountId, SafeFraction> = split_between
            .into_iter()
            .map(|(addr, numerator)| {
                crate::near_assert!(
                    near_sdk::env::is_valid_account_id(addr.as_bytes()),
                    "{} is not a valid account ID on NEAR",
                    addr
                );
                let sf = SafeFraction::new(numerator);
                sum += sf.numerator;
                (addr, sf)
            })
            .collect();
        crate::near_assert!(
            sum == 10_000,
            "Splits numerators must sum up to 10_000"
        );

        Self { split_between }
    }
}

/// A representation of permanent partial ownership of a Token's revenues.
/// Percentages must add to 10,000. On purchase of the `Token`, a percentage of
/// the value of the transaction will be paid out to each account in the
/// `Royalty` mapping. `Royalty` field once set can NEVER change for this
/// `Token`, even if removed and re-added.
#[derive(
    PartialEq,
    Eq,
    BorshDeserialize,
    BorshSerialize,
    Clone,
    Debug,
    Deserialize,
    Serialize,
)]
pub struct Royalty {
    /// Mapping of addresses to relative percentages of the overall royalty percentage
    pub split_between: HashMap<near_sdk::AccountId, SafeFraction>,
    /// The overall royalty percentage taken
    pub percentage: SafeFraction,
}

/// Stable
impl Royalty {
    /// Validates all arguments. Addresses must be valid and percentages must be
    /// within accepted values. Hashmap percentages must add to 10000.
    pub fn new(royalty_args: RoyaltyArgs) -> Self {
        let percentage = royalty_args.percentage;
        let split_between = royalty_args.split_between;

        crate::near_assert!(
            percentage <= crate::constants::ROYALTY_UPPER_LIMIT,
            "Royalties must not exceed 50% of a sale",
        );
        crate::near_assert!(
            percentage > 0,
            "Royalty percentage cannot be zero"
        );
        crate::near_assert!(
            !split_between.is_empty(),
            "Royalty mapping may not be empty"
        );

        let mut sum: u32 = 0;
        let split_between: SplitBetween = split_between
            .into_iter()
            .map(|(addr, numerator)| {
                crate::near_assert!(
                    near_sdk::env::is_valid_account_id(addr.as_bytes()),
                    "{} is not a valid account ID on NEAR",
                    addr
                );
                crate::near_assert!(
                    numerator > 0,
                    "Royalty for {} cannot be zero",
                    addr
                );
                let sf = SafeFraction::new(numerator);
                sum += sf.numerator;
                (addr, sf)
            })
            .collect();
        crate::near_assert!(
            sum == 10_000,
            "Fractions need to add up to 10_000"
        );

        Self {
            percentage: SafeFraction::new(percentage),
            split_between,
        }
    }
}

/// Unparsed pre-image of a Royalty struct. Used in `Store::mint_tokens`.
#[derive(Clone, Deserialize, Serialize)]
pub struct RoyaltyArgs {
    pub split_between: SplitBetweenUnparsed,
    pub percentage: u32,
}

// ---------------------- args for initializing store ----------------------- //
#[derive(
    Clone, Debug, Deserialize, Serialize, BorshDeserialize, BorshSerialize,
)]
pub struct StoreInitArgs {
    pub metadata: NFTContractMetadata,
    pub owner_id: AccountId,
}
