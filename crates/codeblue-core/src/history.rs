//! The chart: a ring of the last burns and payments, and the last code
//! sheets. Display history only. The contract keeps it apart from the state
//! the rules read, so a compression does not pay to load it.
use crate::events::{BurnKind, CodeSheet, Event, PayKind};
use crate::ring::Ring;

pub const LEDGER_RING: u16 = 256;
pub const SHEET_RING: u16 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LedgerKind {
    Burn(BurnKind),
    /// A bounty share credited to a rescuer.
    Bounty,
    Paid(PayKind),
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LedgerEntry<A> {
    pub block: u64,
    pub kind: LedgerKind,
    pub account: Option<A>,
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub amount: u128,
    /// How many flows this line adds up. Oxygen burns are summed per bucket.
    pub count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct History<A> {
    pub ledger: Ring<LedgerEntry<A>>,
    pub sheets: Ring<CodeSheet<A>>,
}

impl<A: Clone> Default for History<A> {
    fn default() -> Self {
        History { ledger: Ring::new(LEDGER_RING), sheets: Ring::new(SHEET_RING) }
    }
}

/// The ledger line an event leaves, if it leaves one.
pub fn ledger_entry<A: Clone>(event: &Event<A>) -> Option<LedgerEntry<A>> {
    let (block, kind, account, amount) = match event {
        Event::Burn { block, kind, amount } => (*block, LedgerKind::Burn(*kind), None, *amount),
        Event::BountyCredited { block, account, amount } => (*block, LedgerKind::Bounty, Some(account.clone()), *amount),
        Event::Paid { block, kind, account, amount } => (*block, LedgerKind::Paid(*kind), Some(account.clone()), *amount),
        _ => return None,
    };
    Some(LedgerEntry { block, kind, account, amount, count: 1 })
}

/// Every transfer burns oxygen, so those burns are kept as one line per
/// monitor bucket and a busy hour cannot flush the ring. Folds `next` into
/// `last` when both are oxygen burns in the same bucket.
pub fn absorb<A>(last: &mut LedgerEntry<A>, next: &LedgerEntry<A>, bucket_blocks: u64) -> bool {
    let oxygen = LedgerKind::Burn(BurnKind::Oxygen);
    if last.kind != oxygen || next.kind != oxygen || last.block / bucket_blocks != next.block / bucket_blocks {
        return false;
    }
    last.amount += next.amount;
    last.count += next.count;
    last.block = next.block;
    true
}

/// The code sheet an event closes, if it closes one.
pub fn code_sheet<A>(event: &Event<A>) -> Option<&CodeSheet<A>> {
    match event {
        Event::Rosc { sheet, .. } | Event::TimeOfDeath { sheet, .. } => Some(sheet),
        _ => None,
    }
}

impl<A: Clone> History<A> {
    /// Files one event. `bucket_blocks` is the monitor bucket width.
    pub fn record(&mut self, event: &Event<A>, bucket_blocks: u64) {
        if let Some(sheet) = code_sheet(event) {
            self.sheets.push(sheet.clone());
        }
        let Some(entry) = ledger_entry(event) else { return };
        if let Some(last) = self.ledger.last_mut() {
            if absorb(last, &entry, bucket_blocks) {
                return;
            }
        }
        self.ledger.push(entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::PayKind;

    #[test]
    fn oxygen_burns_share_a_line_per_bucket() {
        let mut h: History<u32> = History::default();
        let burn = |block, amount| Event::Burn { block, kind: BurnKind::Oxygen, amount };
        h.record(&burn(100, 5), 10);
        h.record(&burn(104, 7), 10);
        h.record(&burn(111, 1), 10);
        h.record(&Event::Paid { block: 112, kind: PayKind::Faucet, account: 3, amount: 9 }, 10);
        h.record(&burn(113, 2), 10);
        let lines: Vec<(u64, u128, u32)> = h.ledger.iter().map(|e| (e.block, e.amount, e.count)).collect();
        assert_eq!(lines, vec![(104, 12, 2), (111, 1, 1), (112, 9, 1), (113, 2, 1)]);
    }
}
