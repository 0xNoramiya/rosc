//! The rulebook's host on chain: NEP-141 balances, per-account records,
//! NEP-297 logs and the history rings.
use codeblue_core::history::{LedgerEntry, absorb, code_sheet, ledger_entry};
use codeblue_core::{BurnKind, DeathReason, Event, Host, PayKind, Rhythm};
use near_sdk::serde_json::{Value, json};
use near_sdk::{AccountId, env};
use near_sdk_contract_tools::standard::nep141::{
    FtTransferData, Nep141Burn, Nep141Controller, Nep141Event, Nep141Transfer,
};
use near_sdk_contract_tools::standard::nep297::Event as _;

use crate::Contract;
use crate::store::{self, Shelf};

fn must<T, E: ToString>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|e| env::panic_str(&e.to_string()))
}

pub fn rhythm_name(rhythm: Rhythm) -> &'static str {
    match rhythm {
        Rhythm::Vf => "vf",
        Rhythm::Pea => "pea",
        Rhythm::Asystole => "asystole",
    }
}

fn burn_memo(kind: BurnKind) -> &'static str {
    match kind {
        BurnKind::Oxygen => "oxygen",
        BurnKind::NoFlow => "no_flow",
        BurnKind::Epinephrine => "epinephrine",
        BurnKind::Death => "time_of_death",
        BurnKind::Estate => "estate",
    }
}

/// Four bytes that stand for an account on the monitor trace.
pub fn tag(who: &AccountId) -> u32 {
    let hash = env::sha256_array(who.as_bytes());
    u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]])
}

pub struct ChainHost<'a> {
    contract: &'a mut Contract,
    me: AccountId,
    bucket_blocks: u64,
    shelf: Option<Shelf>,
    /// The newest ledger line, held back so oxygen burns can fold into it.
    last: Option<LedgerEntry<AccountId>>,
    dirty: bool,
}

impl<'a> ChainHost<'a> {
    pub fn new(contract: &'a mut Contract, bucket_blocks: u64) -> Self {
        ChainHost { contract, me: env::current_account_id(), bucket_blocks, shelf: None, last: None, dirty: false }
    }

    fn shelf(&mut self) -> &mut Shelf {
        self.shelf.get_or_insert_with(|| store::shelf().read().unwrap_or_default())
    }

    fn file(&mut self, entry: LedgerEntry<AccountId>) {
        let filed = self.shelf().ledger;
        if self.last.is_none() && filed > 0 {
            self.last = store::ledger_line(filed - 1).read();
        }
        if let Some(last) = &mut self.last {
            if absorb(last, &entry, self.bucket_blocks) {
                self.dirty = true;
                return;
            }
            if self.dirty {
                store::ledger_line(filed - 1).write(last);
            }
        }
        self.last = Some(entry);
        self.shelf().ledger = filed + 1;
        self.dirty = true;
    }

    /// Writes what was held back. Call once, after the rulebook returns.
    pub fn finish(mut self) {
        if !self.dirty {
            return;
        }
        let shelf = self.shelf().clone();
        if let Some(last) = &self.last {
            store::ledger_line(shelf.ledger - 1).write(last);
        }
        store::shelf().write(&shelf);
    }
}

impl Host for ChainHost<'_> {
    type Account = AccountId;

    fn balance_of(&self, who: &AccountId) -> u128 {
        Nep141Controller::balance_of(self.contract, who)
    }

    fn total_supply(&self) -> u128 {
        Nep141Controller::total_supply(self.contract)
    }

    fn burn_from_holder(&mut self, who: &AccountId, amount: u128) {
        must(Nep141Controller::burn(self.contract, &Nep141Burn::new(amount, who.clone()).memo("oxygen")));
    }

    fn holder_to_contract(&mut self, who: &AccountId, amount: u128) {
        must(self.contract.transfer_unchecked(who, &self.me, amount));
        Nep141Event::FtTransfer(vec![FtTransferData {
            old_owner_id: who.clone().into(),
            new_owner_id: self.me.clone().into(),
            amount: amount.into(),
            memo: Some("premium".into()),
        }])
        .emit();
    }

    fn burn_from_contract(&mut self, amount: u128, kind: BurnKind) {
        must(Nep141Controller::burn(self.contract, &Nep141Burn::new(amount, self.me.clone()).memo(burn_memo(kind))));
    }

    fn contract_to_holder(&mut self, who: &AccountId, amount: u128, kind: PayKind) {
        let memo = match kind {
            PayKind::Faucet => "faucet",
            PayKind::Bounty => "bounty",
        };
        // The hook lets the contract's own payouts through: no fee, no beat.
        must(Nep141Controller::transfer(
            self.contract,
            &Nep141Transfer::new(amount, self.me.clone(), who.clone()).memo(memo),
        ));
    }

    fn last_beat_of(&self, who: &AccountId) -> Option<u64> {
        store::account(who).read().and_then(|a| a.last_beat)
    }

    fn set_last_beat_of(&mut self, who: &AccountId, block: u64) {
        let mut slot = store::account(who);
        let mut record = slot.read().unwrap_or_default();
        record.last_beat = Some(block);
        slot.write(&record);
    }

    fn claimable_of(&self, who: &AccountId) -> u128 {
        store::account(who).read().map_or(0, |a| a.claimable)
    }

    fn set_claimable_of(&mut self, who: &AccountId, amount: u128) {
        let mut slot = store::account(who);
        let mut record = slot.read().unwrap_or_default();
        record.claimable = amount;
        slot.write(&record);
    }

    fn faucet_claimed(&self, who: &AccountId) -> bool {
        store::account(who).read().is_some_and(|a| a.faucet_claimed)
    }

    fn set_faucet_claimed(&mut self, who: &AccountId) {
        let mut slot = store::account(who);
        let mut record = slot.read().unwrap_or_default();
        record.faucet_claimed = true;
        slot.write(&record);
    }

    fn tag_of(&self, who: &AccountId) -> u32 {
        tag(who)
    }

    fn emit(&mut self, event: Event<AccountId>) {
        log(&event);
        if let Some(sheet) = code_sheet(&event) {
            let filed = self.shelf().sheets;
            store::sheet(filed).write(sheet);
            self.shelf().sheets = filed + 1;
            self.dirty = true;
        }
        if let Some(entry) = ledger_entry(&event) {
            self.file(entry);
        }
    }
}

/// NEP-297, standard `codeblue`, version `1.0.0`. Burns and payments are
/// already logged as NEP-141 `ft_burn` and `ft_transfer`.
fn log(event: &Event<AccountId>) {
    let (name, data): (&str, Value) = match event {
        Event::Arrest { block, code_no, rhythm } => {
            ("arrest", json!({ "block": block, "code": code_no, "rhythm": rhythm_name(*rhythm) }))
        }
        Event::Round { block, code_no, round, quality_bps, fatigued, perfusion, target } => (
            "round",
            json!({ "block": block, "code": code_no, "round": round, "quality_bps": quality_bps,
                    "fatigued": fatigued, "perfusion": perfusion, "target": target }),
        ),
        Event::Shock { block, round, success, perfusion } => {
            ("shock", json!({ "block": block, "round": round, "success": success, "perfusion": perfusion }))
        }
        Event::Epi { block, round, perfusion } => {
            ("epi", json!({ "block": block, "round": round, "perfusion": perfusion }))
        }
        Event::Rosc { block, sheet } => (
            "rosc",
            json!({ "block": block, "code": sheet.code_no, "downtime_blocks": sheet.downtime_blocks(),
                    "rounds": sheet.rounds.len(), "perfusion": sheet.perfusion, "target": sheet.target,
                    "bounty": sheet.bounty.to_string(),
                    "shares": sheet.shares.iter().map(|(who, share)| json!({ "account": who, "amount": share.0.to_string() })).collect::<Vec<_>>() }),
        ),
        Event::TimeOfDeath { block, reason, deaths, sheet } => (
            "time_of_death",
            json!({ "block": block, "code": sheet.code_no, "deaths": deaths, "reason": match reason {
                DeathReason::NoFlow => "no_flow",
                DeathReason::Pause => "pause",
                DeathReason::RoundsExhausted => "rounds_exhausted",
            } }),
        ),
        Event::Deceased { block, estate } => ("deceased", json!({ "block": block, "estate": estate.to_string() })),
        Event::Rehab { block, scars } => ("rehab", json!({ "block": block, "scars": scars })),
        Event::Burn { .. } | Event::BountyCredited { .. } | Event::Paid { .. } => return,
    };
    env::log_str(&format!(
        "EVENT_JSON:{}",
        json!({ "standard": "codeblue", "version": "1.0.0", "event": name, "data": [data] })
    ));
}

/// A host for view calls. The rulebook only reads through it: `preview` and
/// `monitor` keep every write in a shadow of their own.
pub struct ViewHost<'a>(pub &'a Contract);

impl Host for ViewHost<'_> {
    type Account = AccountId;

    fn balance_of(&self, who: &AccountId) -> u128 {
        Nep141Controller::balance_of(self.0, who)
    }
    fn total_supply(&self) -> u128 {
        Nep141Controller::total_supply(self.0)
    }
    fn last_beat_of(&self, who: &AccountId) -> Option<u64> {
        store::account(who).read().and_then(|a| a.last_beat)
    }
    fn claimable_of(&self, who: &AccountId) -> u128 {
        store::account(who).read().map_or(0, |a| a.claimable)
    }
    fn faucet_claimed(&self, who: &AccountId) -> bool {
        store::account(who).read().is_some_and(|a| a.faucet_claimed)
    }
    fn tag_of(&self, who: &AccountId) -> u32 {
        tag(who)
    }
    fn burn_from_holder(&mut self, _: &AccountId, _: u128) {}
    fn holder_to_contract(&mut self, _: &AccountId, _: u128) {}
    fn burn_from_contract(&mut self, _: u128, _: BurnKind) {}
    fn contract_to_holder(&mut self, _: &AccountId, _: u128, _: PayKind) {}
    fn set_last_beat_of(&mut self, _: &AccountId, _: u64) {}
    fn set_claimable_of(&mut self, _: &AccountId, _: u128) {}
    fn set_faucet_claimed(&mut self, _: &AccountId) {}
    fn emit(&mut self, _: Event<AccountId>) {}
}
