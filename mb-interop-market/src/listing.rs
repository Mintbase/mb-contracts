use mb_sdk::{
    events::market_v2 as events,
    near_assert,
    near_sdk::{
        self,
        env,
        AccountId,
    },
    utils::{
        assert_predecessor,
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
    /// This is called when a token is approved on an NFT contract for this
    /// market. The method creates the listing according to the following rules:
    ///
    /// - The NFT contract and the token owner must not be banned. If the NFT is
    ///   listed for an FT, the FT contract must not be banned.
    /// - The `token_id` must not be larger than 128 bytes. This is to prevent
    ///   a storage staking attack by large token IDs
    /// - The owner must have sufficient storage deposits to cover the listing.
    pub fn nft_on_approve(
        &mut self,
        token_id: String,
        owner_id: AccountId,
        approval_id: u64,
        msg: String,
    ) {
        let nft_contract_id = env::predecessor_account_id();
        let msg: CreateListingMsg =
            near_parse(&msg, "Invalid arguments to create listing");
        let listing =
            Listing::new(token_id, approval_id, owner_id, nft_contract_id, msg);

        // No involved party must be banned from using the market
        self.assert_not_banned(&listing.nft_owner_id);
        self.assert_not_banned(&listing.nft_contract_id);
        if let Currency::FtContract(ft_contract_id) = listing.currency.clone() {
            self.assert_not_banned(&ft_contract_id)
        }
        // Token IDs must not be longer than 128 bytes to guard against the
        // million cheap data additions attack
        near_assert!(
            listing.nft_token_id.len() <= 128,
            "Cannot process token IDs with more than 128 bytes"
        );
        // Lister must have purchased storage for processing
        near_assert!(
            self.free_storage_deposit(&listing.nft_owner_id)
                >= self.listing_storage_deposit,
            "Storage for listing not covered"
        );

        self.increase_listings_count(&listing.nft_owner_id, 1);
        if let Some(old_listing) =
            self.listings.insert(&listing.token_key(), &listing)
        {
            if listing.current_offer.is_some() {
                env::panic_str(ERR_OFFER_IN_PROGRESS);
            }
            env::log_str(
                &events::NftUnlistData {
                    nft_contract_id: old_listing.nft_contract_id,
                    nft_token_id: old_listing.nft_token_id,
                    nft_approval_id: old_listing.nft_approval_id,
                }
                .serialize_event(),
            );
        }

        env::log_str(
            &events::NftListData {
                kind: LISTING_KIND_SIMPLE.to_string(),
                nft_token_id: listing.nft_token_id,
                nft_approval_id: listing.nft_approval_id,
                nft_owner_id: listing.nft_owner_id,
                nft_contract_id: listing.nft_contract_id,
                currency: listing.currency.to_string(),
                price: listing.price.into(),
            }
            .serialize_event(),
        )
    }

    /// Allows a token owner to unlist tokens from this marketplace. The
    /// storage deposit will be refunded automatically. Unlike listing, multiple
    /// tokens can be unlisted at once, but only if they live on the same smart
    /// contract.
    #[payable]
    pub fn unlist(
        &mut self,
        nft_contract_id: AccountId,
        token_ids: Vec<String>,
    ) {
        for token_id in token_ids.iter() {
            let listing = self.unlist_single_nft(&format!(
                "{}<$>{}",
                nft_contract_id, token_id
            ));

            env::log_str(
                &events::NftUnlistData {
                    nft_contract_id: nft_contract_id.clone(),
                    nft_token_id: token_id.clone(),
                    nft_approval_id: listing.nft_approval_id,
                }
                .serialize_event(),
            );
        }

        self.refund_listings(
            &env::predecessor_account_id(),
            token_ids.len() as u64,
        );
    }

    /// Internally used for unlisting NFTs, panics if withdrawal is impossible
    /// or method is not called by token owner
    fn unlist_single_nft(&mut self, token_key: &String) -> Listing {
        let listing = match self.get_listing_internal(token_key) {
            None => env::panic_str(ERR_LISTING_NOT_FOUND),
            Some(l) => l,
        };

        if listing.current_offer.is_some() {
            env::panic_str(ERR_OFFER_IN_PROGRESS);
        }

        let minimum_withdrawal_timestamp =
            listing.created_at + self.listing_lock_seconds * 1_000_000_000;

        assert_predecessor(&listing.nft_owner_id);
        near_assert!(
            env::block_timestamp() > minimum_withdrawal_timestamp,
            "Listing cannot be withdrawn before timestamp {}",
            minimum_withdrawal_timestamp / 1_000_000_000
        );

        self.listings.remove(&listing.token_key());
        listing
    }

    /// Show a listing.
    pub fn get_listing(
        &self,
        nft_contract_id: AccountId,
        token_id: String,
    ) -> Option<ListingJson> {
        self.get_listing_internal(&format!(
            "{}<$>{}",
            nft_contract_id, token_id
        ))
        .map(Into::into)
    }

    pub(crate) fn get_listing_internal(
        &self,
        token_key: &String,
    ) -> Option<Listing> {
        self.listings.get(token_key)
    }
}
