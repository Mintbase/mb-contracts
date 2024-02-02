use mb_sdk::{
    data::store::{
        NFTContractMetadata,
        TokenMetadata,
    },
    events::store::NftContractMetadataUpdateLog,
    near_panic,
    near_sdk::{
        self,
        near_bindgen,
    },
};

use crate::*;

// --------------------- standardized metadata methods ---------------------- //
#[near_bindgen]
impl MintbaseStore {
    /// Contract-level metadata view method as described in
    /// [NEP-177](https://nomicon.io/Standards/Tokens/NonFungibleToken/Metadata)
    pub fn nft_metadata(&self) -> &NFTContractMetadata {
        &self.metadata
    }
}

// ------------------- non-standardized metadata methods -------------------- //
#[near_bindgen]
impl MintbaseStore {
    // -------------------------- change methods ---------------------------

    /// `icon_base64` is best understood as the `Store` logo/icon.
    ///
    /// Only the store owner may call this function.
    #[payable]
    pub fn set_icon_base64(&mut self, icon: Option<String>) {
        self.assert_store_owner();
        near_assert!(
            icon.as_ref().map(|b| b.len() <= 100).unwrap_or(true),
            "Icon URI must be less then 100 chars"
        );
        // log_set_icon_base64(&icon);
        env::log_str(
            &NftContractMetadataUpdateLog { memo: None }.serialize_event(),
        );
        self.metadata.icon = icon;
    }

    // -------------------------- view methods -----------------------------

    /// Get the on-contract metadata for a Token. Note that on-contract metadata
    /// is only a small subset of the metadata stored at the `token_uri`, which
    /// can be retrieved by calling `get_token_uri`. The metadata structure is not
    /// stored on the token, as this would lead to duplication of Metadata across
    /// tokens. Instead, the Metadata is stored in a Contract `LookupMap`.
    pub fn nft_token_metadata(&self, token_id: String) -> TokenMetadata {
        let token_id = parse_token_id(&token_id);
        let minting_metadata = self
            .token_metadata
            .get(&self.nft_token_internal(token_id).metadata_id)
            .expect("bad metadata_id");
        let mut metadata = minting_metadata.metadata;
        // This conversion might fail, but we need the u16 for compatibility!
        metadata.copies =
            Some((minting_metadata.minted - minting_metadata.burned) as u16);
        metadata
    }

    /// The Token URI is generated to index the token on whatever distributed
    /// storage platform this `Store` uses. Mintbase publishes token data on
    /// Arweave. `Store` owners may opt to use their own storage platform.
    pub fn nft_token_reference_uri(&self, token_id: String) -> String {
        let base = self.metadata.base_uri.clone();
        let reference = self.nft_token_metadata(token_id).reference;
        match (base, reference) {
            (Some(b), Some(r)) if r.starts_with(&b) => r,
            (Some(b), Some(r)) if b.ends_with('/') => format!("{}{}", b, r),
            (Some(b), Some(r)) => format!("{}/{}", b, r),
            (Some(b), None) => b,
            (None, Some(r)) => r,
            (None, None) => {
                near_panic!("Cannot construct URI, as neither base_uri nor reference exist.")
            }
        }
    }
}
