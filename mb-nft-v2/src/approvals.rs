use mb_sdk::{
    assert_storage_deposit,
    assert_token_owned_by_predecessor,
    assert_token_unloaned,
    constants::{
        gas,
        MAX_APPROVALS_PER_TOKEN,
    },
    data::store::Token,
    events::store::{
        NftApproveData,
        NftApproveLog,
        NftRevokeAllData,
        NftRevokeData,
    },
    interfaces::ext_nft_on_approve,
    near_sdk::{
        self,
        assert_one_yocto,
        env,
        json_types::U64,
        near_bindgen,
        AccountId,
        Promise,
        PromiseOrValue,
    },
};

use crate::*;

// --------------------- standardized approval methods ---------------------- //
#[near_bindgen]
impl MintbaseStore {
    // -------------------------- change methods ---------------------------
    /// Granting NFT transfer approval as specified by
    /// [NEP-178](https://nomicon.io/Standards/Tokens/NonFungibleToken/ApprovalManagement)
    #[payable]
    pub fn nft_approve(
        &mut self,
        token_id: String,
        account_id: AccountId,
        msg: Option<String>,
    ) -> Option<Promise> {
        // Note: This method only guarantees that the store-storage is covered.
        // The market may still reject.
        assert_storage_deposit!(self.storage_costs.common);
        let token_id_tuple = parse_token_id(&token_id);
        // validates owner and loaned
        let approval_id = self.approve_internal(token_id_tuple, &account_id);
        log_approve(token_id_tuple, approval_id, &account_id);

        if let Some(msg) = msg {
            ext_nft_on_approve::ext(account_id)
                .with_static_gas(gas::NFT_ON_APPROVE)
                .nft_on_approve(
                    token_id,
                    env::predecessor_account_id(),
                    approval_id,
                    msg,
                )
                .into()
        } else {
            None
        }
    }

    /// Revokes NFT transfer approval as specified by
    /// [NEP-178](https://nomicon.io/Standards/Tokens/NonFungibleToken/ApprovalManagement)
    #[payable]
    pub fn nft_revoke(
        &mut self,
        token_id: String,
        account_id: AccountId,
    ) -> PromiseOrValue<()> {
        let token_id_tuple = parse_token_id(&token_id);
        let mut token = self.nft_token_internal(token_id_tuple);
        assert_token_unloaned!(token);
        assert_token_owned_by_predecessor!(token);
        assert_one_yocto();

        if token.approvals.remove(&account_id).is_some() {
            self.save_token(&token);
            log_revoke(token_id_tuple, &account_id);
            PromiseOrValue::Promise(
                Promise::new(env::predecessor_account_id())
                    .transfer(self.storage_costs.common),
            )
        } else {
            PromiseOrValue::Value(())
        }
    }

    /// Revokes all NFT transfer approvals as specified by
    /// as specified by [NEP-178](https://nomicon.io/Standards/Tokens/NonFungibleToken/ApprovalManagement)
    #[payable]
    pub fn nft_revoke_all(&mut self, token_id: String) -> Promise {
        let token_id_tuple = parse_token_id(&token_id);
        let mut token = self.nft_token_internal(token_id_tuple);
        assert_token_unloaned!(token);
        assert_token_owned_by_predecessor!(token);
        assert_one_yocto();

        let refund = token.approvals.len() as u128 * self.storage_costs.common;

        if !token.approvals.is_empty() {
            token.approvals.clear();
            self.save_token(&token);
            log_revoke_all(token_id_tuple);
        }
        Promise::new(env::predecessor_account_id()).transfer(refund)
    }

    // -------------------------- view methods -----------------------------
    pub fn nft_is_approved(
        &self,
        token_id: String,
        approved_account_id: AccountId,
        approval_id: Option<u64>,
    ) -> bool {
        let token_id_tuple = parse_token_id(&token_id);
        self.nft_is_approved_internal(
            &self.nft_token_internal(token_id_tuple),
            &approved_account_id,
            approval_id,
        )
    }
}

// ------------------- non-standardized approval methods -------------------- //
#[near_bindgen]
impl MintbaseStore {
    // -------------------------- change methods ---------------------------
    /// Like `nft_approve`, but it allows approving multiple tokens in one call.
    /// The `msg` argument will be forwarded towards a `nft_on_batch_approve`.
    /// As this is not standardized and only supported by the legacy Mintbase
    /// market.
    #[payable]
    pub fn nft_batch_approve(
        &mut self,
        token_ids: Vec<String>,
        account_id: AccountId,
        msg: Option<String>,
    ) -> Option<Promise> {
        let tlen = token_ids.len() as u128;
        assert!(tlen > 0);
        assert!(tlen <= 70);
        let storage_stake = self.storage_costs.common * tlen;
        // Note: This method only guarantees that the store-storage is covered.
        // The financial contract may still reject.
        assert_storage_deposit!(storage_stake);
        let approval_ids: Vec<U64> = token_ids
            .iter()
            // validates owner and loaned
            .map(|token_id| {
                let token_id_tuple = parse_token_id(token_id);
                self.approve_internal(token_id_tuple, &account_id).into()
            })
            .collect();
        log_batch_approve(token_ids.clone(), &approval_ids, &account_id);

        if let Some(msg) = msg {
            ext_nft_on_approve::ext(account_id)
                .with_attached_deposit(env::attached_deposit() - storage_stake)
                .with_static_gas(gas::NFT_BATCH_APPROVE)
                .nft_on_batch_approve(
                    token_ids,
                    approval_ids,
                    env::predecessor_account_id(),
                    msg,
                )
                .into()
        } else {
            None
        }
    }

    // -------------------------- view methods -----------------------------
    /// Returns the most recent `approval_id` for `account_id` on `token_id`.
    /// If the account doesn't have approval on the token, it will return
    /// `None`.
    ///
    /// Panics if the token doesn't exist.
    pub fn nft_approval_id(
        &self,
        token_id: String,
        account_id: AccountId,
    ) -> Option<u64> {
        let token_id_tuple = parse_token_id(&token_id);
        let token = self.nft_token_internal(token_id_tuple);
        token.approvals.get(&account_id).cloned()
    }

    // -------------------------- private methods --------------------------
    // -------------------------- internal methods -------------------------

    /// Called from nft_approve and nft_batch_approve.
    fn approve_internal(
        &mut self,
        token_id_tuple: (u64, u64),
        account_id: &AccountId,
    ) -> u64 {
        let mut token = self.nft_token_internal(token_id_tuple);
        // token.assert_unloaned();
        // token.assert_owned_by_predecessor();
        assert_token_unloaned!(token);
        assert_token_owned_by_predecessor!(token);
        near_assert!(
            token.approvals.len() as u64 <= MAX_APPROVALS_PER_TOKEN,
            "Cannot approve more than {} accounts for a token",
            MAX_APPROVALS_PER_TOKEN
        );

        let approval_id = self.num_approved;
        self.num_approved += 1;
        token.approvals.insert(account_id.clone(), approval_id);
        self.save_token(&token);
        approval_id
    }

    /// Same as `nft_is_approved`, but uses internal u64 (u64) typing for
    /// Copy-efficiency.
    pub(crate) fn nft_is_approved_internal(
        &self,
        token: &Token,
        approved_account_id: &AccountId,
        approval_id: Option<u64>,
    ) -> bool {
        if approved_account_id.to_string() == token.owner_id.to_string() {
            true
        } else {
            let approval_id = approval_id.expect("approval_id required");
            let stored_approval = token.approvals.get(approved_account_id);
            match stored_approval {
                None => false,
                Some(&stored_approval_id) => stored_approval_id == approval_id,
            }
        }
    }
}

pub(crate) fn log_approve(
    token_id: (u64, u64),
    approval_id: u64,
    account_id: &AccountId,
) {
    let data = NftApproveData(vec![NftApproveLog {
        token_id: fmt_token_id(token_id),
        approval_id,
        account_id: account_id.to_string(),
    }]);
    env::log_str(&data.serialize_event());
}

fn log_batch_approve(
    token_ids: Vec<String>,
    approval_ids: &[U64],
    account_id: &AccountId,
) {
    let data = NftApproveData(
        approval_ids
            .iter()
            .zip(token_ids)
            .map(|(approval_id, token_id)| NftApproveLog {
                token_id,
                approval_id: approval_id.0,
                account_id: account_id.to_string(),
            })
            .collect::<Vec<_>>(),
    );
    env::log_str(&data.serialize_event());
}

fn log_revoke(token_id: (u64, u64), account_id: &AccountId) {
    env::log_str(
        &NftRevokeData {
            token_id: fmt_token_id(token_id),
            account_id: account_id.to_string(),
        }
        .serialize_event(),
    );
}

fn log_revoke_all(token_id: (u64, u64)) {
    env::log_str(
        &NftRevokeAllData {
            token_id: fmt_token_id(token_id),
        }
        .serialize_event(),
    );
}
