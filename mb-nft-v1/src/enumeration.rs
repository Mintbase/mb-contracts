use mb_sdk::{
    data::store::TokenCompliant,
    near_sdk::{
        self,
        json_types::U64,
        near_bindgen,
        AccountId,
    },
};

use crate::*;

// -------------------- standardized enumeration methods -------------------- //
#[near_bindgen]
impl MintbaseStore {
    /// Total number of available NFTs on this smart contract according to
    /// [NEP-181](https://nomicon.io/Standards/Tokens/NonFungibleToken/Enumeration)
    pub fn nft_total_supply(&self) -> U64 {
        (self.tokens_minted - self.tokens_burned).into()
    }

    /// List NFTs according to
    /// [NEP-181](https://nomicon.io/Standards/Tokens/NonFungibleToken/Enumeration)
    pub fn nft_tokens(
        &self,
        from_index: Option<String>, // default: "0"
        limit: Option<u32>,         // default: = self.nft_total_supply()
    ) -> Vec<TokenCompliant> {
        let from_index: u64 = from_index
            .unwrap_or_else(|| "0".to_string())
            .parse()
            .unwrap();
        let to_index = match limit {
            Some(limit) => from_index + limit as u64,
            None => self.tokens_minted,
        };
        (from_index..to_index)
            .flat_map(|token_id| self.nft_token_compliant_internal(token_id))
            .collect()
    }

    /// Total number of available NFTs for specified owner according to
    /// [NEP-181](https://nomicon.io/Standards/Tokens/NonFungibleToken/Enumeration)
    pub fn nft_supply_for_owner(&self, account_id: AccountId) -> U64 {
        self.tokens_per_owner
            .get(&account_id)
            .map(|v| v.len())
            .unwrap_or(0)
            .into()
    }

    /// List NFTs for specified owner according to
    /// [NEP-181](https://nomicon.io/Standards/Tokens/NonFungibleToken/Enumeration)
    pub fn nft_tokens_for_owner(
        &self,
        account_id: AccountId,
        from_index: Option<String>,
        limit: Option<u32>,
    ) -> Vec<TokenCompliant> {
        let limit = limit.map(|l| l as u64);
        self.tokens_per_owner
            .get(&account_id)
            .expect("no tokens")
            .iter()
            .skip(
                from_index
                    .unwrap_or_else(|| "0".to_string())
                    .parse()
                    .unwrap(),
            )
            .take(
                limit
                    .unwrap_or(self.tokens_minted)
                    .try_into()
                    .expect("Too many tokens to convert into wasm32 usize"),
            )
            .flat_map(|x| self.nft_token_compliant_internal(x))
            .collect::<Vec<_>>()
    }
}
