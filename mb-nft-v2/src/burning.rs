use mb_sdk::{
    assert_token_owned_by,
    assert_token_unloaned,
    events::store::NftBurnLog,
    near_sdk::{
        self,
        assert_one_yocto,
        env,
        near_bindgen,
    },
};

use crate::*;

#[near_bindgen]
impl MintbaseStore {
    // -------------------------- change methods ---------------------------

    /// The token will be permanently removed from this contract. Burn each
    /// token_id in `token_ids`.
    ///
    /// Only the tokens' owner may call this function.
    #[payable]
    pub fn nft_batch_burn(&mut self, token_ids: Vec<String>) {
        assert_one_yocto();
        assert!(!token_ids.is_empty());
        let token_ids_iter =
            token_ids.iter().map(|s| parse_token_id(s.as_str()));

        let account_id = env::predecessor_account_id();
        let mut set_owned =
            self.tokens_per_owner.get(&account_id).expect("none owned");

        token_ids_iter.for_each(|token_id_tuple| {
            let token = self.nft_token_internal(token_id_tuple);
            assert_token_unloaned!(token);
            assert_token_owned_by!(token, &account_id);

            // update the counts on token metadata and royalties stored
            let metadata_id =
                self.nft_token_internal(token_id_tuple).metadata_id;
            let mut minting_metadata =
                self.token_metadata.get(&metadata_id).unwrap();
            let count = minting_metadata.minted - minting_metadata.burned;
            if count > 1 {
                minting_metadata.burned += 1;
                self.token_metadata.insert(&metadata_id, &minting_metadata);
            } else {
                self.token_metadata.remove(&metadata_id);
                self.token_royalty.remove(&metadata_id);
            }

            set_owned.remove(&token_id_tuple);
            let (metadata_id, token_id) = token.id_tuple();
            let mut metadata_tokens = self
                .tokens
                .get(&metadata_id)
                .expect("This metadata does not yet exist in storage!");
            metadata_tokens.remove(&token_id);
            self.tokens.insert(&metadata_id, &metadata_tokens);
        });

        if set_owned.is_empty() {
            self.tokens_per_owner.remove(&account_id);
        } else {
            self.tokens_per_owner.insert(&account_id, &set_owned);
        }
        self.tokens_burned += token_ids.len() as u64;
        log_nft_batch_burn(token_ids, account_id.to_string());
    }

    // -------------------------- view methods -----------------------------
    // -------------------------- private methods --------------------------
    // -------------------------- internal methods -------------------------
}

fn log_nft_batch_burn(token_ids: Vec<String>, owner_id: String) {
    let log = NftBurnLog {
        owner_id,
        authorized_id: None,
        token_ids,
        memo: None,
    };

    env::log_str(log.serialize_event().as_str());
}
