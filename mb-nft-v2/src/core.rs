use std::{
    collections::HashMap,
    convert::TryFrom,
};

use mb_sdk::{
    assert_token_owned_by,
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
    near_panic,
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
        token_id: String,
        approval_id: Option<u64>,
        memo: Option<String>,
    ) {
        assert_one_yocto();
        let token_id_tuple = parse_token_id(&token_id);
        let mut token = self.nft_token_internal(token_id_tuple);
        let old_owner = token.owner_id.to_string();
        assert_token_unloaned!(token);
        let authorized_id = assert_token_owned_or_approved(
            &token,
            &env::predecessor_account_id(),
            approval_id,
        );

        self.transfer_internal(&mut token, receiver_id.clone(), true);
        log_nft_transfer(
            &receiver_id,
            token_id_tuple,
            &memo,
            old_owner,
            authorized_id,
        );
    }

    /// Transfer-and-call function as specified by [NEP-171](https://nomicon.io/Standards/Tokens/NonFungibleToken/Core).
    #[payable]
    pub fn nft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        token_id: String,
        msg: String,
        approval_id: Option<u64>,
        memo: Option<String>,
    ) -> Promise {
        assert_one_yocto();
        let token_id_tuple = parse_token_id(&token_id);
        let mut token = self.nft_token_internal(token_id_tuple);
        let pred = env::predecessor_account_id();
        assert_token_unloaned!(token);
        let authorized_id = assert_token_owned_or_approved(
            &token,
            &env::predecessor_account_id(),
            approval_id,
        );

        let previous_owner_id =
            AccountId::new_unchecked(token.owner_id.to_string());
        let approved_account_ids = token.approvals.clone();
        let split_owners = token.split_owners.clone();
        // prevent race condition, temporarily lock-replace owner
        self.transfer_internal(&mut token, receiver_id.clone(), true);
        log_nft_transfer(
            &receiver_id,
            token.id_tuple(),
            &memo,
            previous_owner_id.to_string(),
            authorized_id,
        );
        self.lock_token(&mut token);

        ext_nft_on_transfer::ext(receiver_id.clone())
            .with_static_gas(gas::NFT_TRANSFER_CALL)
            .nft_on_transfer(
                pred,
                previous_owner_id.clone(),
                token_id.clone(),
                msg,
            )
            .then(
                store_self::ext(env::current_account_id())
                    .with_static_gas(gas::NFT_TRANSFER_CALL)
                    .nft_resolve_transfer(
                        previous_owner_id,
                        receiver_id,
                        token_id,
                        approved_account_ids,
                        split_owners,
                    ),
            )
    }

    // -------------------------- view methods -----------------------------

    /// Token view method as specified by [NEP-171](https://nomicon.io/Standards/Tokens/NonFungibleToken/Core).
    pub fn nft_token(&self, token_id: String) -> Option<TokenCompliant> {
        self.nft_token_compliant_internal(&parse_token_id(&token_id))
    }

    // -------------------------- private methods --------------------------

    /// Call back of a transfer-and-call as specified by [NEP-171](https://nomicon.io/Standards/Tokens/NonFungibleToken/Core).
    #[private]
    pub fn nft_resolve_transfer(
        &mut self,
        previous_owner_id: AccountId,
        receiver_id: AccountId,
        token_id: String,
        // NOTE: might borsh::maybestd::collections::HashMap be more appropriate?
        approved_account_ids: HashMap<AccountId, u64>,
        split_owners: Option<SplitOwners>,
    ) -> bool {
        let l = format!(
            "previous_owner_id={} receiver_id={} token_id={} approved_account_ids={:?} split_owners={:?} pred={}",
            previous_owner_id,
            receiver_id,
            token_id,
            approved_account_ids,
            split_owners,
            env::predecessor_account_id()
        );
        env::log_str(l.as_str());
        let token_id_tuple = parse_token_id(&token_id);
        let mut token = self.nft_token_internal(token_id_tuple);
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
            self.transfer_internal(&mut token, previous_owner_id.clone(), true);
            log_nft_transfer(
                &previous_owner_id,
                token_id_tuple,
                &None,
                receiver_id.to_string(),
                None,
            );
            // restore approvals
            token.approvals = approved_account_ids;
            for (account_id, &approval_id) in token.approvals.iter() {
                crate::approvals::log_approve(
                    token.id_tuple(),
                    approval_id,
                    account_id,
                );
            }
            // restore split owners
            token.split_owners = split_owners;
            if let Some(split_owners) = token.split_owners.as_ref() {
                crate::payout::log_set_split_owners(
                    vec![token.fmt_id()],
                    // clone needed because this is drained in the logging function
                    split_owners.clone(),
                );
            }
            false
        }
    }

    /// Locking an NFT during a transfer-and-call chain
    fn lock_token(&mut self, token: &mut Token) {
        if let Owner::Account(ref s) = token.owner_id {
            token.owner_id = Owner::Lock(s.clone());
            self.tokens.insert(&token.id_tuple(), token);
        }
    }

    /// Unlocking an NFT after a transfer-and-call chain
    fn unlock_token(&mut self, token: &mut Token) {
        if let Owner::Lock(ref s) = token.owner_id {
            token.owner_id = Owner::Account(s.clone());
            self.tokens.insert(&token.id_tuple(), token);
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
    pub fn nft_batch_transfer(&mut self, token_ids: Vec<(String, AccountId)>) {
        assert_one_yocto();
        near_assert!(!token_ids.is_empty(), "Token IDs cannot be empty");
        let pred = env::predecessor_account_id();
        let mut set_owned =
            self.tokens_per_owner.get(&pred).expect("none owned");
        let (tokens, accounts, old_owners) = token_ids
            .into_iter()
            .map(|(token_id, account_id)| {
                let token_id_tuple = parse_token_id(&token_id);
                let mut token = self.nft_token_internal(token_id_tuple);
                let old_owner = token.owner_id.to_string();
                assert_token_unloaned!(token);
                assert_token_owned_by!(token, &pred);
                near_assert!(
                    account_id.to_string() != token.owner_id.to_string(),
                    "Token {}:{} is already owned by {}",
                    token.metadata_id,
                    token.id,
                    account_id
                ); // can't transfer to self
                self.transfer_internal(&mut token, account_id.clone(), false);
                set_owned.remove(&token_id_tuple);
                (token_id, account_id, old_owner)
            })
            .fold((vec![], vec![], vec![]), |mut acc, (tid, aid, oid)| {
                acc.0.push(tid);
                acc.1.push(aid);
                acc.2.push(oid);
                acc
            });
        self.tokens_per_owner.insert(&pred, &set_owned);
        log_nft_batch_transfer(tokens, &accounts, old_owners);
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
        self.update_tokens_per_owner(
            token.id_tuple(),
            update_set,
            Some(to.clone()),
        );
        token.owner_id = Owner::Account(to);
        token.approvals.clear();
        self.tokens.insert(&token.id_tuple(), token);
    }

    /// Gets the token as stored on the smart contract
    pub(crate) fn nft_token_internal(&self, token_id: (u64, u64)) -> Token {
        self.tokens.get(&token_id).unwrap_or_else(|| {
            panic!("token: {}:{} doesn't exist", token_id.0, token_id.1)
        })
    }

    /// Gets the token as specified by relevant NEPs.
    pub(crate) fn nft_token_compliant_internal(
        &self,
        token_id: &(u64, u64),
    ) -> Option<TokenCompliant> {
        self.tokens.get(token_id).map(|x| {
            let token_id_string = fmt_token_id(*token_id);
            let metadata = self.nft_token_metadata(token_id_string.clone());
            let royalty = self.get_token_royalty(token_id_string);
            TokenCompliant {
                token_id: format!("{}:{}", x.metadata_id, x.id),
                owner_id: x.owner_id,
                approved_account_ids: x.approvals,
                metadata: metadata.into(),
                royalty,
                split_owners: x.split_owners,
                minter: x.minter,
                loan: x.loan,
                composable_stats: x.composable_stats,
                origin_key: x.origin_key,
            }
        })
    }
}

/// Checks if `account_id` is allowed to transfer the token and returns the
/// `authorized_id` to log. Explicitly, returns `None` if token is owned by
/// `account_id`, returns `Some(account_id)` if `account_id` was approved
/// with the correct `approval_id`, panics otherwise.
fn assert_token_owned_or_approved(
    token: &Token,
    account_id: &AccountId,
    approval_id: Option<u64>,
) -> Option<String> {
    if token.is_owned_by(account_id) {
        return None;
    }

    match (token.approvals.get(account_id), approval_id) {
        // approval ID needs to exist
        (_, None) => near_panic!("Disallowing approvals without approval ID"),
        // account_id needs to be approved
        (None, _) => {
            near_panic!(
                "{} has no approval for token {}:{}",
                account_id,
                token.metadata_id,
                token.id
            )
        }
        // approval IDs need to match
        (Some(a), Some(b)) if *a != b => near_panic!(
            "The current approval ID is {}, but {} has been provided",
            a,
            b
        ),
        _ => Some(account_id.to_string()),
    }
}

fn log_nft_transfer(
    to: &AccountId,
    token_id: (u64, u64),
    memo: &Option<String>,
    old_owner_id: String,
    authorized_id: Option<String>,
) {
    let data = NftTransferData(vec![NftTransferLog {
        authorized_id,
        old_owner_id,
        new_owner_id: to.to_string(),
        token_ids: vec![fmt_token_id(token_id)],
        memo: memo.clone(),
    }]);

    env::log_str(data.serialize_event().as_str());
}

fn log_nft_batch_transfer(
    token_ids: Vec<String>,
    accounts: &[AccountId],
    old_owners: Vec<String>,
) {
    let data = NftTransferData(
        accounts
            .iter()
            .zip(token_ids)
            .enumerate()
            .map(|(u, (account_id, token_id))| NftTransferLog {
                authorized_id: None,
                old_owner_id: old_owners[u].clone(),
                new_owner_id: account_id.to_string(),
                token_ids: vec![token_id],
                memo: None,
            })
            .collect::<Vec<_>>(),
    );

    env::log_str(data.serialize_event().as_str());
}
