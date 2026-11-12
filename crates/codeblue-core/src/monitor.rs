//! The derived state: what the coin is at a given block, whether or not a
//! call has written it yet. Everything the page draws comes from here.
use crate::events::{BurnKind, Event, PayKind, Rhythm};
use crate::host::Host;
use crate::state::{Beat, Bucket, Phase, Pools, State, Status, Totals};
use crate::Config;

/// A host that reads through to the real one and keeps every write to itself.
struct Shadow<'a, H: Host> {
    inner: &'a H,
    burned: u128,
    claimable: Vec<(H::Account, u128)>,
}

impl<H: Host> Host for Shadow<'_, H> {
    type Account = H::Account;

    fn balance_of(&self, who: &H::Account) -> u128 {
        self.inner.balance_of(who)
    }
    fn total_supply(&self) -> u128 {
        self.inner.total_supply() - self.burned
    }
    fn burn_from_holder(&mut self, _: &H::Account, amount: u128) {
        self.burned += amount;
    }
    fn holder_to_contract(&mut self, _: &H::Account, _: u128) {}
    fn burn_from_contract(&mut self, amount: u128, _: BurnKind) {
        self.burned += amount;
    }
    fn contract_to_holder(&mut self, _: &H::Account, _: u128, _: PayKind) {}
    fn last_beat_of(&self, who: &H::Account) -> Option<u64> {
        self.inner.last_beat_of(who)
    }
    fn set_last_beat_of(&mut self, _: &H::Account, _: u64) {}
    fn claimable_of(&self, who: &H::Account) -> u128 {
        match self.claimable.iter().find(|(a, _)| a == who) {
            Some((_, amount)) => *amount,
            None => self.inner.claimable_of(who),
        }
    }
    fn set_claimable_of(&mut self, who: &H::Account, amount: u128) {
        match self.claimable.iter_mut().find(|(a, _)| a == who) {
            Some(entry) => entry.1 = amount,
            None => self.claimable.push((who.clone(), amount)),
        }
    }
    fn faucet_claimed(&self, who: &H::Account) -> bool {
        self.inner.faucet_claimed(who)
    }
    fn set_faucet_claimed(&mut self, _: &H::Account) {}
    fn tag_of(&self, who: &H::Account) -> u32 {
        self.inner.tag_of(who)
    }
    fn emit(&mut self, _: Event<H::Account>) {}
}

/// The state as of `block`, with the supply and claimable balances that the
/// next successful call would write.
#[derive(Clone, Debug)]
pub struct Preview<A> {
    pub block: u64,
    pub state: State<A>,
    pub supply: u128,
    /// Claimable balances that differ from what is stored.
    pub claimable: Vec<(A, u128)>,
}

impl<A: Clone + PartialEq> State<A> {
    /// The derived state at `now`. Writes nothing.
    pub fn preview<H: Host<Account = A>>(&self, host: &H, now: u64) -> Preview<A> {
        let mut state = self.clone();
        let mut shadow = Shadow { inner: host, burned: 0, claimable: Vec::new() };
        state.advance(&mut shadow, now);
        Preview { block: now, supply: shadow.total_supply(), claimable: shadow.claimable, state }
    }

    /// What `who` could claim at `now`, including a bounty not yet written.
    pub fn claimable_at<H: Host<Account = A>>(&self, host: &H, now: u64, who: &A) -> u128 {
        let preview = self.preview(host, now);
        match preview.claimable.iter().find(|(a, _)| a == who) {
            Some((_, amount)) => *amount,
            None => host.claimable_of(who),
        }
    }

    /// Everything the page draws, in one value.
    pub fn monitor<H: Host<Account = A>>(&self, host: &H, now: u64) -> Monitor<A> {
        let Preview { state, supply, .. } = self.preview(host, now);
        let ef = state.ejection_fraction();
        let c = &state.config;
        let (status, code) = match &state.status {
            Status::Alive => (StatusView::Alive { arrest_block: state.arrest_due_block() }, None),
            Status::Morgue { since_block, until_block } => {
                (StatusView::Morgue { since_block: *since_block, until_block: *until_block }, None)
            }
            Status::Deceased { block } => (StatusView::Deceased { block: *block }, None),
            Status::Arrest(code) => {
                let (deadline_block, shock_open, epi_open) = match (code.first_compression_block, code.phase) {
                    (None, _) => (code.arrest_block + c.no_flow_max_blocks, false, false),
                    (Some(_), Phase::Compressions) => (code.last_compression_block + c.max_pause_blocks, false, false),
                    (Some(_), Phase::Ventilation { opened }) => (opened + c.vent_window_blocks, false, false),
                    (Some(_), Phase::AfterRound { opened, shock_open, epi_open }) => {
                        (opened + c.vent_window_blocks, shock_open, epi_open)
                    }
                };
                // The ETCO2 number: the quality of the cycle so far, else of the last round.
                let etco2_bps = if code.cycle_compressions > 0 {
                    let span = now.max(code.last_compression_block) - code.cycle_first_block + 1;
                    (code.cycle_compressions as u64 * crate::BPS / span).min(crate::BPS) as u16
                } else {
                    code.cycle_quality_bps.last().copied().unwrap_or(code.round_quality_bps)
                };
                let view = CodeView {
                    code_no: code.code_no,
                    arrest_block: code.arrest_block,
                    rhythm: code.rhythm,
                    called: code.called,
                    first_compression_block: code.first_compression_block,
                    last_compression_block: code.last_compression_block,
                    round: code.round,
                    cycle: code.cycle,
                    phase: code.phase,
                    deadline_block,
                    shock_open,
                    epi_open,
                    cycle_compressions: code.cycle_compressions,
                    cycle_breaths: code.cycle_breaths,
                    cycle_lead: code.cycle_lead.clone(),
                    perfusion: code.perfusion,
                    target: if code.first_compression_block.is_some() {
                        code.target
                    } else {
                        state.target_for(now - code.arrest_block)
                    },
                    etco2_bps,
                    units: code.units.clone(),
                };
                (StatusView::Arrest, Some(view))
            }
        };
        Monitor {
            block: now,
            status,
            code,
            last_beat_block: state.vitals.last_beat_block,
            alive_since_block: state.vitals.alive_since_block,
            scars: state.vitals.scars,
            deaths: state.vitals.deaths,
            max_deaths: c.max_deaths,
            ejection_fraction: ef,
            beats: state.vitals.beats.iter().copied().collect(),
            buckets: {
                let mut buckets: Vec<Bucket> =
                    state.vitals.buckets.iter().filter(|b| b.index != u64::MAX).copied().collect();
                buckets.sort_by_key(|b| b.index);
                buckets
            },
            pools: state.pools.clone(),
            supply,
            totals: state.totals.clone(),
            config: state.config,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StatusView {
    /// Alive, and in arrest from `arrest_block` if no beat lands first.
    Alive { arrest_block: u64 },
    Arrest,
    Morgue { since_block: u64, until_block: u64 },
    Deceased { block: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CodeView<A> {
    pub code_no: u32,
    pub arrest_block: u64,
    pub rhythm: Rhythm,
    pub called: bool,
    pub first_compression_block: Option<u64>,
    pub last_compression_block: u64,
    pub round: u8,
    pub cycle: u8,
    pub phase: Phase,
    /// Last block of the current phase: the no-flow limit, the pause limit,
    /// or the end of the open window.
    pub deadline_block: u64,
    pub shock_open: bool,
    pub epi_open: bool,
    pub cycle_compressions: u8,
    pub cycle_breaths: u8,
    pub cycle_lead: Option<A>,
    pub perfusion: u32,
    /// Fixed once the first compression lands; until then, what it would be now.
    pub target: u32,
    pub etco2_bps: u16,
    pub units: Vec<(A, u32)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Monitor<A> {
    pub block: u64,
    pub status: StatusView,
    pub code: Option<CodeView<A>>,
    pub last_beat_block: u64,
    pub alive_since_block: u64,
    pub scars: u8,
    pub deaths: u8,
    pub max_deaths: u8,
    pub ejection_fraction: u8,
    pub beats: Vec<Beat>,
    pub buckets: Vec<Bucket>,
    pub pools: Pools,
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub supply: u128,
    pub totals: Totals,
    pub config: Config,
}
