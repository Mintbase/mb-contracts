#[cfg(feature = "factory-wasm")]
#[near_events::near_event_data(
    standard = "mb_store",
    version = "0.1.0",
    event = "deploy"
)]
pub struct MbStoreDeployData {
    pub contract_metadata: crate::store_data::NFTContractMetadata,
    pub owner_id: String,
    pub store_id: String,
}
