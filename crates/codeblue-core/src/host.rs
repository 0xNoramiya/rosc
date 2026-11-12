//! What the rulebook needs from whoever runs it. The contract backs this
//! with NEP-141 storage; the simulator and the tests with plain memory.
//!
//! The host is dumb storage. Every amount and every decision comes from the
//! rulebook, so two hosts running the same calls cannot disagree.
use crate::events::{BurnKind, Event, PayKind};

pub trait Host {
    type Account: Clone + PartialEq;

    // ── Token ledger ───────────────────────────────────────────────────
    fn balance_of(&self, who: &Self::Account) -> u128;
    fn total_supply(&self) -> u128;
    /// Burn from a holder. Lowers total supply.
    fn burn_from_holder(&mut self, who: &Self::Account, amount: u128);
    /// Move from a holder into the contract's own balance.
    fn holder_to_contract(&mut self, who: &Self::Account, amount: u128);
    /// Burn from the contract's own balance. Lowers total supply.
    fn burn_from_contract(&mut self, amount: u128, kind: BurnKind);
    /// Pay a holder from the contract's own balance.
    fn contract_to_holder(&mut self, who: &Self::Account, amount: u128, kind: PayKind);

    // ── Per-account records ────────────────────────────────────────────
    fn last_beat_of(&self, who: &Self::Account) -> Option<u64>;
    fn set_last_beat_of(&mut self, who: &Self::Account, block: u64);
    fn claimable_of(&self, who: &Self::Account) -> u128;
    fn set_claimable_of(&mut self, who: &Self::Account, amount: u128);
    fn faucet_claimed(&self, who: &Self::Account) -> bool;
    fn set_faucet_claimed(&mut self, who: &Self::Account);
    /// Four bytes that stand for the account on the monitor trace.
    fn tag_of(&self, who: &Self::Account) -> u32;

    // ── Log ────────────────────────────────────────────────────────────
    fn emit(&mut self, event: Event<Self::Account>);
}
