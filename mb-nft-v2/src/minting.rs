use std::convert::TryInto;

use mb_sdk::{
    constants::{
        MAX_LEN_PAYOUT,
        MINIMUM_FREE_STORAGE_STAKE,
        MINTING_FEE,
    },
    data::store::{
        Royalty,
        RoyaltyArgs,
        SplitBetweenUnparsed,
        SplitOwners,
        Token,
        TokenMetadata,
    },
    events::store::{
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
        PromiseOrValue,
    },
};

use crate::*;

#[near_bindgen]
impl MintbaseStore {
    // -------------------------- change methods ---------------------------

    /// The core `Store` function. `mint_token` mints `num_to_mint` copies of
    /// a token.
    ///
    /// Restrictions:
    /// - Only minters may call this function.
    /// - `owner_id` must be a valid Near address.
    /// - Because of logging limits, this method may mint at most 125 tokens per call.
    /// - 1.0 >= `royalty_f` >= 0.0. `royalty_f` is ignored if `royalty` is `None`.
    /// - If a `royalty` is provided, percentages **must** be non-negative and add to one.
    /// - The maximum length of the royalty mapping is 50.
    ///
    /// This method is the most significant increase of storage costs on this
    /// contract. Minters are expected to manage their own storage costs.
    // #[payable]
    // pub fn nft_batch_mint(
    //     &mut self,
    //     owner_id: AccountId,
    //     #[allow(unused_mut)] // cargo complains, but it's required
    //     mut metadata: TokenMetadata,
    //     num_to_mint: Option<u64>,
    //     token_ids: Option<Vec<U64>>,
    //     royalty_args: Option<RoyaltyArgs>,
    //     split_owners: Option<SplitBetweenUnparsed>,
    // ) -> PromiseOrValue<()> {
    //     let (num_to_mint, token_ids, predefined_ids) = match (num_to_mint, token_ids) {
    //         (None, None) => near_panic!("Must either specify `num_to_mint` or `token_ids`"),
    //         (Some(_), Some(_)) => near_panic!("Cannot specify both `num_to_mint` and `token_ids` at the same time"),
    //         (Some(n), None) => (n, (self.tokens_minted..self.tokens_minted + n).collect::<Vec<u64>>(), false),
    //         (None, Some(ids)) => (ids.len() as u64, ids.into_iter().map(|id| id.0).collect::<Vec<u64>>(), true),
    //     };

    //     near_assert!(num_to_mint > 0, "No tokens to mint");
    //     near_assert!(
    //         num_to_mint <= 125,
    //         "Cannot mint more than 125 tokens due to gas limits"
    //     ); // upper gas limit
    //     if let Some(cap) = self.minting_cap {
    //         near_assert!(
    //             self.tokens_minted + num_to_mint <= cap,
    //             "This mint would exceed the smart contracts minting cap"
    //         );
    //     }
    //     near_assert!(
    //         env::attached_deposit() >= 1,
    //         "Requires deposit of at least 1 yoctoNEAR"
    //     );
    //     let minter_id = env::predecessor_account_id();
    //     near_assert!(
    //         self.minters.contains(&minter_id) || self.minters.is_empty(),
    //         "{} is not allowed to mint on this store",
    //         minter_id
    //     );

    //     near_assert!(
    //         !option_string_starts_with(
    //             &metadata.reference,
    //             &self.metadata.base_uri
    //         ),
    //         "`metadata.reference` must not start with contract base URI"
    //     );
    //     near_assert!(
    //         !option_string_starts_with(
    //             &metadata.media,
    //             &self.metadata.base_uri
    //         ),
    //         "`metadata.media` must not start with contract base URI"
    //     );
    //     near_assert!(
    //         option_string_is_u64(&metadata.starts_at),
    //         "`metadata.starts_at` needs to parse to a u64"
    //     );
    //     near_assert!(
    //         option_string_is_u64(&metadata.expires_at),
    //         "`metadata.expires_at` needs to parse to a u64"
    //     );

    //     // Calculating storage consuption upfront saves gas if the transaction
    //     // were to fail later.
    //     let covered_storage = env::attached_deposit() - MINTING_FEE;
    //     metadata.copies = metadata.copies.or(Some(num_to_mint as u16));
    //     let md_size = borsh::to_vec(&metadata).unwrap().len() as u64;
    //     let roy_len = royalty_args
    //         .as_ref()
    //         .map(|pre_roy| {
    //             let len = pre_roy.split_between.len();
    //             len as u32
    //         })
    //         .unwrap_or(0);
    //     let split_len = split_owners
    //         .as_ref()
    //         .map(|pre_split| {
    //             let len = pre_split.len();
    //             len as u32
    //         })
    //         // if there is no split map, there still is an owner, thus default to 1
    //         .unwrap_or(1);
    //     near_assert!(
    //         roy_len + split_len <= MAX_LEN_PAYOUT,
    //         "Number of payout addresses may not exceed {}",
    //         MAX_LEN_PAYOUT
    //     );
    //     let expected_storage_consumption: Balance =
    //         self.storage_cost_to_mint(num_to_mint, md_size, roy_len, split_len);
    //     near_assert!(
    //         covered_storage >= expected_storage_consumption,
    //         "This mint would exceed the current storage coverage of {} yoctoNEAR. Requires at least {} yoctoNEAR",
    //         covered_storage,
    //         expected_storage_consumption
    //     );

    //     let checked_royalty = royalty_args.map(Royalty::new);
    //     let checked_split = split_owners.map(SplitOwners::new);

    //     let mut owned_set = self.get_or_make_new_owner_set(&owner_id);

    //     // Lookup Id is used by the token to lookup Royalty and Metadata fields on
    //     // the contract (to avoid unnecessary duplication)
    //     let lookup_id: u64 = self.tokens_minted;
    //     let royalty_id = checked_royalty.clone().map(|royalty| {
    //         self.token_royalty
    //             .insert(&lookup_id, &(num_to_mint as u16, royalty));
    //         lookup_id
    //     });

    //     let meta_ref = metadata.reference.as_ref().map(|s| s.to_string());
    //     let meta_extra = metadata.extra.as_ref().map(|s| s.to_string());
    //     self.token_metadata
    //         .insert(&lookup_id, &(num_to_mint as u16, metadata));

    //     // Mint em up hot n fresh with a side of vegan bacon
    //     let token_ids = token_ids
    //         .into_iter()
    //         .map(|mut token_id| {
    //             // Check if token ID is already occupied, panic for predefined,
    //             // otherwise create non-occupied ID
    //             if self.tokens.contains_key(&token_id) && predefined_ids {
    //                 near_panic!("Predefined token ID is already in use");
    //             }
    //             while self.tokens.contains_key(&token_id) {
    //                 token_id += num_to_mint
    //             }

    //             let token = Token::new(
    //                 owner_id.clone(),
    //                 token_id,
    //                 lookup_id,
    //                 royalty_id,
    //                 checked_split.clone(),
    //                 minter_id.clone(),
    //             );
    //             owned_set.insert(&token_id);
    //             self.tokens.insert(&token_id, &token);
    //             token_id
    //         })
    //         .collect::<Vec<u64>>();
    //     self.tokens_minted += num_to_mint;
    //     self.tokens_per_owner.insert(&owner_id, &owned_set);

    //     // check if sufficient storage stake (e.g. 0.5 NEAR) remains
    //     let used_storage_stake: Balance =
    //         env::storage_usage() as u128 * env::storage_byte_cost();
    //     let free_storage_stake: Balance =
    //         env::account_balance() - used_storage_stake;
    //     near_assert!(
    //         free_storage_stake > MINIMUM_FREE_STORAGE_STAKE,
    //         "A minimum of {} yoctoNEAR is required as free contract balance to allow updates (currently: {})",
    //         MINIMUM_FREE_STORAGE_STAKE,
    //         free_storage_stake
    //     );

    //     log_nft_batch_mint(
    //         &token_ids,
    //         minter_id.as_ref(),
    //         owner_id.as_ref(),
    //         &checked_royalty,
    //         &checked_split,
    //         &meta_ref,
    //         &meta_extra,
    //     );

    //     // Transfer minting fee if parent is a valid account (assuming this is
    //     // a factory). If parent is not valid, e.g. this contract was deployed
    //     // to a random top-level account, do nothing.
    //     match parent_account_id(&env::current_account_id()) {
    //         Some(factory) => {
    //             let p = Promise::new(factory).transfer(MINTING_FEE);
    //             PromiseOrValue::Promise(p)
    //         }
    //         _ => PromiseOrValue::Value(()),
    //     }
    // }

    #[payable]
    pub fn create_metadata(
        &mut self,
        //     owner_id: AccountId,
        metadata: TokenMetadata,
        metadata_id: Option<U64>,
        royalty_args: Option<RoyaltyArgs>,
        minters_allowlist: Option<Vec<AccountId>>,
        price: U128,
    ) -> String {
        // metadata ID: either predefined (must not conflict with existing), or
        // increasing the counter for it
        let metadata_id = self.get_metadata_id(metadata_id);

        // creator needs to be allowed to create metadata on this smart contract
        let creator = env::predecessor_account_id();
        near_assert!(self.creators.contains(&creator), "{}", creator);

        // validate metadata
        validate_metadata(&metadata, &self.metadata.base_uri);

        // validate royalties
        let roy_len = royalty_args
            .as_ref()
            .map(|pre_roy| {
                let len = pre_roy.split_between.len();
                len as u32
            })
            .unwrap_or(0);
        near_assert!(
            // TODO: this should probably be less than MAX_LEN_PAYOUT, such
            // that splits can still be added
            roy_len <= MAX_LEN_PAYOUT,
            "Number of royalty holders may not exceed {}",
            MAX_LEN_PAYOUT
        );

        // makes sure storage is covered
        // FIXME: add minters list and token price to storage calculation
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
            expected_storage_consumption
        );

        // insert metadata
        self.token_metadata
            .insert(&metadata_id, &(0, price.0, minters_allowlist, metadata));

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

        // FIXME: add event

        return metadata_id.to_string();
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

    pub fn mint_on_metadata(
        &mut self,
        metadata_id: U64,
        owner_id: AccountId,
        num_to_mint: Option<u8>,
        token_ids: Option<Vec<U64>>,
    ) {
        // check if this account is allowed to mint this metadata
        // TODO:

        // was the storage deposit attached? should the storage be paid by the metadata creator?
        // TODO:

        // is the price attached?
        // TODO:

        // get valid token IDs
        // TODO:

        // mint the tokens and emit event

        // Transfer minting fee to parent account
        // TODO:
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
        // create a metadata record
        metadata_storage as u128 * self.storage_costs.storage_price_per_byte
            // create a royalty record
            + num_royalties as u128 * self.storage_costs.common
            // store the minters list
            + num_minters as u128 * self.storage_costs.account_id
            // store the price
            + self.storage_costs.balance
    }

    /// Get the storage in bytes to mint `num_tokens` each with
    /// `metadata_storage` and `len_map` royalty receivers.
    /// Internal
    fn storage_cost_to_mint(
        &self,
        num_tokens: u64,
        metadata_storage: StorageUsage,
        num_royalties: u32,
        num_splits: u32,
    ) -> near_sdk::Balance {
        // create a metadata record
        metadata_storage as u128 * self.storage_costs.storage_price_per_byte
            // create a royalty record
            + num_royalties as u128 * self.storage_costs.common
            // create n tokens each with splits stored on-token
            + num_tokens as u128 * (
                // token base storage
                self.storage_costs.token
                // dynamic split storage
                + num_splits as u128 * self.storage_costs.common
                // create an entry in tokens_per_owner
                + self.storage_costs.common
            )
    }
}

fn option_string_starts_with(
    string: &Option<String>,
    prefix: &Option<String>,
) -> bool {
    match (string, prefix) {
        (Some(s), Some(p)) => s.starts_with(p),
        _ => false,
    }
}

fn option_string_is_u64(opt_s: &Option<String>) -> bool {
    opt_s
        .as_ref()
        .map(|s| s.parse::<u64>().is_ok())
        .unwrap_or(true)
}

fn log_nft_batch_mint(
    token_ids: &[u64],
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
        token_ids: token_ids.iter().map(|t| t.to_string()).collect(),
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

fn validate_metadata(metadata: &TokenMetadata, base_uri: &Option<String>) {
    near_assert!(
        !option_string_starts_with(&metadata.reference, base_uri),
        "`metadata.reference` must not start with contract base URI"
    );
    near_assert!(
        !option_string_starts_with(&metadata.media, base_uri),
        "`metadata.media` must not start with contract base URI"
    );
    near_assert!(
        option_string_is_u64(&metadata.starts_at),
        "`metadata.starts_at` needs to parse to a u64"
    );
    near_assert!(
        option_string_is_u64(&metadata.expires_at),
        "`metadata.expires_at` needs to parse to a u64"
    );
}
