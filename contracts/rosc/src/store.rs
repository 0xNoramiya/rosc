//! Where everything lives in contract storage.
//!
//! The root struct is empty. The rulebook's state sits in one slot that only
//! the calls that need it load; per-account records and the history rings
//! sit under their own keys, so a compression never pays to read the ledger.
use codeblue_core::history::LedgerEntry;
use codeblue_core::{CodeSheet, State};
use near_sdk::{AccountId, AccountIdRef, NearToken, env, near};
use near_sdk_contract_tools::slot::Slot;

pub type Rules = State<AccountId>;

/// Lines kept in the ledger ring, and code sheets kept beside it.
pub const LEDGER_RING: u32 = 256;
pub const SHEET_RING: u32 = 16;

/// Bytes one account can occupy, for the longest account id (64 characters):
/// the NEP-141 balance (66 + 64) and the record below (67 + 64 once it holds
/// a last beat), 261 in all, with room to spare. Checked in `tests/sandbox.rs`.
pub const STORAGE_BYTES_PER_ACCOUNT: u128 = 280;

/// What the coin remembers about an account besides its balance.
/// The record's existence is the account's NEP-145 registration.
#[near(serializers = [borsh])]
#[derive(Default, Clone)]
pub struct Account {
    /// Block of this account's last beat. Decides the refractory period.
    pub last_beat: Option<u64>,
    /// Bounty credited and not yet claimed.
    pub claimable: u128,
    pub faucet_claimed: bool,
}

/// How many lines and sheets have ever been filed. The rings keep the newest.
#[near(serializers = [borsh])]
#[derive(Default, Clone)]
pub struct Shelf {
    pub ledger: u32,
    pub sheets: u32,
}

pub fn rules() -> Slot<Rules> {
    Slot::new(b"s".to_vec())
}

pub fn account(id: &AccountIdRef) -> Slot<Account> {
    Slot::new([b"a".as_slice(), id.as_bytes()].concat())
}

pub fn shelf() -> Slot<Shelf> {
    Slot::new(b"h".to_vec())
}

pub fn ledger_line(seq: u32) -> Slot<LedgerEntry<AccountId>> {
    Slot::new([b"l".as_slice(), &((seq % LEDGER_RING) as u16).to_le_bytes()].concat())
}

pub fn sheet(seq: u32) -> Slot<CodeSheet<AccountId>> {
    Slot::new(vec![b'c', (seq % SHEET_RING) as u8])
}

/// The one-time NEAR deposit that registers an account.
pub fn storage_fee() -> NearToken {
    env::storage_byte_cost().saturating_mul(STORAGE_BYTES_PER_ACCOUNT)
}
