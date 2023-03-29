use std::str::FromStr;

use mb_sdk::{
    constants::{
        StorageCostsMarket,
        YOCTO_PER_BYTE,
    },
    data::market_v1::{
        TokenListing,
        TokenOffer,
    },
    events::market_v1::{
        UpdateAllowlistData,
        UpdateBanlistData,
    },
    near_assert,
    near_panic,
    near_sdk::{
        self,
        assert_one_yocto,
        borsh::{
            self,
            BorshDeserialize,
            BorshSerialize,
        },
        collections::{
            LookupMap,
            UnorderedSet,
        },
        env,
        json_types::U128,
        near_bindgen,
        AccountId,
        PanicOnDefault,
    },
    utils::{
        ntoy,
        SafeFraction,
        TokenKey,
    },
};

mod listings;
mod offers;

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault)]
pub struct Marketplace {
    /// The active list of tokens this contract has receieved as listed
    /// entities.
    pub listings: LookupMap<TokenKey, TokenListing>,
    /// Privileged account for the Market. May call methods in
    /// `market_owner`.
    pub owner_id: AccountId,
    /// The percentage taken by Mintbase for transfers on this contract.
    pub take: SafeFraction,
    /// The minimum number of hours an offer must be valid for.
    pub min_offer_hours: u64,
    /// The amount of Near deposited onto the Loan contract that has been
    /// earmarked for users. The remainder of `env::current_balance` may be
    /// withdrawn by the owner.
    pub deposit_required: u128,
    /// Base accounts that are allowed to list tokens to this `Marketplace`
    pub allowlist: UnorderedSet<AccountId>,
    /// Accounts that are banned from the `Marketplace`
    pub banlist: UnorderedSet<AccountId>,
    /// The Near-denominated price-per-byte of storage. As of April 2021, the
    /// price per bytes is set by default to 10^19, but this may change in
    /// the future, thus this future-proofing field.
    pub storage_costs: StorageCostsMarket,
}

#[near_bindgen]
impl Marketplace {
    /// Create a new `Marketplace`. Validate that owner must is a valid
    /// address.
    #[init]
    pub fn new(init_allowlist: Vec<AccountId>) -> Self {
        let mut allowlist = UnorderedSet::new(b"a".to_vec());
        init_allowlist.iter().for_each(|account| {
            allowlist.insert(account);
        });

        Self {
            listings: LookupMap::new(b"b".to_vec()),
            owner_id: env::predecessor_account_id(),
            take: SafeFraction::new(250), // 2.5%
            min_offer_hours: 24,
            deposit_required: env::account_balance(),
            allowlist,
            banlist: UnorderedSet::new(b"d".to_vec()),
            storage_costs: StorageCostsMarket::new(YOCTO_PER_BYTE), // 10^19
        }
    }

    /// The Near Storage price per byte has changed in the past, and may change in
    /// the future. This method may never be used.
    #[payable]
    pub fn set_storage_price_per_byte(&mut self, new_price: U128) {
        self.assert_owner_marketplace();
        self.storage_costs = StorageCostsMarket::new(new_price.into());
    }

    /// Update the owner of the `Marketplace`.
    #[payable]
    pub fn set_owner(&mut self, new_owner: AccountId) {
        self.assert_owner_marketplace();
        self.owner_id = new_owner;
    }

    /// Set the percentage taken by the `Marketplace`.
    #[payable]
    pub fn set_take(&mut self, percentage: u32) {
        self.assert_owner_marketplace();
        near_assert!(
            percentage < 1000,
            "Cannot set marketplace revenue take above 10%"
        );
        self.take = SafeFraction::new(percentage);
    }

    /// Set the minimum number of hours an `Offer` must be valid for.
    #[payable]
    pub fn set_min_offer_hours(&mut self, min_offer_hours: u64) {
        self.assert_owner_marketplace();
        self.min_offer_hours = min_offer_hours;
    }

    /// Owner of this `Marketplace` may call to remove Near deposited from
    /// contract storage cost, and Market royalty fees.
    #[payable]
    pub fn withdraw_revenue(&mut self) {
        self.assert_owner_marketplace();
        let withdrawable = env::account_balance() - self.deposit_required;
        near_sdk::Promise::new(self.owner_id.clone()).transfer(withdrawable);
    }

    /// Asserts that the function caller owns the the token with `String`.
    fn assert_caller_owns_token(&self, token_key: &str) {
        let token = self.get_token(token_key.to_string());
        let caller = env::predecessor_account_id();
        near_assert!(
            token.owner_id == caller,
            "{} does not own token {}",
            caller,
            token_key
        );
    }

    /// Assume accounts can have format:
    /// 0. "12345678901234567890123456789012" return "12345678901234567890123456789012"
    /// 1. "abc.near" => return "abc.near"
    /// 2. "sub.abc.near" => return "abc.near"
    /// 3. "dub.sub.abc.near" => return "sub.abc.near"
    /// 4. "wub.dub.sub.abc.near" return "dub.sub.abc.near"
    /// ... more periods
    ///
    /// In other words:
    /// case 0,1: return the whole account
    /// case 2..:  return the account stripping the first prefix off
    fn get_pred_base_account(&self) -> near_sdk::AccountId {
        let account = env::predecessor_account_id();
        if let Some(strip_prefix) =
            account.to_string().split_once('.').map(|x| x.1)
        {
            match strip_prefix.split_once('.').map(|x| x.1) {
                // case 2...
                Some(_) => {
                    AccountId::from_str(strip_prefix.to_string().as_str())
                        .unwrap()
                }
                // case 1
                None => account,
            }
        } else {
            // case 0
            account
        }
    }

    /// Send `account_id` `amount` of Near. Handle possible failure.
    fn tx_send(&mut self, account_id: near_sdk::AccountId, amount: u128) {
        self.deposit_required -= amount;
        near_sdk::Promise::new(account_id).transfer(amount);
    }

    /// Only allow permitted addresses to list tokens at the Market
    fn is_pred_mintbase_or_allowlist_and_not_banlist(&self) -> bool {
        if self.banlist.contains(&env::predecessor_account_id()) {
            return false;
        }
        let base_account = self.get_pred_base_account();
        self.allowlist.contains(&base_account)
    }

    /// Update base accounts that are allowed to list tokens to this
    /// `Marketplace`.
    #[payable]
    pub fn update_allowlist(&mut self, account_id: AccountId, state: bool) {
        self.assert_owner_marketplace();
        match state {
            true => {
                near_assert!(
                    env::account_balance()
                        - env::storage_usage() as u128
                            * env::storage_byte_cost()
                        > ntoy(1) / 1000,
                    "Storage cost not covered"
                );
                self.allowlist.insert(&account_id)
            }
            false => self.allowlist.remove(&account_id),
        };
        log_allowlist_update(&account_id, state);
    }

    /// Update accounts that are banned from the `Marketplace`.
    #[payable]
    pub fn update_banlist(&mut self, account_id: AccountId, state: bool) {
        self.assert_owner_marketplace();
        match state {
            true => self.banlist.insert(&account_id),
            false => self.banlist.remove(&account_id),
        };
        log_banlist_update(&account_id, state);
    }

    /// Kick a set of tokens off the marketplace. This should only be used
    /// when listing processing has failed in the past and no offers are
    /// currently in progress, as it might interfere with running XCC chains
    /// from `make_offer`.
    #[payable]
    pub fn kick_tokens(&mut self, token_keys: Vec<String>) {
        self.assert_owner_marketplace();
        token_keys.into_iter().for_each(|token_key| {
            let token = self.get_token(token_key.clone());
            let key: TokenKey = token_key.as_str().into();
            self.delist_internal(&key, token);
        });
    }

    /// Helper function determining contract ownership.
    fn assert_owner_marketplace(&self) {
        assert_one_yocto();
        near_assert!(
            env::predecessor_account_id() == self.owner_id,
            "Only the market owner can call this method."
        );
    }

    /// Get `Marketplace` owner.
    pub fn get_owner(&self) -> &AccountId {
        &self.owner_id
    }

    /// Get `Marketplace` royalty take.
    pub fn get_take(&self) -> SafeFraction {
        self.take
    }

    /// Get `Marketplace` minimum `Offer` hours for an `Offer` to expire.
    pub fn get_min_offer_hours(&self) -> u64 {
        self.min_offer_hours
    }

    pub fn get_banlist(&self) -> Vec<AccountId> {
        self.banlist.iter().collect()
    }

    pub fn get_allowlist(&self) -> Vec<AccountId> {
        self.allowlist.iter().collect()
    }

    /// Get the Token with `TokenKey`.
    pub fn get_token(&self, token_key: String) -> TokenListing {
        let key: TokenKey = token_key.as_str().into();
        self.listings
            .get(&key)
            .unwrap_or_else(|| near_panic!("Cannot find token {}", token_key))
    }

    /// Get Token `owner_id`.
    pub fn get_token_owner_id(&self, token_key: String) -> AccountId {
        self.get_token(token_key).owner_id
    }

    /// Get Token `autotransfer`.
    pub fn get_token_autotransfer(&self, token_key: String) -> bool {
        self.get_token(token_key).autotransfer
    }

    /// Get Token `asking_price`.
    pub fn get_token_asking_price(&self, token_key: String) -> U128 {
        self.get_token(token_key).asking_price
    }

    /// Get the offers for token `token_key`.
    pub fn get_current_offer(&self, token_key: String) -> Option<TokenOffer> {
        self.get_token(token_key).current_offer
    }
}

pub(crate) fn log_banlist_update(account_id: &AccountId, state: bool) {
    let data = UpdateBanlistData {
        account_id: account_id.to_string(),
        state,
    };
    env::log_str(&data.serialize_event());
}

fn log_allowlist_update(account_id: &AccountId, state: bool) {
    let data = UpdateAllowlistData {
        account_id: account_id.to_string(),
        state,
    };
    env::log_str(&data.serialize_event());
}
