use mb_sdk::{
    constants::{storage_stake, StorageCosts, YOCTO_PER_BYTE},
    data::store::{
        NFTContractMetadata, Royalty, SplitOwners, Token, TokenMetadata,
        TokenMetadataCompliant,
    },
    near_assert, near_panic,
    near_sdk::{
        self,
        borsh::{self, BorshDeserialize, BorshSerialize},
        collections::{LookupMap, TreeMap, UnorderedSet},
        env, ext_contract,
        json_types::{U128, U64},
        near_bindgen, AccountId, Balance, StorageUsage,
    },
};

/// Implementing approval management as [described in the Nomicon](https://nomicon.io/Standards/NonFungibleToken/ApprovalManagement).
mod approvals;
/// Implementing any methods related to burning.
mod burning;
/// Implementing core functionality of an NFT contract as [described in the Nomicon](https://nomicon.io/Standards/NonFungibleToken/Core).
mod core;
/// Implementing enumeration as [described in the Nomicon](https://nomicon.io/Standards/NonFungibleToken/Enumeration).
mod enumeration;
/// Implementing metadata as [described in the Nomicon](https://nomicon.io/Standards/NonFungibleToken/Metadata).
mod metadata;
/// Implementing any methods related to minting.
mod minting;
/// Implementing any methods related to store ownership.
mod ownership;
/// Implementing payouts as [described in the Nomicon](https://nomicon.io/Standards/NonFungibleToken/Payout).
mod payout;

// ----------------------------- smart contract ----------------------------- //

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct MintbaseStore {
    /// Accounts that are allowed to create metadata.
    pub creators: UnorderedSet<AccountId>,
    /// Initial deployment data of this Store.
    pub metadata: NFTContractMetadata,
    /// If a Minter mints more than one token at a time, all tokens will
    /// share the same `TokenMetadata`. It's more storage-efficient to store
    /// that `TokenMetadata` once, rather than to copy the data on each
    /// Token. The key is generated from `tokens_minted`. The map keeps count
    /// of how many copies of this token remain, so that the element may be
    /// dropped when the number reaches zero (ie, when tokens are burnt).
    pub token_metadata: LookupMap<
        u64,
        (
            u16,                    // number of minted tokens
            Balance,                // price
            Option<Vec<AccountId>>, // allowlist
            AccountId,              // creator
            TokenMetadata,          // actual metadata
        ),
    >,
    // Metadata ID for the next minted metadata
    pub metadata_id: u64,
    /// If a Minter mints more than one token at a time, all tokens will
    /// share the same `Royalty`. It's more storage-efficient to store that
    /// `Royalty` once, rather than to copy the data on each Token. The key
    /// is generated from `tokens_minted`. The map keeps count of how many
    /// copies of this token remain, so that the element may be dropped when
    /// the number reaches zero (ie, when tokens are burnt).
    pub token_royalty: LookupMap<u64, Royalty>,
    /// Tokens this Store has minted, excluding those that have been burned.
    pub tokens: TreeMap<(u64, u64), Token>,
    /// A mapping from each user to the tokens owned by that user. The owner
    /// of the token is also stored on the token itself.
    pub tokens_per_owner: LookupMap<AccountId, UnorderedSet<(u64, u64)>>,
    /// DEPRECATED. Kept to avoid storage migrations.
    ///
    /// A map from a token_id of a token on THIS contract to a set of tokens,
    /// that may be on ANY contract. If the owned-token is on this contract,
    /// the id will have format "<u64>". If the token is on another contract,
    /// the token will have format "<u64>:account_id"
    pub composeables: LookupMap<String, UnorderedSet<String>>,
    /// Lookup map for next token ID to mint for a given metadata ID
    pub next_token_id: LookupMap<u64, u64>,
    /// The number of tokens this `Store` has minted. Used to generate
    /// `TokenId`s.
    pub tokens_minted: u64,
    /// The number of tokens this `Store` has burned.
    pub tokens_burned: u64,
    /// The number of tokens approved (listed) by this `Store`. Used to index
    /// listings and approvals. List ID format: `list_nonce:token_key`
    pub num_approved: u64,
    /// The owner of the Contract.
    pub owner_id: AccountId,
    /// The Near-denominated price-per-byte of storage, and associated
    /// contract storage costs. As of April 2021, the price per bytes is set
    /// to 10^19, but this may change in the future, thus this
    /// future-proofing field.
    pub storage_costs: StorageCosts,
    /// DEPRECATED. Kept to avoid storage migrations.
    ///
    /// If false, disallow users to call `nft_move`.
    pub allow_moves: bool,
    /// Possibly limit minting to this number of tokens, cannot be changed once
    /// set
    pub minting_cap: Option<u64>,
}

impl Default for MintbaseStore {
    fn default() -> Self {
        env::panic_str("no default")
    }
}

#[near_bindgen]
impl MintbaseStore {
    /// Create a new `Store`. `new` validates the `store_description`.
    ///
    /// The `Store` is initialized with the owner as a `minter`.
    #[init]
    pub fn new(metadata: NFTContractMetadata, owner_id: AccountId) -> Self {
        let mut creators = UnorderedSet::new(b"a".to_vec());
        creators.insert(&owner_id);

        Self {
            creators,
            metadata,
            metadata_id: 0,
            token_metadata: LookupMap::new(b"b".to_vec()),
            token_royalty: LookupMap::new(b"c".to_vec()),
            tokens: TreeMap::new(b"d".to_vec()),
            tokens_per_owner: LookupMap::new(b"e".to_vec()),
            composeables: LookupMap::new(b"f".to_vec()),
            next_token_id: LookupMap::new(b"g".to_vec()),
            tokens_minted: 0,
            tokens_burned: 0,
            num_approved: 0,
            owner_id,
            storage_costs: StorageCosts::new(YOCTO_PER_BYTE), // 10^19
            allow_moves: true,
            minting_cap: None,
        }
    }

    // -------------------------- change methods ---------------------------
    // -------------------------- view methods -----------------------------

    /// A non-indexed implementation. `from_index` and `limit are removed, so as
    /// to support the:
    ///
    /// `tokens_per_owner: LookupMap<AccountId, UnorderedSet<TokenId>>`
    ///
    /// type. They may be used in an implementation if the type is instead:
    ///
    /// `tokens_per_owner: LookupMap<AccountId, Vector<TokenId>>`
    pub fn nft_tokens_for_owner_set(
        &self,
        account_id: AccountId,
    ) -> Vec<String> {
        self.tokens_per_owner
            .get(&account_id)
            .expect("no tokens")
            .iter()
            .map(fmt_token_id)
            .collect()
    }

    /// Get total count of minted NFTs on this smart contracts. Can be used to
    /// predict next token ID.
    pub fn get_tokens_minted(&self) -> U64 {
        self.tokens_minted.into()
    }

    /// Get total count of burned NFTs on this smart contracts.
    pub fn get_tokens_burned(&self) -> U64 {
        self.tokens_burned.into()
    }

    /// Get count of all issued approvals ever. Can be used to predict next
    /// approval ID.
    pub fn get_num_approved(&self) -> u64 {
        self.num_approved
    }

    /// Get maximum number of minted tokens on this contract
    pub fn get_minting_cap(&self) -> Option<u64> {
        self.minting_cap
    }

    /// Get status of open minting enablement
    pub fn get_open_creating(&self) -> bool {
        self.creators.is_empty()
    }

    // -------------------------- private methods --------------------------

    /// Contract metadata and methods in the API may be updated. All other
    /// elements of the state should be copied over. This method may only be
    /// called by the holder of the Store public key, in this case the
    /// Factory.
    #[private]
    #[init(ignore_state)]
    pub fn migrate(metadata: NFTContractMetadata) -> Self {
        let old = env::state_read().expect("ohno ohno state");
        Self { metadata, ..old }
    }

    // -------------------------- internal methods -------------------------

    /// Internal
    /// Transfer a token_id from one account's owned-token-set to another's.
    /// Callers of this method MUST validate that `from` owns the token before
    /// calling this method.
    ///
    /// If `to` is None, the tokens are either being burned or composed.
    ///
    /// If `from` is None, the tokens are being uncomposed.
    ///
    /// If neither are None, the tokens are being transferred.
    fn update_tokens_per_owner(
        &mut self,
        token_id: (u64, u64),
        from: Option<AccountId>,
        to: Option<AccountId>,
    ) {
        if let Some(from) = from {
            let mut old_owner_owned_set =
                self.tokens_per_owner.get(&from).unwrap();
            old_owner_owned_set.remove(&token_id);
            if old_owner_owned_set.is_empty() {
                self.tokens_per_owner.remove(&from);
            } else {
                self.tokens_per_owner.insert(&from, &old_owner_owned_set);
            }
        }
        if let Some(to) = to {
            let mut new_owner_owned_set = self.get_or_make_new_owner_set(&to);
            new_owner_owned_set.insert(&token_id);
            self.tokens_per_owner.insert(&to, &new_owner_owned_set);
        }
    }

    /// If an account_id has never owned tokens on this store, we must
    /// construct an `UnorderedSet` for them. If they have owned tokens on
    /// this store, get that set.
    /// Internal
    pub(crate) fn get_or_make_new_owner_set(
        &self,
        account_id: &AccountId,
    ) -> UnorderedSet<(u64, u64)> {
        self.tokens_per_owner.get(account_id).unwrap_or_else(|| {
            let mut prefix: Vec<u8> = vec![b'j'];
            prefix.extend_from_slice(account_id.as_bytes());
            UnorderedSet::new(prefix)
        })
    }
}

// ----------------------- contract interface modules ----------------------- //

#[ext_contract(store_self)]
pub trait NonFungibleResolveTransfer {
    /// Finalize an `nft_transfer_call` chain of cross-contract calls.
    ///
    /// The `nft_transfer_call` process:
    ///
    /// 1. Sender calls `nft_transfer_call` on FT contract
    /// 2. NFT contract transfers token from sender to receiver
    /// 3. NFT contract calls `nft_on_transfer` on receiver contract
    /// 4+. [receiver contract may make other cross-contract calls]
    /// N. NFT contract resolves promise chain with `nft_resolve_transfer`, and may
    ///    transfer token back to sender
    ///
    /// Requirements:
    /// * Contract MUST forbid calls to this function by any account except self
    /// * If promise chain failed, contract MUST revert token transfer
    /// * If promise chain resolves with `true`, contract MUST return token to
    ///   `sender_id`
    ///
    /// Arguments:
    /// * `sender_id`: the sender of `ft_transfer_call`
    /// * `token_id`: the `token_id` argument given to `ft_transfer_call`
    /// * `approved_token_ids`: if using Approval Management, contract MUST provide
    ///   set of original approved accounts in this argument, and restore these
    ///   approved accounts in case of revert.
    ///
    /// Returns true if token was successfully transferred to `receiver_id`.
    ///
    /// Mild modifications from core standard, commented where applicable.
    #[private]
    fn nft_resolve_transfer(
        &mut self,
        previous_owner_id: AccountId,
        receiver_id: AccountId,
        token_id: String,
        approved_account_ids: std::collections::HashMap<AccountId, u64>,
        split_owners: Option<SplitOwners>,
    );
}

pub(crate) fn parse_token_id(s: &str) -> (u64, u64) {
    match s.split_once(':') {
        None => near_panic!(
            "Token ID needs to be of shape {{metadata_id}}:{{minted_id}}"
        ),
        Some((p, s)) => {
            let metadata_id = match p.parse() {
                Ok(m) => m,
                Err(_) => near_panic!("The metadata_id portion of the token_id {} is not a valid u64!", s)
            };
            let minted_id = match s.parse() {
                Ok(m) => m,
                Err(_) => near_panic!("The minted_id portion of the token_id {} is not a valid u64!", s)
            };
            (metadata_id, minted_id)
        }
    }
}

pub(crate) fn fmt_token_id(tuple: (u64, u64)) -> String {
    format!("{}:{}", tuple.0, tuple.1)
}
