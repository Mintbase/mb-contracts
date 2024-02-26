use std::convert::TryInto;

use mb_sdk::{
    constants::{
        DYNAMIC_METADATA_MAX_TOKENS,
        MAX_LEN_ROYALTIES,
        MAX_LEN_SPLITS,
        MINIMUM_FREE_STORAGE_STAKE,
        MINTING_FEE,
    },
    data::store::{
        ComposableStats,
        Royalty,
        RoyaltyArgs,
        SplitBetweenUnparsed,
        TokenMetadata,
    },
    events::store::{
        CreateMetadataData,
        MbStoreChangeSettingDataV020,
        NftMintLog,
        NftMintLogMemo,
    },
    near_assert,
    near_panic,
    near_sdk::{
        self,
        assert_one_yocto,
        env,
        near_bindgen,
        serde_json,
        AccountId,
        Balance,
        Promise,
    },
};

use crate::*;

#[near_bindgen]
impl MintbaseStore {
    // -------------------------- change methods ---------------------------
    #[payable]
    pub fn create_metadata(
        &mut self,
        metadata: TokenMetadata,
        metadata_id: Option<U64>,
        royalty_args: Option<RoyaltyArgs>,
        minters_allowlist: Option<Vec<AccountId>>,
        max_supply: Option<u32>,
        last_possible_mint: Option<U64>,
        is_dynamic: Option<bool>,
        price: U128,
    ) -> String {
        // metadata ID: either predefined (must not conflict with existing), or
        // increasing the counter for it
        let metadata_id = self.get_metadata_id(metadata_id);

        let is_locked = !is_dynamic.unwrap_or(false);

        // creator needs to be allowed to create metadata on this smart contract
        let creator = env::predecessor_account_id();
        near_assert!(self.creators.contains(&creator), "{}", creator);

        // validate metadata
        validate_metadata(&metadata);

        // validate royalties
        let roy_len = royalty_args
            .as_ref()
            .map(|pre_roy| {
                let len = pre_roy.split_between.len();
                len as u32
            })
            .unwrap_or(0);
        let checked_royalty = royalty_args.map(Royalty::new);
        near_assert!(
            roy_len <= MAX_LEN_ROYALTIES,
            "Number of royalty holders may not exceed {}",
            MAX_LEN_ROYALTIES
        );

        // makes sure storage is covered
        let metadata_size = borsh::to_vec(&metadata).unwrap().len() as u64;
        let expected_storage_consumption: Balance = self
            .storage_cost_to_create_metadata(
                metadata_size,
                roy_len,
                minters_allowlist.as_ref().map(|l| l.len()).unwrap_or(0) as u64,
            );
        let covered_storage = env::attached_deposit() - MINTING_FEE;
        near_assert!(
            covered_storage >= expected_storage_consumption,
            "This mint would exceed the current storage coverage of {} yoctoNEAR. Requires at least {} yoctoNEAR",
            covered_storage,
            expected_storage_consumption + MINTING_FEE
        );

        // insert metadata and royalties
        let minting_metadata = MintingMetadata {
            minted: 0,
            burned: 0,
            price: price.0,
            max_supply,
            allowlist: minters_allowlist.clone(),
            last_possible_mint: last_possible_mint.map(|t| t.0),
            creator: creator.clone(),
            is_locked,
            metadata,
        };
        self.token_metadata.insert(&metadata_id, &minting_metadata);
        checked_royalty
            .as_ref()
            .map(|r| self.token_royalty.insert(&metadata_id, r));
        self.next_token_id.insert(&metadata_id, &0);
        self.tokens.insert(
            &metadata_id,
            &TreeMap::new(format!("d{}", metadata_id).as_bytes().to_vec()),
        );

        // padding for updates required
        let used_storage_stake: Balance =
            env::storage_usage() as u128 * env::storage_byte_cost();
        let free_storage_stake: Balance =
            env::account_balance() - used_storage_stake;
        near_assert!(
            free_storage_stake > MINIMUM_FREE_STORAGE_STAKE,
            "A minimum of {} yoctoNEAR is required as free contract balance to allow updates (currently: {})",
            MINIMUM_FREE_STORAGE_STAKE,
            free_storage_stake
        );

        log_create_metadata(metadata_id, minting_metadata, checked_royalty);

        metadata_id.to_string()
    }

    #[payable]
    pub fn mint_on_metadata(
        &mut self,
        metadata_id: U64,
        owner_id: AccountId,
        num_to_mint: Option<u16>,
        token_ids: Option<Vec<U64>>,
        split_owners: Option<SplitBetweenUnparsed>,
    ) {
        let metadata_id = metadata_id.0;
        let minter = env::predecessor_account_id();

        // make sure metadata exists
        let mut minting_metadata = self.get_minting_metadata(metadata_id);

        // check if this account is allowed to mint this metadata
        if let Some(ref allowlist) = minting_metadata.allowlist {
            near_assert!(
                allowlist.contains(&minter),
                "{} is not allowed to mint this metadata",
                minter
            )
        }

        // make sure token_ids and num_to_mint are not conflicting, create valid IDs if necessary
        let (num_to_mint, token_ids) =
            self.get_token_ids(metadata_id, num_to_mint, token_ids);

        // Cannot mint more than NFTs than the threshold for dynamic metadata,
        // as that would exceed the log limit when emitting the event
        near_assert!(
            minting_metadata.is_locked
                || minting_metadata.minted + (num_to_mint as u32)
                    < DYNAMIC_METADATA_MAX_TOKENS,
            "Cannot mint more than {} tokens on dynamic metadata",
            DYNAMIC_METADATA_MAX_TOKENS
        );

        // check that splits are not too long and parse properly
        let num_splits = split_owners
            .as_ref()
            .map(|pre_split| pre_split.len() as u32)
            .unwrap_or(0);
        let split_owners = split_owners.map(SplitOwners::new);
        near_assert!(
            num_splits <= MAX_LEN_SPLITS,
            "Number of split holders may not exceed {}",
            MAX_LEN_SPLITS
        );

        // are storage deposit and price attached?
        let storage_usage = self.storage_cost_to_mint(num_to_mint, num_splits);
        let attached_deposit = env::attached_deposit();
        let min_attached_deposit = storage_usage
            + minting_metadata.price * num_to_mint as u128
            + MINTING_FEE;
        near_assert!(
            attached_deposit >= min_attached_deposit,
            "Attached deposit must cover storage usage, token price and minting fee ({})",
            min_attached_deposit
        );

        // TODO: is this still necessary with a per-token minting cap?
        if let Some(minting_cap) = self.minting_cap {
            near_assert!(
                self.tokens_minted + num_to_mint as u64 <= minting_cap,
                "This mint would exceed the smart contracts minting cap"
            );
        }

        if let Some(max_supply) = minting_metadata.max_supply {
            near_assert!(
                minting_metadata.minted + num_to_mint as u32 <= max_supply,
                "This mint would exceed the metadatas minting cap"
            );
        }

        if let Some(expiry) = minting_metadata.last_possible_mint {
            near_assert!(
                env::block_timestamp() <= expiry,
                "This metadata has expired and can no longer be minted on"
            );
        }

        // mint the tokens, store splits
        let royalty_id = match self.token_royalty.contains_key(&metadata_id) {
            true => Some(metadata_id),
            false => None,
        };
        let mut owned_set = self.get_or_make_new_owner_set(&owner_id);
        self.tokens_minted += num_to_mint as u64;
        for &id in token_ids.iter() {
            let token = Token {
                id,
                owner_id: mb_sdk::data::store::Owner::Account(owner_id.clone()),
                approvals: std::collections::HashMap::new(),
                metadata_id,
                royalty_id,
                split_owners: split_owners.clone(),
                minter: minter.clone(),
                // These fields are theoretically unused, but stay here to share
                // this type with NFT v1
                loan: None,
                composable_stats: ComposableStats {
                    local_depth: 0,
                    cross_contract_children: 0,
                },
                origin_key: None,
            };
            self.save_token(&token);
            owned_set.insert(&(metadata_id, id));
        }
        minting_metadata.minted += num_to_mint as u32;
        self.token_metadata.insert(&metadata_id, &minting_metadata);
        self.tokens_per_owner.insert(&owner_id, &owned_set);

        // emit event
        log_nft_batch_mint(
            token_ids
                .into_iter()
                .map(|id| fmt_token_id((metadata_id, id)))
                .collect(),
            minter.as_str(),
            owner_id.as_str(),
            &self.token_royalty.get(&metadata_id),
            &split_owners,
            &minting_metadata.metadata.reference,
            &minting_metadata.metadata.extra,
        );

        // payout for creator(s) and minting fee
        self.minting_payout(
            metadata_id,
            attached_deposit - storage_usage - MINTING_FEE,
            minting_metadata.creator,
        );
    }

    /// Tries to remove an acount ID from the minters list, will only fail
    /// if the owner should be removed from the minters list.
    fn revoke_creator_internal(&mut self, account_id: &AccountId) {
        near_assert!(
            *account_id != self.owner_id,
            "Owner cannot be removed from minters"
        );
        // does nothing if account_id wasn't a minter
        if self.creators.remove(account_id) {
            log_revoke_creator(account_id);
        }
    }

    /// Allows batched granting and revoking of minting rights in a single
    /// transaction. Subject to the same restrictions as `grant_minter`
    /// and `revoke_minter`.
    ///
    /// Should you include an account in both lists, it will end up becoming
    /// approved and immediately revoked in the same step.
    #[payable]
    pub fn batch_change_creators(
        &mut self,
        grant: Option<Vec<AccountId>>,
        revoke: Option<Vec<AccountId>>,
    ) {
        self.assert_store_owner();
        near_assert!(
            grant.is_some() || revoke.is_some(),
            "You need to either grant or revoke at least one account"
        );
        near_assert!(
            !self.creators.is_empty(),
            "Cannot change creators since open minting is enabled"
        );

        if let Some(grant_ids) = grant {
            for account_id in grant_ids {
                // does nothing if account_id is already a minter
                if self.creators.insert(&account_id) {
                    log_grant_creator(&account_id);
                }
            }
        }

        if let Some(revoke_ids) = revoke {
            for account_id in revoke_ids {
                self.revoke_creator_internal(&account_id)
            }
        }
    }

    /// The calling account will try to withdraw as minter from this NFT smart
    /// contract. If the calling account is not a minter on the NFT smart
    /// contract, this will still succeed but have no effect.
    #[payable]
    pub fn withdraw_creator(&mut self) {
        assert_one_yocto();
        self.revoke_creator_internal(&env::predecessor_account_id())
    }

    // -------------------------- view methods -----------------------------

    /// Check if `account_id` is a minter.
    pub fn check_is_creator(&self, account_id: AccountId) -> bool {
        self.creators.contains(&account_id)
    }

    /// Lists all account IDs that are currently allowed to mint on this
    /// contract.
    pub fn list_creators(&self) -> Vec<AccountId> {
        self.creators.iter().collect()
    }

    /// Retrieves metadata
    pub fn get_metadata(
        &self,
        metadata_id: U64,
    ) -> Option<TokenMetadataCompliant> {
        self.token_metadata
            .get(&metadata_id.0)
            .map(|minting_metadata| minting_metadata.metadata.into())
    }

    // -------------------------- private methods --------------------------
    // -------------------------- internal methods -------------------------

    /// Get the storage in bytes to create metadata each with
    /// `metadata_storage` and `len_map` royalty receivers.
    /// Internal
    fn storage_cost_to_create_metadata(
        &self,
        metadata_storage: StorageUsage,
        num_royalties: u32,
        num_minters: u64,
    ) -> near_sdk::Balance {
        // - metadata_storage
        // - minters allowlist: account_id * length
        // - creator: account_id
        // - royalties
        // - burned: 5 bytes
        // - minted: 5 bytes
        // - max_supply: 5 bytes
        // - expiry: 9 bytes
        // - price: 16 bytes
        // - is_locked: 1 bytes
        metadata_storage as u128 * self.storage_costs.storage_price_per_byte
            // create a royalty record
            + num_royalties as u128 * self.storage_costs.common
            // store the minters list
            + num_minters as u128 * self.storage_costs.account_id
            // store the creator
            + self.storage_costs.account_id
            // price, burned, minted, max_supply, expiry, is_locked
            + self.storage_costs.common
    }

    /// Get the storage in bytes to mint `num_tokens` each with
    /// `metadata_storage` and `len_map` royalty receivers.
    /// Internal
    fn storage_cost_to_mint(
        &self,
        num_tokens: u16,
        num_splits: u32,
    ) -> near_sdk::Balance {
        num_tokens as u128
            * (
                // token base storage
                self.storage_costs.token
                // dynamic split storage
                + num_splits as u128 * self.storage_costs.common
                // create an entry in tokens_per_owner
                + self.storage_costs.common
            )
    }

    fn get_metadata_id(&mut self, metadata_id: Option<U64>) -> u64 {
        match metadata_id {
            Some(U64(metadata_id)) => {
                if self.token_metadata.contains_key(&metadata_id) {
                    near_panic!("Metadata ID {} already exists", metadata_id);
                }
                metadata_id
            }
            None => {
                while self.token_metadata.contains_key(&self.metadata_id) {
                    self.metadata_id += 1;
                }
                self.metadata_id
            }
        }
    }

    fn get_token_ids(
        &self,
        metadata_id: u64,
        num_to_mint: Option<u16>,
        token_ids: Option<Vec<U64>>,
    ) -> (u16, Vec<u64>) {
        let metadata_tokens = self
            .tokens
            .get(&metadata_id)
            .expect("metadata existence was checked earlier");
        match (num_to_mint, token_ids) {
            (None, None) => near_panic!(
                "You are required to either specify num_to_mint or token_ids"
            ),
            (Some(n), None) => {
                let mut token_ids = Vec::with_capacity(n as usize);
                let mut generated = 0;
                let mut minted_id = self
                    .next_token_id
                    .get(&metadata_id)
                    .expect("metadata existence was checked earlier");

                while generated < n {
                    if !metadata_tokens.contains_key(&minted_id) {
                        token_ids.push(minted_id);
                        generated += 1;
                    }
                    minted_id += 1;
                }
                (n, token_ids)
            }
            (None, Some(ids)) => (
                ids.len() as u16,
                self.process_tokens_ids_arg(metadata_id, &metadata_tokens, ids),
            ),
            (Some(n), Some(ids)) => {
                near_assert!(n == ids.len() as u16, "num_to_mint does not match the number of specified token IDs");
                let ids = self.process_tokens_ids_arg(
                    metadata_id,
                    &metadata_tokens,
                    ids,
                );
                (n, ids)
            }
        }
    }

    fn process_tokens_ids_arg(
        &self,
        metadata_id: u64,
        metadata_tokens: &TreeMap<u64, Token>,
        token_ids: Vec<U64>,
    ) -> Vec<u64> {
        token_ids
            .into_iter()
            .map(|id| {
                near_assert!(
                    !metadata_tokens.contains_key(&id.0),
                    "Token with ID {}:{} already exists",
                    metadata_id,
                    id.0
                );
                id.0
            })
            .collect()
    }

    fn minting_payout(
        &self,
        metadata_id: u64,
        mut balance: u128,
        creator: AccountId,
    ) {
        // pay minting fee to parent account
        if let Some(factory) = parent_account_id(&env::current_account_id()) {
            Promise::new(factory).transfer(MINTING_FEE);
            balance -= MINTING_FEE
        }

        // pay out royalty holders
        if let Some(royalties) = self.token_royalty.get(&metadata_id) {
            let royalties_total =
                royalties.percentage.multiply_balance(balance);
            for (account_id, percentage) in royalties.split_between.iter() {
                Promise::new(account_id.clone())
                    .transfer(percentage.multiply_balance(royalties_total));
            }
            balance -= royalties_total;
        }

        // rest goes to the creator
        Promise::new(creator).transfer(balance);
    }

    pub(crate) fn get_minting_metadata(
        &self,
        metadata_id: u64,
    ) -> MintingMetadata {
        match self.token_metadata.get(&metadata_id) {
            None => {
                near_panic!("Metadata with ID {} does not exist", metadata_id)
            }
            Some(metadata) => metadata,
        }
    }
}

fn option_string_is_u64(opt_s: &Option<String>) -> bool {
    opt_s
        .as_ref()
        .map(|s| s.parse::<u64>().is_ok())
        .unwrap_or(true)
}

fn log_create_metadata(
    metadata_id: u64,
    minting_metadata: MintingMetadata,
    royalty: Option<Royalty>,
) {
    env::log_str(
        CreateMetadataData {
            metadata_id: metadata_id.into(),
            creator: minting_metadata.creator,
            minters_allowlist: minting_metadata.allowlist,
            price: minting_metadata.price.into(),
            royalty,
            max_supply: minting_metadata.max_supply,
            last_possible_mint: minting_metadata
                .last_possible_mint
                .map(Into::into),
            is_locked: minting_metadata.is_locked,
        }
        .serialize_event()
        .as_str(),
    );
}

fn log_nft_batch_mint(
    token_ids: Vec<String>,
    minter: &str,
    owner: &str,
    royalty: &Option<mb_sdk::data::store::Royalty>,
    split_owners: &Option<mb_sdk::data::store::SplitOwners>,
    meta_ref: &Option<String>,
    meta_extra: &Option<String>,
) {
    let memo = serde_json::to_string(&NftMintLogMemo {
        royalty: royalty.clone(),
        split_owners: split_owners.clone(),
        meta_id: meta_ref.clone(),
        meta_extra: meta_extra.clone(),
        minter: minter.to_string(),
    })
    .unwrap();
    let log = NftMintLog {
        owner_id: owner.to_string(),
        token_ids,
        memo: Option::from(memo),
    };

    env::log_str(log.serialize_event().as_str());
}

pub(crate) fn log_grant_creator(account_id: &AccountId) {
    env::log_str(
        &MbStoreChangeSettingDataV020 {
            granted_minter: Some(account_id.to_string()),
            ..MbStoreChangeSettingDataV020::empty()
        }
        .serialize_event(),
    );
}

pub(crate) fn log_revoke_creator(account_id: &AccountId) {
    env::log_str(
        &MbStoreChangeSettingDataV020 {
            revoked_minter: Some(account_id.to_string()),
            ..MbStoreChangeSettingDataV020::empty()
        }
        .serialize_event(),
    );
}

fn parent_account_id(child: &AccountId) -> Option<AccountId> {
    child
        .as_str()
        .split_once('.')
        .unwrap()
        .1
        .to_string()
        .try_into()
        .ok()
}

pub(crate) fn validate_metadata(metadata: &TokenMetadata) {
    near_assert!(
        option_string_is_u64(&metadata.starts_at),
        "`metadata.starts_at` needs to parse to a u64"
    );
    near_assert!(
        option_string_is_u64(&metadata.expires_at),
        "`metadata.expires_at` needs to parse to a u64"
    );
}
