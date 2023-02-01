use mb_sdk::{
    near_assert,
    near_sdk::{
        self,
        borsh::{
            self,
            BorshDeserialize,
            BorshSerialize,
        },
        collections::{
            UnorderedMap,
            UnorderedSet,
        },
        env,
        json_types::U128,
        AccountId,
        Balance,
        Promise,
    },
};

/// Contains constants and type definitions
mod data;
mod listing;
mod offers;

use data::*;

// ------------------------- market smart contract -------------------------- //
/// Storage of the market contract
#[derive(BorshSerialize, BorshDeserialize, near_sdk::PanicOnDefault)]
#[near_sdk::near_bindgen]
pub struct Market {
    /// Contains all currently listed tokens
    pub listings: UnorderedMap<String, Listing>,
    /// Contains a list of accounts that we don't do business with
    pub banned_accounts: UnorderedSet<AccountId>,
    /// Contains a list of accounts that are allowed to set referrals
    pub referrers: UnorderedMap<AccountId, u16>,
    /// Contains the storage deposits of all accounts, which are needed to list
    /// a token without being able to hold our market hostage
    pub storage_deposits_by_account: UnorderedMap<AccountId, Balance>,
    /// Simple counter how many listings a given account has with the market,
    /// required for book-keeping
    pub listings_count_by_account: UnorderedMap<AccountId, u64>,
    /// How much storage deposit we require for a single listing
    pub listing_storage_deposit: Balance,
    /// How long (in seconds) a listing must be active in the market before it
    /// can be unlisted
    pub listing_lock_seconds: u64,
    /// The percentage of a cut that remains with Mintbase in case that a token
    /// is sold by referral. E.g.: Ife `referral_cut` is 10%, `mb_cut` is 40%,
    /// and a token gets sold for 100 $NEAR, then 4 $NEAR will end up with
    /// mintbase and 6 $NEAR will end up with the referrer.
    pub mintbase_cut: u16,
    /// The fallback cut that is applied for the case of no referral.
    pub fallback_cut: u16,
    /// The owner of the market, who is allowed to configure it.
    pub owner: AccountId,
}

#[near_sdk::near_bindgen]
impl Market {
    #[init]
    pub fn init(
        owner: AccountId,
        mintbase_cut: u16,
        fallback_cut: u16,
        listing_lock_seconds: u64,
    ) -> Self {
        Self {
            listings: UnorderedMap::new(&b"k2l"[..]),
            banned_accounts: UnorderedSet::new(&b"b"[..]),
            referrers: UnorderedMap::new(&b"r"[..]),
            storage_deposits_by_account: UnorderedMap::new(&b"a2d"[..]),
            listings_count_by_account: UnorderedMap::new(&b"a2l"[..]),
            listing_storage_deposit: TEN_MILLINEAR,
            listing_lock_seconds,
            mintbase_cut,
            fallback_cut,
            owner,
        }
    }

    // ---------------- config methods reserved to market owner ----------------
    // -------- ownership itself
    /// Sets the owner of the market contract. The owner will be allowed to
    /// modify market settings. Only the owner can call this.
    #[payable]
    pub fn set_owner(&mut self, new_owner: AccountId) {
        self.assert_predecessor_is_owner();
        self.owner = new_owner;
    }
    /// Show owner of the market contract
    pub fn get_owner(&self) -> AccountId {
        self.owner.clone()
    }

    // -------- cut remaining with mintbase in case of referral
    /// Set the cut that the market takes from each affiliate sale. In total,
    /// `mintbase_cut * affiliate_cut * sale_price` will remain with the
    /// markets. The unit is `1 / 10_000`. Only the owner can call this.
    #[payable]
    pub fn set_mintbase_cut(&mut self, new_cut: u16) {
        self.assert_predecessor_is_owner();
        self.mintbase_cut = new_cut;
    }
    /// Show cut that mintbase takes from each affiliate sale
    pub fn get_mintbase_cut(&self) -> u16 {
        self.mintbase_cut
    }

    // -------- fallback cut (no referral)
    /// Set the fallback cut that the market keeps for each non-affiliated sale.
    /// Again, units are in `1 / 10_000`.  Only the owner can call this.
    #[payable]
    pub fn set_fallback_cut(&mut self, new_cut: u16) {
        self.assert_predecessor_is_owner();
        self.fallback_cut = new_cut;
    }
    /// Show the cut that the market keeps on non-affiliated sales.
    pub fn get_fallback_cut(&self) -> u16 {
        self.fallback_cut
    }

    // -------- how long listings are locked
    /// Set the duration (in seconds) that each listing is locked after
    /// creation. Only the owner can call this.
    #[payable]
    pub fn set_listing_lock_seconds(&mut self, secs: u64) {
        self.assert_predecessor_is_owner();
        self.listing_lock_seconds = secs;
    }
    /// Show duration (in seconds) that each listing is locked after creation.
    pub fn get_listing_lock_seconds(&self) -> u64 {
        self.listing_lock_seconds
    }

    // -------- storage deposit for single listing
    /// Set the deposit in yoctoNEAR that each listing will (maximally) require.
    /// Only the owner can call this.
    #[payable]
    pub fn set_listing_storage_deposit(&mut self, deposit: U128) {
        self.assert_predecessor_is_owner();
        self.listing_storage_deposit = deposit.0;
    }
    /// Show current deposit in yoctoNEAR that each listing will (maximally)
    /// require.
    pub fn get_listing_storage_deposit(&self) -> U128 {
        self.listing_storage_deposit.into()
    }

    // -------- banning accounts
    /// Add an account to the banlist. These might be misbehaving NFT contracts,
    /// FT contracts, sellers, or buyers. Banned accounts will still be
    /// respected in payouts. Only the owner can call this.
    #[payable]
    pub fn ban(&mut self, account_id: AccountId) {
        self.assert_predecessor_is_owner();
        self.banned_accounts.insert(&account_id);
    }
    /// Remove an account from the banlist.  Only the owner can call this.
    #[payable]
    pub fn unban(&mut self, account_id: AccountId) {
        self.assert_predecessor_is_owner();
        self.banned_accounts.remove(&account_id);
    }
    /// Show a list of all accounts that are banned from interacting with the
    /// market.
    pub fn banned_accounts(&self) -> Vec<AccountId> {
        self.banned_accounts.iter().collect()
    }

    // -------- affiliates whitelist
    /// Add a registered affiliate. This allows to set a custom fee whereas
    /// non-registered affiliates will share the fallback with the market.
    /// Only the owner can call this.
    #[payable]
    pub fn add_affiliate(&mut self, account_id: AccountId, cut: u16) {
        self.assert_predecessor_is_owner();
        self.referrers.insert(&account_id, &cut);
    }
    /// Remove a registered affiliate. Only the owner can call this.
    #[payable]
    pub fn del_affiliate(&mut self, account_id: AccountId) {
        self.assert_predecessor_is_owner();
        self.referrers.remove(&account_id);
    }
    /// Show all registered affiliates together with their custom fees.
    pub fn affiliates(&self) -> Vec<(AccountId, u16)> {
        self.referrers.iter().collect()
    }

    // ---------------------- anything related to storage ----------------------
    /// Get the number of listings created by a specific account ID
    pub fn get_listings_count(&self, account: &AccountId) -> u64 {
        self.listings_count_by_account.get(account).unwrap_or(0)
    }
    /// Increment the number of listings created by a specific account ID
    fn increase_listings_count(&mut self, account: &AccountId, n: u64) {
        let new_count = self.get_listings_count(account) + n;
        self.listings_count_by_account.insert(account, &new_count);
    }
    /// Decrement the number of listings created by a specific account ID
    fn decrease_listings_count(&mut self, account: &AccountId, n: u64) {
        let new_count = self.get_listings_count(account) - n;
        if new_count == 0 {
            self.listings_count_by_account.remove(account);
        } else {
            self.listings_count_by_account.insert(account, &new_count);
        }
    }

    /// Get the storage deposit required for all the listings of a specific
    /// account ID.
    pub fn get_storage_deposit(&self, account: &AccountId) -> U128 {
        self.storage_deposit_by(account).into()
    }
    /// Deposit NEAR for storage staking on the market. This is required to
    /// create new listings.
    #[payable]
    pub fn deposit_storage(&mut self) {
        let account = env::predecessor_account_id();
        self.assert_not_banned(&account);

        let new_deposit = env::attached_deposit();
        let old_deposit = self.storage_deposit_by(&account);
        self.storage_deposits_by_account
            .insert(&account, &(old_deposit + new_deposit));
    }
    /// Claim storage deposits that are not required to cover any listings.
    #[payable]
    pub fn claim_unused_storage_deposit(&mut self) -> Promise {
        // checks on caller
        let account = env::predecessor_account_id();
        self.assert_not_banned(&account);
        near_sdk::assert_one_yocto();

        // get required amount
        let deposit = self.storage_deposit_by(&account);
        let required = self.get_listings_count(&account) as Balance
            * self.listing_storage_deposit;
        let refund = deposit - required;

        // send the refund
        self.refund_storage_deposit(&account, refund)
    }
    /// Get the storage of a specified account.
    fn storage_deposit_by(&self, account: &AccountId) -> Balance {
        self.storage_deposits_by_account.get(account).unwrap_or(0)
    }

    /// Refund a storage deposit.
    fn refund_storage_deposit(
        &mut self,
        account: &AccountId,
        refund: Balance,
    ) -> Promise {
        // decrease for internal usage
        let old_deposit =
            self.storage_deposits_by_account.get(account).unwrap_or(0);
        let new_deposit = old_deposit - refund;
        if new_deposit == 0 {
            self.storage_deposits_by_account.remove(account);
        } else {
            self.storage_deposits_by_account
                .insert(account, &new_deposit);
        }

        // actual refund
        Promise::new(account.to_owned()).transfer(refund)
    }

    /// Decrease listings count and refund the lister with the deposits.
    fn refund_listings(&mut self, account: &AccountId, n: u64) -> Promise {
        // decrease listing number
        self.decrease_listings_count(account, n);

        // decrease storage deposit
        self.refund_storage_deposit(
            account,
            self.listing_storage_deposit * n as u128,
        )
    }

    // ---------------------------- utility methods ----------------------------
    /// Panics if the given account is banned
    fn assert_not_banned(&self, account: &AccountId) {
        near_assert!(
            !self.banned_accounts.contains(account),
            "{} is banned from the market",
            account
        );
    }

    /// Panics if the current call is not from the market owner.
    fn assert_predecessor_is_owner(&self) {
        near_assert!(
            env::predecessor_account_id() == self.owner,
            "Method is restricted to market owner"
        );
    }

    /// Calculates the storage deposit for a given account that is not currently
    /// needed to cover listings
    fn free_storage_deposit(&self, account: &AccountId) -> Balance {
        let deposit = self.storage_deposit_by(account);
        let required = self.get_listings_count(account) as u128
            * self.listing_storage_deposit;
        deposit - required
    }
}
