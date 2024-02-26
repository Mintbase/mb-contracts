use mb_sdk::{
    data::store::TokenMetadata,
    events::store::{
        MintingMetadataUpdateData,
        NftMetadataUpdateLog,
    },
    near_sdk::{
        self,
        near_bindgen,
    },
};

use crate::*;

#[near_bindgen]
impl MintbaseStore {
    #[payable]
    pub fn nft_metadata_update(
        &mut self,
        metadata_id: U64,
        metadata: TokenMetadata,
    ) {
        // Get metadata: needs to exist
        let mut minting_metadata = self.get_minting_metadata(metadata_id.0);

        // Only creator of metadata is allowed to update it (require yoctoNEAR deposit)
        near_sdk::assert_one_yocto();
        near_assert!(
            minting_metadata.creator == env::predecessor_account_id(),
            "This method can only be called by the metadata creator"
        );

        // Metadata must not be locked
        near_assert!(!minting_metadata.is_locked, "Metadata is locked");

        // FIXME: new metadata needs to be validated
        // Update the metadata
        minting_metadata.metadata = metadata;
        self.token_metadata
            .insert(&metadata_id.0, &minting_metadata);

        // Get token IDs and emit the event
        let token_ids: Vec<_> = self
            .tokens
            .get(&metadata_id.0)
            .expect("metadata existence was verified earlier")
            .into_iter()
            .map(|(token_id, _)| format!("{}:{}", metadata_id.0, token_id))
            .collect();
        log_nft_metadata_update(token_ids);
    }

    #[payable]
    pub fn lock_metadata(&mut self, metadata_id: U64) {
        // Get metadata: needs to exist
        let mut minting_metadata = self.get_minting_metadata(metadata_id.0);

        // Only creator of metadata is allowed to lock it (require yoctoNEAR deposit)
        near_sdk::assert_one_yocto();
        near_assert!(
            minting_metadata.creator == env::predecessor_account_id(),
            "This method can only be called by the metadata creator"
        );

        // Must not be locked already
        near_assert!(!minting_metadata.is_locked, "Metadata is already locked");

        // Lock it
        minting_metadata.is_locked = true;
        self.token_metadata
            .insert(&metadata_id.0, &minting_metadata);

        // Emit event
        log_token_lock(metadata_id.0);
    }
}

fn log_nft_metadata_update(token_ids: Vec<String>) {
    env::log_str(&NftMetadataUpdateLog { token_ids }.serialize_event())
}

fn log_token_lock(metadata_id: u64) {
    env::log_str(
        &MintingMetadataUpdateData {
            metadata_id: metadata_id.into(),
            minters_allowlist: None,
            price: None,
            is_dynamic: Some(false),
        }
        .serialize_event(),
    )
}
