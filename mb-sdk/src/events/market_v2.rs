use std::collections::HashMap;

use near_events::near_event_data;
// #[cfg(feature = "de")]
// use near_sdk::serde::Deserialize;
// #[cfg(feature = "ser")]
// use near_sdk::serde::Serialize;
use near_sdk::{
    json_types::U128,
    AccountId,
};

#[cfg_attr(feature = "all", derive(Clone, Debug))]
#[near_event_data(
    standard = "mb_market",
    version = "0.2.1",
    event = "nft_list"
)]
pub struct NftListData {
    pub kind: String,
    pub nft_contract_id: AccountId,
    pub nft_token_id: String,
    pub nft_approval_id: u64,
    pub nft_owner_id: AccountId,
    pub currency: String,
    pub price: U128,
}

// This could be more efficient by vectorizing token IDs and approval IDs, but
// leads to more code complexity -> do when necessary
#[cfg_attr(feature = "all", derive(Clone, Debug))]
#[near_event_data(
    standard = "mb_market",
    version = "0.2.1",
    event = "nft_unlist"
)]
pub struct NftUnlistData {
    pub nft_contract_id: AccountId,
    pub nft_token_id: String,
    pub nft_approval_id: u64,
}

#[cfg_attr(feature = "all", derive(Clone, Debug))]
#[near_event_data(
    standard = "mb_market",
    version = "0.2.2",
    event = "nft_sale"
)]
pub struct NftSaleDataV022 {
    pub nft_contract_id: AccountId,
    pub nft_token_id: String,
    pub nft_approval_id: u64,
    pub accepted_offer_id: u64,
    pub payout: HashMap<AccountId, U128>,
    pub currency: String,
    pub price: U128,
    pub referrer_id: Option<AccountId>,
    pub referral_amount: Option<U128>,
    // this field should always be populated, `Option` for backwards
    // compatibility of generated JSON
    pub mintbase_amount: Option<U128>,
}

#[cfg_attr(feature = "all", derive(Clone, Debug))]
#[near_event_data(
    standard = "mb_market",
    version = "0.3.0",
    event = "nft_sale"
)]
pub struct NftSaleData {
    pub nft_contract_id: AccountId,
    pub nft_token_id: String,
    pub nft_approval_id: u64,
    pub accepted_offer_id: u64,
    pub payout: HashMap<AccountId, U128>,
    pub currency: String,
    pub price: U128,
    pub affiliate_id: Option<AccountId>,
    pub affiliate_amount: Option<U128>,
    // this field should always be populated, `Option` for backwards
    // compatibility of generated JSON
    pub mintbase_amount: U128,
}

#[cfg_attr(feature = "all", derive(Clone, Debug))]
#[near_event_data(
    standard = "mb_market",
    version = "0.2.1",
    event = "nft_make_offer"
)]
pub struct NftMakeOfferDataV021 {
    pub nft_contract_id: AccountId,
    pub nft_token_id: String,
    pub nft_approval_id: u64,
    pub currency: String,
    pub offer_id: u64,
    pub offerer_id: AccountId,
    pub price: U128,
    pub referrer_id: Option<AccountId>,
    pub referral_amount: Option<U128>,
}

#[cfg_attr(feature = "all", derive(Clone, Debug))]
#[near_event_data(
    standard = "mb_market",
    version = "0.3.0",
    event = "nft_make_offer"
)]
pub struct NftMakeOfferData {
    pub nft_contract_id: AccountId,
    pub nft_token_id: String,
    pub nft_approval_id: u64,
    pub currency: String,
    pub offer_id: u64,
    pub offerer_id: AccountId,
    pub price: U128,
    pub affiliate_id: Option<AccountId>,
    pub affiliate_amount: Option<U128>,
}

#[cfg_attr(feature = "all", derive(Clone, Debug))]
#[near_event_data(
    standard = "mb_market",
    version = "0.2.1",
    event = "nft_withdraw_offer"
)]
pub struct NftWithdrawOfferData {
    pub nft_contract_id: AccountId,
    pub nft_token_id: String,
    pub offer_id: u64,
}

#[cfg_attr(feature = "all", derive(Clone, Debug))]
#[near_event_data(
    standard = "mb_market",
    version = "0.2.1",
    event = "nft_failed_listing"
)]
pub struct NftFailedSaleData {
    pub nft_contract_id: AccountId,
    pub nft_token_id: String,
    pub nft_approval_id: u64,
    pub offer_id: u64,
}
