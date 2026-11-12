//! NEP-145 storage management: one fixed deposit registers an account.
use near_sdk::json_types::U128;
use near_sdk::{AccountId, NearToken, assert_one_yocto, env, near, require};
use near_sdk_contract_tools::standard::nep141::{Nep141Controller, Nep141ControllerInternal};

use crate::store::{self, Account};
use crate::{Contract, ContractExt};

#[near(serializers = [json])]
pub struct StorageBalance {
    pub total: NearToken,
    pub available: NearToken,
}

#[near(serializers = [json])]
pub struct StorageBalanceBounds {
    pub min: NearToken,
    pub max: Option<NearToken>,
}

const NONE: NearToken = NearToken::from_yoctonear(0);

/// Sends NEAR back with a plain transfer action.
fn refund(to: &AccountId, amount: NearToken) {
    if amount > NONE {
        let batch = env::promise_batch_create(to);
        env::promise_batch_action_transfer(batch, amount);
    }
}

fn balance() -> StorageBalance {
    StorageBalance { total: store::storage_fee(), available: NONE }
}

#[near]
impl Contract {
    /// Registers `account_id`, or the caller. Anything above the fee comes
    /// back, and so does the whole deposit if the account is already registered.
    #[payable]
    pub fn storage_deposit(&mut self, account_id: Option<AccountId>, registration_only: Option<bool>) -> StorageBalance {
        let _ = registration_only;
        let payer = env::predecessor_account_id();
        let deposit = env::attached_deposit();
        let account_id = account_id.unwrap_or_else(|| payer.clone());
        let mut slot = store::account(&account_id);
        if slot.exists() {
            refund(&payer, deposit);
        } else {
            let fee = store::storage_fee();
            require!(
                deposit >= fee,
                format!("The attached deposit is less than the minimum storage balance of {fee}")
            );
            slot.write(&Account::default());
            refund(&payer, deposit.saturating_sub(fee));
        }
        balance()
    }

    /// The fee is fixed and fully used, so there is never anything to withdraw.
    #[payable]
    pub fn storage_withdraw(&mut self, amount: Option<U128>) -> StorageBalance {
        assert_one_yocto();
        let who = env::predecessor_account_id();
        require!(store::account(&who).exists(), format!("The account {who} is not registered"));
        require!(
            amount.is_none_or(|a| a.0 == 0),
            "The amount is greater than the available storage balance"
        );
        balance()
    }

    /// Closes an empty account and returns its storage fee. An account that
    /// still holds tokens or an unclaimed bounty, or that took the faucet,
    /// cannot be closed, with or without `force`.
    #[payable]
    pub fn storage_unregister(&mut self, force: Option<bool>) -> bool {
        let _ = force;
        assert_one_yocto();
        let who = env::predecessor_account_id();
        let mut slot = store::account(&who);
        let Some(record) = slot.read() else { return false };
        require!(
            Nep141Controller::balance_of(self, &who) == 0 && record.claimable == 0,
            "Can't unregister an account that holds tokens or an unclaimed bounty"
        );
        // Otherwise closing and reopening an account would reset its faucet claim.
        require!(!record.faucet_claimed, "An account that took the faucet stays registered");
        slot.remove();
        Self::slot_account(&who).remove();
        refund(&who, store::storage_fee().saturating_add(NearToken::from_yoctonear(1)));
        true
    }

    pub fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        let fee = store::storage_fee();
        StorageBalanceBounds { min: fee, max: Some(fee) }
    }

    pub fn storage_balance_of(&self, account_id: AccountId) -> Option<StorageBalance> {
        store::account(&account_id).exists().then(balance)
    }
}
