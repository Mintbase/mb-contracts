use mb_sdk::{
    data::market_v1::TokenListing,
    events::market_v1::{
        NftListData,
        NftListLog,
        NftUnlistLog,
        NftUpdateListData,
    },
    near_assert,
    near_sdk::{
        self,
        assert_one_yocto,
        env,
        json_types::{
            U128,
            U64,
        },
        near_bindgen,
        AccountId,
    },
    utils::TokenKey,
};

use crate::{
    Marketplace,
    MarketplaceExt,
};

#[near_bindgen]
impl Marketplace {
    /// Callback for `nft_approve` on the token contract. This creates a listing
    /// on the market contract. Standardized and thus accessible for all NFT
    /// contracts adhering to NEP-178, though a successful sale will also
    /// require adhering to NEP-199.
    pub fn nft_on_approve(
        &mut self,
        token_id: U64,
        owner_id: AccountId,
        approval_id: u64,
        msg: String, // try to parse into saleArgs
    ) {
        // assert!(env::attached_deposit() >= self.storage_costs.list);
        let sale_args: mb_sdk::data::market_v1::SaleArgs =
            near_sdk::serde_json::from_str(&msg).expect("Not valid SaleArgs");
        near_assert!(
            self.is_pred_mintbase_or_allowlist_and_not_banlist(),
            "Cannot accept tokens from {}",
            env::predecessor_account_id()
        );
        self.deposit_required += self.storage_costs.list;

        let token = self.listing_insert_internal(
            token_id,
            U64(approval_id),
            &owner_id,
            &sale_args,
        );
        log_listing_created(
            &token.get_list_id(),
            &sale_args.price,
            &token.get_token_key().to_string(),
            &owner_id,
            sale_args.autotransfer,
        );
    }

    /// Callback for `nft_batch_approve` (Mintbase-specific). Behaves similar to
    /// `nft_on_approve`, but it does require a storage deposit attached for
    /// each created listing, and creates many listings at once. Again,
    /// successful sales will require adherence to NEP-199.
    #[payable]
    pub fn nft_on_batch_approve(
        &mut self,
        tokens: Vec<U64>,
        approvals: Vec<U64>,
        owner_id: AccountId,
        msg: String,
    ) {
        let storage_deposit = self.storage_costs.list * tokens.len() as u128;
        near_assert!(
            env::attached_deposit() >= storage_deposit,
            "The attached deposit does not cover storage costs"
        );
        let sale_args: mb_sdk::data::market_v1::SaleArgs =
            near_sdk::serde_json::from_str(&msg)
                .expect("Sale arguments are invalid");
        near_assert!(
            self.is_pred_mintbase_or_allowlist_and_not_banlist(),
            "Cannot accept tokens from {}",
            env::predecessor_account_id()
        );
        self.deposit_required += storage_deposit;

        tokens.iter().zip(approvals.iter()).for_each(
            |(&token_id, &approval_id)| {
                self.listing_insert_internal(
                    token_id,
                    approval_id,
                    &owner_id,
                    &sale_args,
                );
            },
        );
        log_batch_listing_created(
            &approvals,
            &sale_args.price,
            &tokens,
            &owner_id,
            &env::predecessor_account_id(),
            sale_args.autotransfer,
        );
    }

    /// If `autotransfer` is enabled, the token will automatically be transferred
    /// to an offer-er if their offer is greater than the asking price. If the
    /// asking price is `None`, autotransfer is ignored. Note that enabling
    /// `autotransfer` does not retroactively trigger on currently active
    /// `Offer`s.
    pub fn set_token_autotransfer(&mut self, token_key: String, state: bool) {
        let mut token = self.get_token_internal(token_key.clone());
        token.assert_not_locked();
        self.assert_caller_owns_token(&token_key);
        token.autotransfer = state;
        self.listings.insert(&token_key.as_str().into(), &token);
        log_set_token_autotransfer(token.autotransfer, &token.get_list_id());
    }

    /// Update the asking price for the `Token`. The new price may be `None`.
    #[payable]
    pub fn set_token_asking_price(&mut self, token_key: String, price: U128) {
        assert_one_yocto();
        let mut token = self.get_token_internal(token_key.clone());
        token.assert_not_locked();
        self.assert_caller_owns_token(&token_key);
        token.asking_price = price;
        self.listings.insert(&token_key.as_str().into(), &token);
        log_set_token_asking_price(&price, &token.get_list_id());
    }

    #[payable]
    pub fn delist(
        &mut self,
        nft_contract_id: AccountId,
        token_ids: Vec<String>,
    ) {
        assert_one_yocto();

        for token_id in token_ids {
            let token_key: TokenKey = From::from(
                format!("{}:{}", token_id, nft_contract_id).as_str(),
            );
            let listing = self.listings.get(&token_key);
            near_assert!(listing.is_some(), "Could not find listing");
            let listing = listing.unwrap();
            near_assert!(!listing.locked, "Listing is locked");
            near_assert!(
                env::predecessor_account_id() == listing.owner_id,
                "Only {} may delist this.",
                listing.owner_id
            );

            self.delist_internal(&token_key, listing);
        }
    }

    /// Remove the `Token` (first to avoid re-entrance), then refund the Offerer
    /// if one exists.
    pub(crate) fn delist_internal(
        &mut self,
        token_key: &TokenKey,
        mut token: TokenListing,
    ) {
        self.listings.remove(token_key);
        self.deposit_required -= self.storage_costs.list;
        log_token_removed(&token.get_list_id());
        self.try_refund_offerer(&mut token);
    }

    pub(crate) fn ban(&mut self, token_key: &TokenKey, token: TokenListing) {
        self.banlist.insert(&token.store_id);
        crate::log_banlist_update(&token.store_id, true);
        self.delist_internal(token_key, token);
    }

    pub(crate) fn listing_insert_internal(
        &mut self,
        token_id: U64,
        approval_id: U64,
        owner_id: &AccountId,
        sale_args: &mb_sdk::data::market_v1::SaleArgs,
    ) -> TokenListing {
        let approval_id: u64 = approval_id.into();
        // Create the tokens. Skip any tokens that are already listed.
        let key = TokenKey {
            token_id: token_id.0,
            account_id: env::predecessor_account_id().to_string(),
        };
        let token = TokenListing::new(
            owner_id.clone(),
            env::predecessor_account_id(),
            token_id.into(),
            approval_id,
            sale_args.autotransfer,
            sale_args.price,
        );
        match self.listings.get(&key) {
            None => {
                self.listings.insert(&key, &token);
            }
            Some(old_token) => {
                // token has been relisted, handle old token data and reinsert.
                self.delist_internal(&key, old_token);
                self.listings.insert(&key, &token);
            }
        }
        token
    }
}

fn log_listing_created(
    list_id: &str,
    price: &U128,
    token_key: &str,
    owner_id: &AccountId,
    autotransfer: bool,
) {
    let mut iter = token_key.split(':');
    let mut iter2 = list_id.split(':');
    let token_id = iter.next();
    let store_id = iter.next();
    iter2.next();
    let approval_id = iter2.next().unwrap();
    let data = NftListData(vec![NftListLog {
        list_id: list_id.to_string(),
        price: price.0.to_string(),
        token_key: token_key.to_string(),
        owner_id: owner_id.to_string(),
        autotransfer,
        approval_id: approval_id.to_string(),
        token_id: token_id.unwrap().to_string(),
        store_id: store_id.unwrap().to_string(),
    }]);
    env::log_str(&data.serialize_event());
}

fn log_batch_listing_created(
    approval_ids: &[U64],
    price: &U128,
    token_ids: &[U64],
    owner_id: &AccountId,
    store_id: &AccountId,
    autotransfer: bool,
) {
    let data = NftListData(
        approval_ids
            .iter()
            .enumerate()
            .map(|(u, x)| {
                let list_id =
                    format!("{}:{}:{}", token_ids[u].0, x.0, store_id);
                let token_key = format!("{}:{}", token_ids[u].0, store_id);
                NftListLog {
                    list_id,
                    price: price.0.to_string(),
                    token_key,
                    owner_id: owner_id.to_string(),
                    autotransfer,
                    approval_id: x.0.to_string(),
                    token_id: token_ids[u].0.to_string(),
                    store_id: store_id.to_string(),
                }
            })
            .collect::<Vec<_>>(),
    );
    env::log_str(&data.serialize_event());
}

fn log_set_token_autotransfer(auto_transfer: bool, list_id: &str) {
    let data = NftUpdateListData {
        list_id: list_id.to_string(),
        auto_transfer: Option::from(auto_transfer),
        price: None,
    };
    env::log_str(&data.serialize_event());
}

fn log_set_token_asking_price(price: &U128, list_id: &str) {
    let data = NftUpdateListData {
        list_id: list_id.to_string(),
        auto_transfer: None,
        price: Option::from(price.0.to_string()),
    };
    env::log_str(&data.serialize_event());
}

fn log_token_removed(list_id: &str) {
    let log = NftUnlistLog {
        list_id: list_id.to_string(),
    };
    env::log_str(&log.serialize_event());
}
