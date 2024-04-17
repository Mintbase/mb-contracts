use std::{
    convert::TryFrom,
    str::FromStr,
};

use mb_sdk::{
    constants::{
        gas,
        MAX_LEN_PAYOUT,
        NO_DEPOSIT,
        ONE_YOCTO,
    },
    data::{
        market_v1::{
            TimeUnit,
            TokenListing,
            TokenOffer,
        },
        store::Payout,
    },
    events::market_v1::{
        NftMakeOfferData,
        NftMakeOfferLog,
        NftSaleData,
        NftWithdrawOfferData,
    },
    interfaces,
    near_assert,
    near_panic,
    near_sdk::{
        self,
        assert_one_yocto,
        env,
        json_types::U128,
        near_bindgen,
        AccountId,
        Balance,
        Promise,
    },
    utils::TokenKey,
};

use crate::{
    Marketplace,
    MarketplaceExt,
};

#[near_bindgen]
impl Marketplace {
    /// Make an `Offer` for `Token`. If the token is listed as simple sale (aka
    /// "buy now", `autotransfer` is `true`), the offer price not be below the
    /// asking price. If the token is listed as rolling auction (`autotransfer`
    /// is `false`), you may place an offer below the asking price.
    ///
    /// The `price` argument MUST be >= `env::attached_deposit` on this function.
    #[payable]
    pub fn make_offer(
        &mut self,
        token_key: Vec<String>,
        price: Vec<U128>,
        timeout: Vec<TimeUnit>,
    ) {
        near_assert!(
            price.len() == token_key.len(),
            "Price list doesn't match up with token list"
        );
        near_assert!(
            timeout.len() == token_key.len(),
            "Timeout list doesn't match up with token list"
        );
        let mut total: Balance = 0;
        let token_offers = token_key
            .into_iter()
            .zip(price)
            .zip(timeout)
            .map(|((token_key, price), timeout)| {
                total += price.0;
                match timeout {
                    TimeUnit::Hours(h) => assert!(h >= self.min_offer_hours),
                };

                let mut listing = self.get_token_internal(token_key.clone());
                listing.assert_not_locked();
                listing.num_offers += 1;
                let offer =
                    TokenOffer::new(price.0, timeout, listing.num_offers);

                self.try_make_offer(&mut listing, offer.clone());
                self.listings.insert(&token_key.as_str().into(), &listing);

                if listing.autotransfer && price.0 >= listing.asking_price.0 {
                    self.help_transfer(
                        &token_key.as_str().into(),
                        listing.clone(),
                    );
                }

                (offer, listing, token_key)
            })
            .collect::<Vec<_>>();
        near_assert!(
            total == env::attached_deposit(),
            "Summed prices must match the attached deposit",
        );

        self.deposit_required += total;

        let offers = token_offers
            .iter()
            .map(|(to, _tl, _tk)| to)
            .collect::<Vec<_>>();
        let listings = token_offers
            .iter()
            .map(|(_to, tl, _tk)| tl.get_list_id())
            .collect::<Vec<_>>();
        let token_keys = token_offers
            .iter()
            .map(|(_to, _tl, tk)| tk)
            .collect::<Vec<_>>();
        let offer_num = token_offers
            .iter()
            .map(|(_to, tl, _tk)| tl.num_offers)
            .collect::<Vec<_>>();
        log_make_offer(offers, token_keys, listings, offer_num);
    }

    /// Withdraw the escrow deposited for an `Offer`. This function may only be
    /// called on an `Offer` that has been active for a minimum length of time,
    /// specified by `self.min_offer_hours`.
    pub fn withdraw_offer(&mut self, token_key: String) {
        let mut token = self.get_token_internal(token_key.clone());
        token.assert_not_locked();
        near_assert!(
            token.current_offer.as_ref().expect("no current offer").from
                == env::predecessor_account_id(),
            "An offer can only be withdrawn by the account that placed it"
        );

        // if the minimum time has elapsed, refund the offerer
        let ns_elapsed = env::block_timestamp()
            - token.current_offer.as_ref().unwrap().timestamp.0;
        let offer_id = token.current_offer.as_ref().unwrap().id;
        let min_ns_elapsed = self.min_offer_hours * 10u64.pow(9) * 3600;

        if ns_elapsed > min_ns_elapsed {
            self.try_refund_offerer(&mut token);
            self.listings.insert(&token_key.as_str().into(), &token);
            log_withdraw_token_offer(&token.get_list_id(), offer_id);
        } else {
            near_panic!(
                "Cannot withdraw offer within {} hours of placing it",
                self.min_offer_hours
            );
            // env::panic_str(
            //     format!(
            //         "{} hours must elapse after offer posting",
            //         self.min_offer_hours,
            //     )
            //     .as_str(),
            // )
        }
    }

    /// Accept the `current_offer` for the `Token`.
    #[payable]
    pub fn accept_and_transfer(&mut self, token_key: String) {
        assert_one_yocto();
        let token = self.get_token_internal(token_key.clone());
        token.assert_not_locked();
        near_assert!(
            token.current_offer.is_some(),
            "There is no offer for this token"
        );
        // near_assert!(
        //     token.current_offer.as_ref().unwrap().is_active(),
        //     "Cannot accept inactive offer"
        // );
        self.assert_caller_owns_token(&token_key);
        // Assert that we can transfer the token locally.
        self.help_transfer(&token_key.as_str().into(), token);
    }

    fn ext_nft_transfer_payout(
        &self,
        receiver_id: AccountId,
        token_key: &TokenKey,
        approval_id: u64,
        balance: u128,
    ) -> Promise {
        let (token_id, store_id) =
            (token_key.token_id, token_key.account_id.clone());
        interfaces::ext_nft::ext(
            AccountId::from_str(store_id.as_str()).unwrap(),
        )
        .with_attached_deposit(ONE_YOCTO)
        .with_static_gas(gas::NFT_TRANSFER_PAYOUT)
        .nft_transfer_payout(
            receiver_id,
            token_id.to_string(),
            approval_id,
            balance.into(),
            MAX_LEN_PAYOUT,
        )
    }

    /// If NFT contract panicked, refund the token's offerer and drop the
    /// token. This method is pretty gas expensive:
    /// https://explorer.testnet.near.org/transactions/2KtnYo7EANwEmcG1HkGHkpr9Q5ZF2ifQpm2dF6JL6De4
    #[private]
    pub fn resolve_nft_payout(
        &mut self,
        token_key: String,
        token: TokenListing,
        others_keep: U128,
        market_keeps: U128,
    ) {
        let token_key = token_key.as_str().into();
        near_assert!(
            env::promise_results_count() == 1,
            "Wtf? Had more than one DataReceipt to process"
        );
        match env::promise_result(0) {
            near_sdk::PromiseResult::Successful(payout) => {
                match near_sdk::serde_json::from_slice(&payout) {
                    Ok(Payout { payout: p }) => {
                        // handle overflow risk:
                        let sum = p.iter().try_fold(0u128, |acc, (_, x)| {
                            acc.checked_add(x.0)
                        });

                        // 3 ways to get banned, each signaling a bad actor NFT contract:
                        if sum.is_none()
                            || sum.unwrap() > others_keep.into()
                            || p.len() > MAX_LEN_PAYOUT as usize
                        {
                            self.ban(&token_key, token);
                        } else {
                            log_sale(
                                &token.get_list_id(),
                                token.current_offer.as_ref().unwrap().id,
                                &token.get_token_key().to_string(),
                                &p,
                                market_keeps,
                            );
                            p.into_iter().for_each(|(account_id, pay)| {
                                self.tx_send(account_id, pay.into())
                            });
                            self.listings.remove(&token_key);
                        }
                    }
                    _ => {
                        // if payout is bad, the contract is bad. ban it and delist the token.
                        self.ban(&token_key, token);
                    }
                }
            }
            near_sdk::PromiseResult::Failed => {
                // refund token offerer and drop the token.
                self.delist_internal(&token_key, token);
            }
            near_sdk::PromiseResult::NotReady => {}
        }
    }

    /// Refund the originator of an `Offer`, if one exists. If one did exist and
    /// transfer was successful, return true.
    pub(crate) fn try_refund_offerer(&mut self, token: &mut TokenListing) {
        if let Some(old_offer) = std::mem::take(&mut token.current_offer) {
            self.tx_send(old_offer.from, old_offer.price);
        }
    }

    /// If the Token already has an offer, replace it if either:
    /// - the old offer is expired
    /// - the new offer has a higher price
    ///
    /// Refund the old offer if one exists.
    ///
    /// Note that `token.num_offers` MUST be updated in the calling method to
    /// avoid a race condition.
    fn try_make_offer(&mut self, token: &mut TokenListing, offer: TokenOffer) {
        near_assert!(
            self.owner_id != offer.from,
            "The market owner must not place offers"
        );
        near_assert!(
            !token.autotransfer || offer.price >= token.asking_price.into(),
            "Cannot set offer below ask for simple sales"
        );
        match &token.current_offer {
            None => {
                token.current_offer = Some(offer);
            }
            Some(old_offer) => {
                if !old_offer.is_active() || offer.price > old_offer.price {
                    let old_offer = std::mem::replace(
                        &mut token.current_offer,
                        Some(offer),
                    )
                    .unwrap();
                    log_withdraw_token_offer(
                        &token.get_list_id(),
                        old_offer.id,
                    );
                    // refund the prior offerer
                    self.tx_send(old_offer.from, old_offer.price);
                } else {
                    near_panic!(
                        "The offer must exceed the current offer price of {}",
                        old_offer.price
                    );
                    // env::panic_str(format!("must exceed: {}", old_offer.price).as_str());
                }
            }
        }
    }

    /// Called in two places:
    /// - by `accept_and_transfer` by `Token::owner_id`
    /// - by `make_offer' on a Token with autotransfer enabled.
    ///
    /// `help_transfer` triggers the following sequence of events:
    /// - First, the Token is locked. Whether or not the transfer succeeds, the
    ///   Token is no longer active on this Marketplace.
    /// - Next, the NFT Contract is queried for a Payout mapping.
    /// - If that succeeded, the Marketplace attempts to transfer the token.
    /// - Finally, the Marketplace handle the success or failure of the call in
    ///   `transfer_from_dispatcher`.
    fn help_transfer(&mut self, token_key: &TokenKey, mut token: TokenListing) {
        token.locked = true;
        self.listings.insert(token_key, &token);

        let price = token.current_offer.as_ref().unwrap().price;
        let market_keeps = self.take.multiply_balance(price);
        let others_keep = price - market_keeps;
        let receiver_id = AccountId::try_from(
            token.current_offer.as_ref().unwrap().from.to_string(),
        )
        .unwrap();
        self.ext_nft_transfer_payout(
            receiver_id,
            token_key,
            token.approval_id,
            others_keep,
        )
        .then(
            interfaces::ext_old_market::ext(env::current_account_id())
                .with_attached_deposit(NO_DEPOSIT)
                .with_static_gas(gas::PAYOUT_RESOLVE)
                .resolve_nft_payout(
                    token_key.to_string(),
                    token,
                    others_keep.into(),
                    market_keeps.into(),
                ),
        );
    }
}

fn log_sale(
    list_id: &str,
    offer_num: u64,
    token_key: &str,
    payout: &std::collections::HashMap<AccountId, U128>,
    mintbase_amount: U128,
) {
    let data = NftSaleData {
        list_id: list_id.to_string(),
        offer_num,
        token_key: token_key.to_string(),
        payout: payout.clone(),
        mintbase_amount: Some(mintbase_amount),
    };
    env::log_str(&data.serialize_event());
}

fn log_make_offer(
    offer: Vec<&TokenOffer>,
    token_key: Vec<&String>,
    list_id: Vec<String>,
    offer_num: Vec<u64>,
) {
    let data = NftMakeOfferData(
        offer
            .iter()
            .enumerate()
            .map(|(u, &x)| NftMakeOfferLog {
                offer: x.clone(),
                list_id: list_id[u].clone(),
                token_key: token_key[u].clone(),
                offer_num: offer_num[u],
            })
            .collect::<Vec<_>>(),
    );
    env::log_str(&data.serialize_event());
}

fn log_withdraw_token_offer(list_id: &str, offer_num: u64) {
    let data = NftWithdrawOfferData {
        offer_num,
        list_id: list_id.to_string(),
    };
    env::log_str(&data.serialize_event());
}
