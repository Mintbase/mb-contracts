use mb_sdk::{
    data::store::TokenMetadata,
    near_sdk::{self, near_bindgen},
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
            "This method can only be called by the store owner"
        );

        // Metadata must not be locked
        near_assert!(minting_metadata.is_locked == false, "Metadata is locked");

        // Update the metadata
        minting_metadata.metadata = metadata;
        self.token_metadata
            .insert(&metadata_id.0, &minting_metadata);

        // TODO: events
        // Figure out token IDs and emit the event
        // Problem: specified token IDs
        //   Solution 1: Iterate over all minted tokens on a smart contract -> might fail
        //   Solution 2: Cannot specify token IDs for unlocked metadata.
        //   Solution 3: Nested structure for self.tokens
    }

    #[payable]
    pub fn lock_metadata(&mut self, metadata_id: U64) {
        // Get metadata: needs to exist
        let mut minting_metadata = self.get_minting_metadata(metadata_id.0);

        // Only creator of metadata is allowed to lock it (require yoctoNEAR deposit)
        near_sdk::assert_one_yocto();
        near_assert!(
            minting_metadata.creator == env::predecessor_account_id(),
            "This method can only be called by the store owner"
        );

        // Must not be locked already
        near_assert!(
            minting_metadata.is_locked == false,
            "Metadata is already locked"
        );

        // Lock it
        minting_metadata.is_locked = true;
        self.token_metadata
            .insert(&metadata_id.0, &minting_metadata);

        // Emit event
        // TODO:
    }
}
