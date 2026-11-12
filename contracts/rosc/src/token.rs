//! NEP-141 and NEP-148, with the pulse in the transfer hook.
//!
//! Balances, supply and events come from near-sdk-contract-tools. The
//! external methods are written out here instead of derived, and every
//! cross-contract call uses the low-level promise functions. That keeps
//! `near_sdk::Promise` out of the wasm, and with it the host functions that
//! deploy code, add keys or delete the account.
use near_sdk::json_types::U128;
use near_sdk::serde_json::{self, json};
use near_sdk::{AccountId, Gas, NearToken, assert_one_yocto, env, near, require};
use near_sdk_contract_tools::hook::Hook;
use near_sdk_contract_tools::standard::nep141::{
    Nep141Burn, Nep141Controller, Nep141ControllerInternal, Nep141Transfer,
};
use near_sdk_contract_tools::standard::nep148::ContractMetadata;

use crate::{Contract, ContractExt, store};

const GAS_FOR_RESOLVE_TRANSFER: Gas = Gas::from_tgas(8);
const GAS_FOR_FT_TRANSFER_CALL: Gas = Gas::from_tgas(30);

const ICON: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 64 64'%3E%3Ccircle cx='32' cy='32' r='30' fill='%23f2c14e'/%3E%3Cpath d='M8 34h12l5-12 8 24 6-16 4 4h13' fill='none' stroke='%23201a12' stroke-width='4' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E";

impl Nep141ControllerInternal for Contract {
    type MintHook = ();
    type TransferHook = PulseHook;
    type BurnHook = ();
}

fn must<T, E: ToString>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|e| env::panic_str(&e.to_string()))
}

/// Runs before every token transfer.
pub struct PulseHook;

impl Hook<Contract, Nep141Transfer<'_>> for PulseHook {
    fn hook<R>(contract: &mut Contract, t: &Nep141Transfer<'_>, f: impl FnOnce(&mut Contract) -> R) -> R {
        let me = env::current_account_id();
        // A transfer to the contract's own account is a donation to the crash cart.
        let to_cart = t.receiver_id.as_str() == me.as_str();
        require!(
            to_cart || store::account(&t.receiver_id).exists(),
            format!("The account {} is not registered", t.receiver_id)
        );
        // The refund leg of `ft_transfer_call` and the contract's own payouts
        // are not circulation: no freeze, no fee, no beat.
        if t.revert || t.sender_id.as_str() == me.as_str() {
            return f(contract);
        }
        // contract-tools would credit a self-transfer without debiting it.
        require!(t.sender_id != t.receiver_id, "Sender and receiver must differ");

        let (sender, receiver) = (t.sender_id.clone().into_owned(), t.receiver_id.clone().into_owned());
        contract.with_rules(|rules, host| {
            rules.on_transfer(host, env::block_height(), &sender, &receiver, t.amount, to_cart)
        });
        f(contract)
    }
}

#[near]
impl Contract {
    #[payable]
    pub fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>) {
        assert_one_yocto();
        require!(amount.0 > 0, "The amount should be a positive number");
        let mut transfer = Nep141Transfer::new(amount.0, env::predecessor_account_id(), receiver_id);
        transfer.memo = memo.map(Into::into);
        must(Nep141Controller::transfer(self, &transfer));
    }

    /// Transfers, then calls `ft_on_transfer` on the receiver and refunds
    /// what it reports unused. The fees are charged once, on the way out,
    /// and are not refunded.
    #[payable]
    pub fn ft_transfer_call(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>, msg: String) {
        assert_one_yocto();
        require!(amount.0 > 0, "The amount should be a positive number");
        require!(
            receiver_id != env::current_account_id(),
            "Use ft_transfer to give to the crash cart"
        );
        let prepaid = env::prepaid_gas();
        require!(prepaid >= GAS_FOR_FT_TRANSFER_CALL, "Insufficient gas attached.");
        let sender_id = env::predecessor_account_id();
        let mut transfer = Nep141Transfer::new(amount.0, sender_id.clone(), receiver_id.clone());
        transfer.memo = memo.map(Into::into);
        transfer.msg = Some(msg.as_str().into());
        must(Nep141Controller::transfer(self, &transfer));

        let on_transfer = env::promise_create(
            receiver_id.clone(),
            "ft_on_transfer",
            json!({ "sender_id": sender_id, "amount": amount, "msg": msg }).to_string(),
            NearToken::from_yoctonear(0),
            prepaid.saturating_sub(GAS_FOR_FT_TRANSFER_CALL),
        );
        let resolve = env::promise_then(
            on_transfer,
            env::current_account_id(),
            "ft_resolve_transfer",
            json!({ "sender_id": sender_id, "receiver_id": receiver_id, "amount": amount }).to_string(),
            NearToken::from_yoctonear(0),
            GAS_FOR_RESOLVE_TRANSFER,
        );
        env::promise_return(resolve);
    }

    /// Returns the amount the receiver kept.
    #[private]
    pub fn ft_resolve_transfer(&mut self, sender_id: AccountId, receiver_id: AccountId, amount: U128) -> U128 {
        let amount = amount.0;
        let unused = match env::promise_result_checked(0, 64) {
            Ok(value) => serde_json::from_slice::<U128>(&value).map_or(amount, |unused| unused.0.min(amount)),
            Err(_) => amount,
        };
        let refund = unused.min(Nep141Controller::balance_of(self, &receiver_id));
        if refund == 0 {
            return U128(amount);
        }
        if store::account(&sender_id).exists() {
            let mut transfer = Nep141Transfer::new(refund, receiver_id, sender_id);
            transfer.revert = true;
            must(Nep141Controller::transfer(self, &transfer));
        } else {
            // The sender closed their account while the call was in flight.
            must(Nep141Controller::burn(self, &Nep141Burn::new(refund, receiver_id).memo("refund to a closed account")));
        }
        U128(amount - refund)
    }

    pub fn ft_total_supply(&self) -> U128 {
        Nep141Controller::total_supply(self).into()
    }

    pub fn ft_balance_of(&self, account_id: AccountId) -> U128 {
        Nep141Controller::balance_of(self, &account_id).into()
    }

    pub fn ft_metadata(&self) -> ContractMetadata {
        ContractMetadata::new("Code Blue", "ROSC", 18).icon(ICON)
    }
}
