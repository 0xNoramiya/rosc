//! A test double: the pool or bridge on the other end of `ft_transfer_call`.
//! `msg` says how much of the transfer it reports unused.
use near_sdk::json_types::U128;
use near_sdk::{AccountId, env, near};

#[derive(Default)]
#[near(contract_state)]
pub struct Receiver {}

#[near]
impl Receiver {
    pub fn ft_on_transfer(&mut self, sender_id: AccountId, amount: U128, msg: String) -> U128 {
        let _ = sender_id;
        match msg.as_str() {
            "keep" => U128(0),
            "half" => U128(amount.0 / 2),
            "all" => amount,
            _ => env::panic_str("receiver panics"),
        }
    }
}
