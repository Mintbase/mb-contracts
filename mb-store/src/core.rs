use std::{
    collections::HashMap,
    convert::TryFrom,
};

// logging functions
use mb_sdk::near_sdk::json_types::U64;
// contract interface modules
use mb_sdk::{
    assert_token_owned_by,
    assert_token_owned_or_approved,
    assert_token_unloaned,
    constants::gas,
    data::store::{
        Owner,
        Token,
        TokenCompliant,
    },
    events::store::{
        NftTransferData,
        NftTransferLog,
    },
    interfaces::ext_nft_on_transfer,
    near_assert,
    near_sdk::{
        self,
        assert_one_yocto,
        env,
        near_bindgen,
        AccountId,
        Promise,
        PromiseResult,
    },
};

use crate::*;

// ----------------------- standardized core methods ------------------------ //
#[near_bindgen]
impl MintbaseStore {
    // -------------------------- change methods ---------------------------

    /// Transfer function as specified by [NEP-171](https://nomicon.io/Standards/Tokens/NonFungibleToken/Core).
    #[payable]
    pub fn nft_transfer(
        &mut self,
        receiver_id: AccountId,
        token_id: U64,
        approval_id: Option<u64>,
        memo: Option<String>,
    ) {
        assert_one_yocto();
        let token_idu64 = token_id.into();
        let mut token = self.nft_token_internal(token_idu64);
        let old_owner = token.owner_id.to_string();
        assert_token_unloaned!(token);
        assert_token_owned_or_approved!(
            token,
            &env::predecessor_account_id(),
            approval_id
        );

        self.transfer_internal(&mut token, receiver_id.clone(), true);
        log_nft_transfer(&receiver_id, token_idu64, &memo, old_owner);
    }

    /// Transfer-and-call function as specified by [NEP-171](https://nomicon.io/Standards/Tokens/NonFungibleToken/Core).
    #[payable]
    pub fn nft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        token_id: U64,
        approval_id: Option<u64>,
        msg: String,
    ) -> Promise {
        assert_one_yocto();
        let token_idu64 = token_id.into();
        let mut token = self.nft_token_internal(token_idu64);
        let pred = env::predecessor_account_id();
        assert_token_unloaned!(token);
        assert_token_owned_or_approved!(token, &pred, approval_id);
        // prevent race condition, temporarily lock-replace owner
        let owner_id = AccountId::new_unchecked(token.owner_id.to_string());
        self.lock_token(&mut token);

        ext_nft_on_transfer::ext(receiver_id.clone())
            .with_static_gas(gas::NFT_TRANSFER_CALL)
            .nft_on_transfer(pred, owner_id.clone(), token_id, msg)
            .then(
                store_self::ext(env::current_account_id())
                    .with_static_gas(gas::NFT_TRANSFER_CALL)
                    .nft_resolve_transfer(
                        owner_id,
                        receiver_id,
                        token_id.0.to_string(),
                        None,
                    ),
            )
    }

    // -------------------------- view methods -----------------------------

    /// Token view method as specified by [NEP-171](https://nomicon.io/Standards/Tokens/NonFungibleToken/Core).
    pub fn nft_token(&self, token_id: U64) -> Option<TokenCompliant> {
        self.nft_token_compliant_internal(token_id.0)
    }

    // -------------------------- private methods --------------------------

    /// Call back of a transfer-and-call as specified by [NEP-171](https://nomicon.io/Standards/Tokens/NonFungibleToken/Core).
    #[private]
    pub fn nft_resolve_transfer(
        &mut self,
        owner_id: AccountId,
        receiver_id: AccountId,
        token_id: String,
        // NOTE: might borsh::maybestd::collections::HashMap be more appropriate?
        approved_account_ids: Option<HashMap<AccountId, u64>>,
    ) -> bool {
        let l = format!(
            "owner_id={} receiver_id={} token_id={} approved_ids={:?} pred={}",
            owner_id,
            receiver_id,
            token_id,
            approved_account_ids,
            env::predecessor_account_id()
        );
        env::log_str(l.as_str());
        let token_id_u64 = token_id.parse::<u64>().unwrap();
        let mut token = self.nft_token_internal(token_id_u64);
        self.unlock_token(&mut token);
        near_assert!(
            env::promise_results_count() == 1,
            "Wtf? Had more than one DataReceipt to process"
        );
        // Get whether token should be returned
        let must_revert = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(value) => {
                if let Ok(yes_or_no) =
                    near_sdk::serde_json::from_slice::<bool>(&value)
                {
                    yes_or_no
                } else {
                    true
                }
            }
            PromiseResult::Failed => true,
        };
        if !must_revert {
            true
        } else {
            self.transfer_internal(&mut token, receiver_id.clone(), true);
            log_nft_transfer(
                &receiver_id,
                token_id_u64,
                &None,
                owner_id.to_string(),
            );
            false
        }
    }

    /// Locking an NFT during a transfer-and-call chain
    fn lock_token(&mut self, token: &mut Token) {
        if let Owner::Account(ref s) = token.owner_id {
            token.owner_id = Owner::Lock(s.clone());
            self.tokens.insert(&token.id, token);
        }
    }

    /// Unlocking an NFT after a transfer-and-call chain
    fn unlock_token(&mut self, token: &mut Token) {
        if let Owner::Lock(ref s) = token.owner_id {
            token.owner_id = Owner::Account(s.clone());
            self.tokens.insert(&token.id, token);
        }
    }
}

// --------------------- non-standardized core methods ---------------------- //
#[near_bindgen]
impl MintbaseStore {
    // -------------------------- change methods ---------------------------

    /// Like `nft_transfer`, but allows transferring multiple tokens in a
    /// single call.
    #[payable]
    pub fn nft_batch_transfer(&mut self, token_ids: Vec<(U64, AccountId)>) {
        assert_one_yocto();
        near_assert!(!token_ids.is_empty(), "Token IDs cannot be empty");
        let pred = env::predecessor_account_id();
        let mut set_owned =
            self.tokens_per_owner.get(&pred).expect("none owned");
        let (tokens, accounts, old_owners) = token_ids
            .into_iter()
            .map(|(token_id, account_id)| {
                let token_idu64 = token_id.into();
                let mut token = self.nft_token_internal(token_idu64);
                let old_owner = token.owner_id.to_string();
                assert_token_unloaned!(token);
                assert_token_owned_by!(token, &pred);
                near_assert!(
                    account_id.to_string() != token.owner_id.to_string(),
                    "Token {} is already owned by {}",
                    token.id,
                    account_id
                ); // can't transfer to self
                self.transfer_internal(&mut token, account_id.clone(), false);
                set_owned.remove(&token_idu64);
                (token_id, account_id, old_owner)
            })
            .fold((vec![], vec![], vec![]), |mut acc, (tid, aid, oid)| {
                acc.0.push(tid);
                acc.1.push(aid);
                acc.2.push(oid);
                acc
            });
        self.tokens_per_owner.insert(&pred, &set_owned);
        log_nft_batch_transfer(&tokens, &accounts, old_owners);
    }

    // -------------------------- view methods -----------------------------

    // -------------------------- private methods --------------------------

    // -------------------------- internal methods -------------------------

    /// Set the owner of `token` to `to` and clear the approvals on the
    /// token. Update the `tokens_per_owner` sets. `remove_prior` is an
    /// optimization on batch removal, in particular useful for batch sending
    /// of tokens.
    ///
    /// If remove prior is true, expect that the token is not composed, and
    /// remove the token owner from self.tokens_per_owner.
    pub(crate) fn transfer_internal(
        &mut self,
        token: &mut Token,
        to: AccountId,
        remove_prior: bool,
    ) {
        let update_set = if remove_prior {
            Some(AccountId::try_from(token.owner_id.to_string()).unwrap())
        } else {
            None
        };
        token.split_owners = None;
        self.update_tokens_per_owner(token.id, update_set, Some(to.clone()));
        token.owner_id = Owner::Account(to);
        token.approvals.clear();
        self.tokens.insert(&token.id, token);
    }

    /// Gets the token as stored on the smart contract
    pub(crate) fn nft_token_internal(&self, token_id: u64) -> Token {
        self.tokens
            .get(&token_id)
            .unwrap_or_else(|| panic!("token: {} doesn't exist", token_id))
    }

    /// Gets the token as specified by relevant NEPs.
    pub(crate) fn nft_token_compliant_internal(
        &self,
        token_id: u64,
    ) -> Option<TokenCompliant> {
        self.tokens.get(&token_id).map(|x| {
            let metadata = self.nft_token_metadata(U64(x.id));
            let royalty = self.get_token_royalty(U64(x.id));
            let metadata = TokenMetadataCompliant {
                title: metadata.title,
                description: metadata.description,
                media: metadata.media,
                media_hash: metadata.media_hash,
                copies: metadata.copies,
                issued_at: None,
                expires_at: metadata.expires_at,
                starts_at: metadata.starts_at,
                updated_at: None,
                extra: metadata.extra,
                reference: metadata.reference,
                reference_hash: metadata.reference_hash,
            };
            TokenCompliant {
                token_id: format!("{}", x.id),
                owner_id: x.owner_id,
                approved_account_ids: x.approvals,
                metadata,
                royalty,
                split_owners: x.split_owners,
                minter: x.minter,
                loan: x.loan,
                composeable_stats: x.composeable_stats,
                origin_key: x.origin_key,
            }
        })
    }
}

fn log_nft_transfer(
    to: &AccountId,
    token_id: u64,
    memo: &Option<String>,
    old_owner: String,
) {
    let data = NftTransferData(vec![NftTransferLog {
        authorized_id: None,
        old_owner_id: old_owner,
        new_owner_id: to.to_string(),
        token_ids: vec![token_id.to_string()],
        memo: memo.clone(),
    }]);

    env::log_str(data.serialize_event().as_str());
}

fn log_nft_batch_transfer(
    tokens: &[U64],
    accounts: &[AccountId],
    old_owners: Vec<String>,
) {
    let data = NftTransferData(
        accounts
            .iter()
            .enumerate()
            .map(|(u, x)| NftTransferLog {
                authorized_id: None,
                old_owner_id: old_owners[u].clone(),
                new_owner_id: x.to_string(),
                token_ids: vec![tokens[u].0.to_string()],
                memo: None,
            })
            .collect::<Vec<_>>(),
    );

    env::log_str(data.serialize_event().as_str());
}
