use near_events::{
    near_event_data,
    near_event_data_log,
};
#[cfg(feature = "de")]
use near_sdk::serde::Deserialize;
#[cfg(feature = "ser")]
use near_sdk::serde::Serialize;
use near_sdk::{
    json_types::U64,
    AccountId,
};

// ----------------------------- Core (NEP171) ------------------------------ //
#[cfg_attr(feature = "all", derive(Clone, Debug))]
#[near_event_data_log(
    standard = "nep171",
    version = "1.0.0",
    event = "nft_mint"
)]
pub struct NftMintLog {
    pub owner_id: String,
    pub token_ids: Vec<String>,
    pub memo: Option<String>,
}

#[near_event_data_log(
    standard = "nep171",
    version = "1.0.0",
    event = "nft_burn"
)]
pub struct NftBurnLog {
    pub owner_id: String,
    pub authorized_id: Option<String>,
    pub token_ids: Vec<String>,
    pub memo: Option<String>,
}

#[cfg_attr(feature = "ser", derive(Serialize))]
#[cfg_attr(feature = "de", derive(Deserialize))]
#[cfg_attr(
    any(feature = "ser", feature = "de"),
    serde(crate = "near_sdk::serde")
)]
pub struct NftTransferLog {
    pub authorized_id: Option<String>,
    pub old_owner_id: String,
    pub new_owner_id: String,
    pub token_ids: Vec<String>,
    pub memo: Option<String>,
}

#[near_event_data(
    standard = "nep171",
    version = "1.0.0",
    event = "nft_transfer"
)]
pub struct NftTransferData(pub Vec<NftTransferLog>);

#[cfg_attr(feature = "ser", derive(near_sdk::serde::Serialize))]
#[cfg_attr(feature = "de", derive(near_sdk::serde::Deserialize))]
#[cfg_attr(
    any(feature = "ser", feature = "de"),
    serde(crate = "near_sdk::serde")
)]
pub struct NftMintLogMemo {
    pub royalty: Option<crate::data::store::Royalty>,
    pub split_owners: Option<crate::data::store::SplitOwners>,
    pub meta_id: Option<String>,
    pub meta_extra: Option<String>,
    pub minter: String,
}

#[near_event_data(
    standard = "nep171",
    version = "1.1.0",
    event = "contract_metadata_update"
)]
pub struct NftContractMetadataUpdateLog {
    pub memo: Option<String>,
}

#[near_event_data_log(
    standard = "nep171",
    version = "1.2.0",
    event = "nft_metadata_update"
)]
pub struct NftMetadataUpdateLog {
    pub token_ids: Vec<String>,
}

// --------------------------- Metadata creation ---------------------------- //
#[cfg_attr(feature = "all", derive(Debug, Clone))]
#[near_event_data(
    standard = "mb_store",
    version = "2.0.0",
    event = "create_metadata"
)]
pub struct CreateMetadataData {
    pub metadata_id: U64,
    pub creator: AccountId,
    pub minters_allowlist: Option<Vec<AccountId>>,
    pub unique_minters: bool,
    pub price: near_sdk::json_types::U128,
    pub ft_contract_id: Option<AccountId>,
    pub royalty: Option<crate::data::store::Royalty>,
    pub max_supply: Option<u32>,
    pub starts_at: Option<U64>,
    pub expires_at: Option<U64>,
    pub is_locked: bool,
}

#[cfg_attr(feature = "all", derive(Debug, Clone))]
#[near_event_data(
    standard = "mb_store",
    version = "2.0.0",
    event = "minting_metadata_update"
)]
pub struct MintingMetadataUpdateData {
    pub metadata_id: U64,
    // TODO: method
    pub minters_allowlist: Option<Vec<AccountId>>,
    // TODO: method
    pub price: Option<near_sdk::json_types::U128>,
    pub is_dynamic: Option<bool>,
}

// ------------------------------- Approvals -------------------------------- //
#[cfg_attr(feature = "ser", derive(near_sdk::serde::Serialize))]
#[cfg_attr(feature = "de", derive(near_sdk::serde::Deserialize))]
#[cfg_attr(
    any(feature = "ser", feature = "de"),
    serde(crate = "near_sdk::serde")
)]
pub struct NftApproveLog {
    pub token_id: String,
    pub approval_id: u64,
    pub account_id: String,
}

#[near_event_data(
    standard = "mb_store",
    version = "0.1.0",
    event = "nft_approve"
)]
pub struct NftApproveData(pub Vec<NftApproveLog>);

#[near_event_data(
    standard = "mb_store",
    version = "0.1.0",
    event = "nft_revoke"
)]
pub struct NftRevokeData {
    pub token_id: String,
    pub account_id: String,
}

#[near_event_data(
    standard = "mb_store",
    version = "0.1.0",
    event = "nft_revoke_all"
)]
pub struct NftRevokeAllData {
    pub token_id: String,
}

// -------------------------------- Payouts --------------------------------- //
#[cfg_attr(feature = "all", derive(Debug, Clone))]
#[near_event_data(
    standard = "mb_store",
    version = "0.1.0",
    event = "nft_set_split_owners"
)]
pub struct NftSetSplitOwnerData {
    pub token_ids: Vec<String>,
    pub split_owners: std::collections::HashMap<AccountId, u16>,
}

// ----------------------------- Store settings ----------------------------- //
#[near_event_data(
    standard = "mb_store",
    version = "0.1.0",
    event = "change_setting"
)]
pub struct MbStoreChangeSettingDataV010 {
    pub granted_minter: Option<String>,
    pub revoked_minter: Option<String>,
    pub new_owner: Option<String>,
    pub new_icon_base64: Option<String>, // deprecated in favor of metadata update
    pub new_base_uri: Option<String>,
}

impl MbStoreChangeSettingDataV010 {
    pub fn empty() -> Self {
        MbStoreChangeSettingDataV010 {
            granted_minter: None,
            revoked_minter: None,
            new_owner: None,
            new_icon_base64: None,
            new_base_uri: None,
        }
    }
}

#[near_event_data(
    standard = "mb_store",
    version = "0.2.0",
    event = "change_setting"
)]
pub struct MbStoreChangeSettingDataV020 {
    pub granted_minter: Option<String>,
    pub revoked_minter: Option<String>,
    pub new_owner: Option<String>,
    pub new_icon_base64: Option<String>, // deprecated in favor of metadata update
    pub new_base_uri: Option<String>,
    pub set_minting_cap: Option<U64>,
    pub allow_open_minting: Option<bool>,
}

impl MbStoreChangeSettingDataV020 {
    pub fn empty() -> Self {
        MbStoreChangeSettingDataV020 {
            granted_minter: None,
            revoked_minter: None,
            new_owner: None,
            new_icon_base64: None,
            new_base_uri: None,
            set_minting_cap: None,
            allow_open_minting: None,
        }
    }
}
