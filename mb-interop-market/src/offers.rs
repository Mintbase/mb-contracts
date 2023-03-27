use mb_sdk::{
    data::store::Payout,
    events::market_v2 as events,
    interfaces::{
        ext_new_market,
        ext_nft,
    },
    near_assert,
    near_sdk::{
        self,
        env,
        json_types::U128,
        AccountId,
        Balance,
        Promise,
        PromiseOrValue,
    },
    utils::{
        ft_transfer,
        near_parse,
    },
};

use crate::{
    data::*,
    Market,
    MarketExt,
};

#[near_sdk::near_bindgen]
impl Market {
    // ----------------------------- offers (NEAR) -----------------------------
    /// Buying an NFT with native NEAR tokens. The transaction takes place
    /// according to the following rules:
    ///
    /// - The buyer must not be banned from using the market.
    /// - The NFT must be listed for NEAR, not an FT.
    /// - The listing must exist, otherwise the method panics and the buyer is
    ///   automatically refunded.
    /// - The attached deposit must equal or be larger than the price the NFT is
    ///   listed for. If it is larger, the whole deposit will be shared between
    ///   royalty holders and the market fee applies to the full deposit.
    /// - There must be no other offer currently executing on this listing.
    ///
    /// Should all these requirements be fullfilled, the offer will be inserted
    /// into the listing, blocking any other offers from executing on it.
    /// The market will call `nft_transfer_payout` on the NFT contract
    /// (processing a max of 50 royalty holders), and a cross-contract call
    /// `resolve_payout_near` on this market processes the payouts or failure
    /// of `nft_transfer_payout`.
    #[payable]
    pub fn buy(
        &mut self,
        nft_contract_id: AccountId,
        token_id: String,
        referrer_id: Option<AccountId>,
        affiliate_id: Option<AccountId>,
    ) -> Promise {
        self.assert_not_banned(&env::predecessor_account_id());

        let token_key = format!("{}<$>{}", nft_contract_id, token_id);
        let mut listing = match self.get_listing_internal(&token_key) {
            None => env::panic_str(ERR_LISTING_NOT_FOUND),
            Some(l) => l,
        };

        // Referrer/affiliate renaming with backwards compatibility
        // internally, this will be named referrer, externally affiliate
        near_assert!(
            referrer_id.is_none() || affiliate_id.is_none(),
            "You can either specify a referrer_id or an affiliate_id, but not both."
        );
        let referrer_id = referrer_id.or(affiliate_id);
        // Insert default cut for non-whitelisted referrers
        let referral_cut = referrer_id.as_ref().map(|account| {
            self.referrers.get(account).unwrap_or(self.fallback_cut)
        });

        // NFT must be listed for NEAR
        if let Currency::FtContract(ft_contract) = listing.currency {
            env::panic_str(&format!(
                "This NFT is not listed for NEAR, you must instead use `ft_transfer_call` on `{}`",
                ft_contract
            ))
        }
        // NEAR amount needs to be at least NFT asking price
        near_assert!(
            env::attached_deposit() >= listing.price,
            "Deposit needs to be higher than listing price"
        );
        // There must be no other offer in progress right now
        near_assert!(
            listing.current_offer.is_none(),
            "Another offer currently executes on this listing"
        );

        // Happy path: insert offer, log event, process stuff
        let offer = Offer {
            offerer_id: env::predecessor_account_id(),
            amount: env::attached_deposit(),
            referrer_id: referrer_id.clone(),
            referral_cut,
        };

        let (ref_earning, _) = self.get_affiliate_mintbase_amounts(&offer);
        env::log_str(
            // TODO: rename referrer -> affiliate once we can point indexer here
            &events::NftMakeOfferData {
                nft_contract_id,
                nft_token_id: token_id,
                nft_approval_id: listing.nft_approval_id,
                offer_id: 0,
                offerer_id: env::predecessor_account_id(),
                currency: listing.currency.to_string(),
                price: env::attached_deposit().into(),
                affiliate_id: referrer_id,
                affiliate_amount: ref_earning.map(Into::into),
            }
            .serialize_event(),
        );

        listing.current_offer = Some(offer);
        self.listings.insert(&token_key, &listing);

        self.execute_transfer(
            listing,
            env::predecessor_account_id(),
            env::attached_deposit(),
        )
    }

    /// Helper method to execute transfers for both NEAR or FT. Any checks must
    /// happen prior to calling this.
    fn execute_transfer(
        &mut self,
        listing: Listing,
        receiver_id: AccountId,
        balance: Balance,
    ) -> Promise {
        let token_key = listing.token_key();
        let offer = listing.current_offer.unwrap();
        let payout_percentage = match offer.referral_cut {
            Some(cut) => 10000 - cut,
            None => 10000 - self.fallback_cut,
        };

        let nft_transfer = ext_nft::ext(listing.nft_contract_id)
            .with_attached_deposit(1)
            .with_static_gas(NFT_TRANSFER_PAYOUT_GAS)
            .nft_transfer_payout(
                receiver_id,
                listing.nft_token_id,
                listing.nft_approval_id,
                (payout_percentage as u128 * balance / 10000).into(),
                if listing.currency.is_near() {
                    MAX_LEN_PAYOUT_NEAR
                } else {
                    MAX_LEN_PAYOUT_FT
                },
            );

        let callback = if listing.currency.is_near() {
            ext_new_market::ext(env::current_account_id())
                .with_static_gas(NFT_RESOLVE_PAYOUT_NEAR_GAS)
                .nft_resolve_payout_near(token_key)
        } else {
            ext_new_market::ext(env::current_account_id())
                .with_static_gas(NFT_RESOLVE_PAYOUT_FT_GAS)
                .nft_resolve_payout_ft(token_key)
        };

        nft_transfer.then(callback)
    }

    /// Resolving the payout after a token has been bought with NEAR.
    /// The following cases are possible:
    ///
    /// - The transfer failed: The offerer will be reimbursed, the listing
    ///   removed, and the lister will regain their storage deposit.
    /// - The transfer is still being processed: This callback will be retried.
    /// - The transfer succeeded and the payout is ill-formatted: The NFT
    ///   contract will be banned, the offerer reimbursed, the listing removed,
    ///   and the lister will regain their storage deposit.
    /// - The transfer succeeded, but the payout seems fishy: The NFT contract
    ///   will be banned, the offerer reimbursed, the listing removed, and the
    ///   lister will regain their storage deposit.
    /// - The transfer succeeded and the payout is legit: Market and affiliate
    ///   cuts are processed, royalty holders will be paid out, and the lister
    ///   will regain their storage deposit.
    #[private]
    pub fn nft_resolve_payout_near(
        &mut self,
        token_key: String,
    ) -> PromiseOrValue<()> {
        let listing = self.get_listing_internal(&token_key).unwrap();
        let offer = listing.current_offer.unwrap();
        let mut payout = match env::promise_result(0) {
            near_sdk::PromiseResult::NotReady => {
                return PromiseOrValue::Promise(
                    ext_new_market::ext(env::current_account_id())
                        .nft_resolve_payout_near(token_key),
                );
            }
            near_sdk::PromiseResult::Failed => {
                // FIXME: this should emit an event!
                Promise::new(offer.offerer_id).transfer(offer.amount);
                self.listings.remove(&token_key);
                self.refund_listings(&listing.nft_owner_id, 1);
                return PromiseOrValue::Value(());
            }

            near_sdk::PromiseResult::Successful(payout) => {
                match near_sdk::serde_json::from_slice::<Payout>(&payout) {
                    Ok(payout) => payout.payout,
                    // ill-formatted payout struct: refund offerer, ban NFT
                    // contract, then return
                    Err(_) => {
                        Promise::new(offer.offerer_id).transfer(offer.amount);
                        self.refund_listing_and_ban_nft_contract(
                            &token_key,
                            &listing.nft_owner_id,
                            &listing.nft_contract_id,
                        );
                        return PromiseOrValue::Value(());
                    }
                }
            }
        };

        let (ref_earning, mb_earning) =
            self.get_affiliate_mintbase_amounts(&offer);
        let sum: u128 = payout.values().map(|x| x.0).sum();

        // Given payouts sum is too large
        if sum > (offer.amount - mb_earning - ref_earning.unwrap_or(0)) {
            Promise::new(offer.offerer_id).transfer(offer.amount);
            self.refund_listing_and_ban_nft_contract(
                &token_key,
                &listing.nft_owner_id,
                &listing.nft_contract_id,
            );
            return PromiseOrValue::Value(());
        }
        // Given payout has too many recipients
        if payout.len() as u32 > MAX_LEN_PAYOUT_NEAR {
            Promise::new(offer.offerer_id).transfer(offer.amount);
            self.refund_listing_and_ban_nft_contract(
                &token_key,
                &listing.nft_owner_id,
                &listing.nft_contract_id,
            );
            return PromiseOrValue::Value(());
        }

        env::log_str(
            // TODO: rename referrer -> affiliate once we can point indexer here
            &events::NftSaleData {
                nft_contract_id: listing.nft_contract_id.clone(),
                nft_token_id: listing.nft_token_id.clone(),
                nft_approval_id: listing.nft_approval_id,
                accepted_offer_id: 0,
                payout: payout.clone(),
                currency: listing.currency.to_string(),
                price: offer.amount.into(),
                affiliate_id: offer.referrer_id.clone(),
                affiliate_amount: ref_earning.map(Into::into),
                mintbase_amount: mb_earning.into(),
            }
            .serialize_event(),
        );

        for (account, amount) in payout.drain() {
            Promise::new(account).transfer(amount.0);
        }
        if let Some(referrer_id) = offer.referrer_id {
            Promise::new(referrer_id).transfer(ref_earning.unwrap());
        }
        self.listings.remove(&token_key);
        self.refund_listings(&listing.nft_owner_id, 1);

        PromiseOrValue::Value(())
    }

    // ------------------------------ offers (FT) ------------------------------
    // Looking up the code on USN, the FT contract does the refund, such that
    // returning the correct value from this market contract is sufficient
    /// Facilitates buying via FT. Unlike buying with native NEAR tokens (which
    /// are attached to the `buy` call), this is a callback to
    /// `ft_transfer_call` on the FT contract. The transfer takes places
    /// according to the same rules as `buy` and:
    ///
    /// - The FT contract must not be banned.
    /// - The NFT must be listed for tokens from the calling FT contract.
    ///
    /// The following chain of cross-contract calls is the same as for the
    /// `buy` call. Due to gas constraints, FT listings are restricted to
    /// paying out 10 royalty holders.
    ///
    /// In general gas limits require lots of fine tuning, and might differ from
    /// FT contract to FT contract. If using this, make sure to attach the
    /// maximum of your open gas budget.
    pub fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        macro_rules! refund {
            ($msg:literal) => {
                env::log_str($msg);
                return PromiseOrValue::Value(amount);
            };
            ($msg:literal, $($fmt:expr),+) => {
                env::log_str(&format!($msg, $($fmt),+));
                return PromiseOrValue::Value(amount);
            };
        }

        let ft_contract_id = env::predecessor_account_id();
        let mut msg: BuyWithFtMessage =
            near_parse(&msg, "Invalid arguments to buy using FT");

        self.assert_not_banned(&sender_id);
        self.assert_not_banned(&ft_contract_id);

        let token_key = format!("{}<$>{}", msg.nft_contract_id, msg.token_id);
        let mut listing = match self.get_listing_internal(&token_key) {
            None => env::panic_str(ERR_LISTING_NOT_FOUND),
            Some(l) => l,
        };

        // Referrer/affiliate renaming with backwards compatibility
        near_assert!(
            msg.referrer_id.is_none() || msg.affiliate_id.is_none(),
            "You can either specify a referrer_id or an affiliate_id, but not both."
        );
        msg.referrer_id = msg.referrer_id.or(msg.affiliate_id);
        // Insert default cut for non-whitelisted referrers
        let referral_cut = msg.referrer_id.as_ref().map(|account| {
            self.referrers.get(account).unwrap_or(self.fallback_cut)
        });

        // NFT needs to be listed for FT
        if let Currency::Near = listing.currency {
            refund!("This NFT can only be bought with NEAR, refunding.");
        }
        // NFT needs to be listed for the transferred FT
        if let Currency::FtContract(requested_ft_contract_id) =
            listing.currency.clone()
        {
            if requested_ft_contract_id != ft_contract_id {
                refund!(
                    "This NFT can only be bought with FTs from {}, refunding.",
                    ft_contract_id
                );
            }
        }
        // FT amount needs to be at least NFT asking price
        if listing.price > amount.0 {
            refund!("You have not supplied sufficient funds to buy this token, refunding.");
        }
        // There must be no other offer in progress right now
        if listing.current_offer.is_some() {
            refund!("Another offer is currently being processed on this token, refunding.");
        }
        // // Referrer must be valid (or not present)
        // if msg.referrer_id.is_some() && referral_cut.is_none() {
        //     refund!(
        //         "{} is not an allowed referrer, refunding",
        //         msg.referrer_id.unwrap()
        //     );
        // }

        // Happy path: insert offer, log event, process stuff
        let offer = Offer {
            offerer_id: sender_id.clone(),
            amount: amount.0,
            referrer_id: msg.referrer_id.clone(),
            referral_cut,
        };

        let (ref_earning, _) = self.get_affiliate_mintbase_amounts(&offer);
        env::log_str(
            // TODO: rename referrer -> affiliate once open-sourced
            &events::NftMakeOfferData {
                nft_contract_id: msg.nft_contract_id,
                nft_token_id: msg.token_id,
                nft_approval_id: listing.nft_approval_id,
                offer_id: 0,
                offerer_id: sender_id.clone(),
                currency: listing.currency.to_string(),
                price: amount,
                affiliate_id: msg.referrer_id,
                affiliate_amount: ref_earning.map(Into::into),
            }
            .serialize_event(),
        );

        listing.current_offer = Some(offer);
        self.listings.insert(&token_key, &listing);

        PromiseOrValue::Promise(
            self.execute_transfer(listing, sender_id, amount.0),
        )
    }

    /// Payout resolution similar to `resolve_payout_near`, but with FT payouts
    /// instead of native NEAR tokens.
    #[private]
    pub fn nft_resolve_payout_ft(
        &mut self,
        token_key: String,
    ) -> PromiseOrValue<U128> {
        let listing = self.get_listing_internal(&token_key).unwrap();
        let offer = listing.current_offer.unwrap();
        let ft_contract_id = listing.currency.get_ft_contract_id().unwrap();
        let mut payout = match env::promise_result(0) {
            near_sdk::PromiseResult::NotReady => {
                return PromiseOrValue::Promise(
                    ext_new_market::ext(env::current_account_id())
                        .nft_resolve_payout_ft(token_key),
                );
            }
            near_sdk::PromiseResult::Failed => {
                self.listings.remove(&token_key);
                self.refund_listings(&listing.nft_owner_id, 1);
                return PromiseOrValue::Value(offer.amount.into());
            }

            near_sdk::PromiseResult::Successful(payout) => {
                match near_sdk::serde_json::from_slice::<Payout>(&payout) {
                    Ok(payout) => payout.payout,
                    Err(_) => {
                        self.refund_listing_and_ban_nft_contract(
                            &token_key,
                            &listing.nft_owner_id,
                            &listing.nft_contract_id,
                        );
                        return PromiseOrValue::Value(offer.amount.into());
                    }
                }
            }
        };

        let (ref_earning, mb_earning) =
            self.get_affiliate_mintbase_amounts(&offer);
        let sum: u128 = payout.values().map(|x| x.0).sum();

        // Given payout is too large
        if sum > (offer.amount - mb_earning - ref_earning.unwrap_or(0)) {
            self.refund_listing_and_ban_nft_contract(
                &token_key,
                &listing.nft_owner_id,
                &listing.nft_contract_id,
            );
            return PromiseOrValue::Value(offer.amount.into());
        }
        // Given payout is too large
        if payout.len() as u32 > MAX_LEN_PAYOUT_FT {
            self.refund_listing_and_ban_nft_contract(
                &token_key,
                &listing.nft_owner_id,
                &listing.nft_contract_id,
            );
            return PromiseOrValue::Value(offer.amount.into());
        }

        env::log_str(
            // TODO: rename referrer -> affiliate once open-sourced
            &events::NftSaleData {
                nft_contract_id: listing.nft_contract_id.clone(),
                nft_token_id: listing.nft_token_id.clone(),
                nft_approval_id: listing.nft_approval_id,
                accepted_offer_id: 0,
                payout: payout.clone(),
                currency: listing.currency.to_string(),
                price: offer.amount.into(),
                affiliate_id: offer.referrer_id.clone(),
                affiliate_amount: ref_earning.map(Into::into),
                mintbase_amount: mb_earning.into(),
            }
            .serialize_event(),
        );

        for (account, amount) in payout.drain() {
            ft_transfer(ft_contract_id.clone(), account, amount.0);
        }
        if let Some(referrer_id) = offer.referrer_id {
            ft_transfer(ft_contract_id, referrer_id, ref_earning.unwrap());
        }
        self.listings.remove(&token_key);
        self.refund_listings(&listing.nft_owner_id, 1);

        PromiseOrValue::Value(0.into())
    }

    // ---------------------------- offers (common) ----------------------------
    /// Calculate the amount that should be transferred to the affiliate and
    /// retained by the market, based on an offer.
    fn get_affiliate_mintbase_amounts(
        &self,
        offer: &Offer,
    ) -> (Option<Balance>, Balance) {
        match offer.referral_cut {
            Some(cut) => {
                let total_cut_amount = cut as u128 * offer.amount / 10_000;
                let mb_amount =
                    total_cut_amount * self.mintbase_cut as u128 / 10_000;
                let referrer_amount = total_cut_amount - mb_amount;
                (Some(referrer_amount), mb_amount)
            }
            None => (None, self.fallback_cut as u128 * offer.amount / 10_000),
        }
    }

    /// Removes a listing, refunds the storage deposit to the lister, and bans
    /// the NFT contract from using the market. This does explicitly NOT refund
    /// the offer amount, as the mechanism for differs between payments with
    /// FTs and payments with NEAR.
    fn refund_listing_and_ban_nft_contract(
        &mut self,
        token_key: &String,
        nft_owner_id: &AccountId,
        nft_contract_id: &AccountId,
    ) {
        self.listings.remove(token_key);
        self.refund_listings(nft_owner_id, 1);
        self.banned_accounts.insert(nft_contract_id);
    }

    /// Allows the market owner to remove offers. This is necessary as listings
    /// can be locked by offers that were not fully processed, originating
    /// usually from gas failures in `nft_resolve_payout_near` or
    /// `nft_resolve_payout_ft`.
    pub fn remove_offer(
        &mut self,
        nft_contract_id: AccountId,
        token_id: String,
    ) {
        // only owner is allowed to call this
        self.assert_predecessor_is_owner();

        // fetch listing
        let token_key = format!("{}<$>{}", nft_contract_id, token_id);
        let listing = self.get_listing_internal(&token_key);
        near_assert!(listing.is_some(), "Listing does not exist");
        let mut listing = listing.unwrap();
        near_assert!(
            listing.current_offer.is_some(),
            "Listing does not have an offer"
        );

        // remove offer and store
        listing.current_offer = None;
        self.listings.insert(&token_key, &listing);
    }
}
