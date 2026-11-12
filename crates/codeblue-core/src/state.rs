//! The state machine: vitals, the code, and every token flow.
use crate::config::{Config, BPS, CENTI};
use crate::error::Error;
use crate::events::{
    BurnKind, CodeSheet, DeathReason, Event, Outcome, PayKind, Rhythm, RoundLog,
};
use crate::host::Host;
use crate::math::mul_div;
use crate::ring::Ring;
use crate::U128;

/// Beats kept for the monitor trace.
pub const BEAT_RING: u16 = 64;
/// Activity buckets kept for the monitor.
pub const BUCKET_RING: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Beat {
    pub block: u64,
    /// Four bytes standing for the sender.
    pub tag: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bucket {
    /// `block / bucket_blocks`. `u64::MAX` marks a slot never written.
    pub index: u64,
    pub beats: u32,
    /// Transfers that went through but carried no pulse.
    pub electrical: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vitals {
    pub last_beat_block: u64,
    pub alive_since_block: u64,
    /// Start of the stretch of unbroken life that the next healed scar is counted from.
    pub rehab_anchor_block: u64,
    pub scars: u8,
    pub deaths: u8,
    /// Non-beat transfers since the last beat. Decides PEA.
    pub electrical_since_beat: u32,
    pub beats: Ring<Beat>,
    /// `BUCKET_RING` slots; a bucket lives at `index % BUCKET_RING`.
    pub buckets: Vec<Bucket>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Phase {
    Compressions,
    /// Hands off the chest. Opened by the last compression of a cycle.
    Ventilation { opened: u64 },
    /// Rhythm check. Open only while a shock or a dose is available.
    AfterRound { opened: u64, shock_open: bool, epi_open: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Code<A> {
    pub code_no: u32,
    pub arrest_block: u64,
    pub rhythm: Rhythm,
    pub rhythm_at_arrest: Rhythm,
    /// Set by `call_code()`, or by the first counted compression.
    pub called: bool,
    pub first_compression_block: Option<u64>,
    pub last_compression_block: u64,
    /// Both count from 1.
    pub round: u8,
    pub cycle: u8,
    pub phase: Phase,

    pub cycle_compressions: u8,
    pub cycle_first_block: u64,
    /// Blocks from the first to the last compression of the cycle, inclusive.
    pub cycle_span: u64,
    pub cycle_breaths: u8,
    pub last_breath_block: Option<u64>,
    /// Who did the most compressions this cycle. They may not take the airway.
    pub cycle_lead: Option<A>,
    pub cycle_by: Vec<(A, u32)>,
    pub round_by: Vec<(A, u32)>,
    /// Quality of each closed cycle of the current round.
    pub cycle_quality_bps: Vec<u16>,
    /// Quality of the last finished round. Decides whether a shock works.
    pub round_quality_bps: u16,

    /// Hundredths of a point. Never falls within a code.
    pub perfusion: u32,
    /// Hundredths of a point. Zero until the first compression fixes it.
    pub target: u32,
    pub prev_lead_compressor: Option<A>,
    /// Work units per rescuer. They decide the bounty split.
    pub units: Vec<(A, u32)>,
    pub rounds: Vec<RoundLog<A>>,
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub noflow_burn: u128,
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub epi_burn: u128,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Status<A> {
    /// What is stored while the coin lives. Arrest is derived from the height.
    Alive,
    Arrest(Code<A>),
    /// After a time of death, before the next code opens at `until_block`.
    Morgue { since_block: u64, until_block: u64 },
    Deceased { block: u64 },
}

/// Where the contract's own token balance belongs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Pools {
    /// The crash cart: pays bounties and takes every arrest burn.
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub cart: u128,
    /// What `faucet_claim()` still has to give.
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub faucet: u128,
    /// Bounty shares credited to rescuers and not yet claimed. Already out of the cart.
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub claimable: u128,
}

/// Lifetime counters. Display only; no rule reads them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Totals {
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub burned_oxygen: u128,
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub burned_noflow: u128,
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub burned_epi: u128,
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub burned_death: u128,
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub burned_estate: u128,
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub premiums: u128,
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub donations: u128,
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub bounties: u128,
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub faucet_paid: u128,
    pub beats: u64,
    pub electrical: u64,
    pub arrests: u32,
    pub saves: u32,
}

impl Totals {
    pub fn burned(&self) -> u128 {
        self.burned_oxygen + self.burned_noflow + self.burned_epi + self.burned_death + self.burned_estate
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct State<A> {
    pub config: Config,
    pub status: Status<A>,
    pub vitals: Vitals,
    pub pools: Pools,
    pub totals: Totals,
    pub genesis_block: u64,
}

/// What a press did. A press that does not count changes nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tap {
    Counted,
    AlreadyThisBlock,
    /// A compression during a ventilation window or a rhythm check.
    HandsOff,
}

impl Tap {
    pub fn as_str(&self) -> &'static str {
        match self {
            Tap::Counted => "counted",
            Tap::AlreadyThisBlock => "already_this_block",
            Tap::HandsOff => "hands_off",
        }
    }
}

/// What a live transfer cost the sender on top of the amount.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Transfer {
    pub oxygen_burn: u128,
    pub premium: u128,
    pub beat: bool,
}

enum Due {
    Death(DeathReason),
    VentLapse,
    AfterRoundLapse,
}

fn bump<A: Clone + PartialEq>(tally: &mut Vec<(A, u32)>, who: &A, by: u32) {
    match tally.iter_mut().find(|(a, _)| a == who) {
        Some((_, n)) => *n += by,
        None => tally.push((who.clone(), by)),
    }
}

/// The account with the highest count. On a tie, the one that got there first.
fn leader<A: Clone>(tally: &[(A, u32)]) -> Option<A> {
    let mut best: Option<&(A, u32)> = None;
    for entry in tally {
        if best.map_or(true, |b| entry.1 > b.1) {
            best = Some(entry);
        }
    }
    best.map(|(a, _)| a.clone())
}

impl<A: Clone + PartialEq> State<A> {
    /// A newborn coin. The host must mint `config.total_supply` into the
    /// contract's own balance in the same breath.
    pub fn new(config: Config, genesis_block: u64) -> Result<Self, &'static str> {
        config.validate()?;
        let pools = Pools { cart: config.cart_supply(), faucet: config.faucet_supply(), claimable: 0 };
        Ok(State {
            config,
            status: Status::Alive,
            vitals: Vitals {
                last_beat_block: genesis_block,
                alive_since_block: genesis_block,
                rehab_anchor_block: genesis_block,
                scars: 0,
                deaths: 0,
                electrical_since_beat: 0,
                beats: Ring::new(BEAT_RING),
                buckets: vec![Bucket { index: u64::MAX, beats: 0, electrical: 0 }; BUCKET_RING],
            },
            pools,
            totals: Totals::default(),
            genesis_block,
        })
    }

    // ── Derived numbers ────────────────────────────────────────────────

    /// `EF = max(EF_FLOOR, EF_START - EF_STEP * scars)`.
    pub fn ejection_fraction(&self) -> u8 {
        let c = &self.config;
        let lost = c.ef_step as u32 * self.vitals.scars as u32;
        (c.ef_start as u32).saturating_sub(lost).max(c.ef_floor as u32) as u8
    }

    /// The target a code would get if its first compression landed after
    /// `no_flow_blocks`, in hundredths of a point.
    pub fn target_for(&self, no_flow_blocks: u64) -> u32 {
        let c = &self.config;
        let fixed = (c.base_target + c.scar_target * self.vitals.scars as u32) * CENTI;
        let cap = (c.noflow_target_cap * CENTI) as u64;
        let delay = (no_flow_blocks.saturating_mul(CENTI as u64) / c.noflow_target_blocks).min(cap);
        fixed + delay as u32
    }

    /// First block at which the stored life would be in arrest.
    pub fn arrest_due_block(&self) -> u64 {
        self.vitals.last_beat_block + self.config.arrest_blocks + 1
    }

    fn epi_round(&self, round: u8) -> bool {
        let c = &self.config;
        round >= c.epi_first_round && (round - c.epi_first_round) % c.epi_every_rounds == 0
    }

    fn classify(&self) -> Rhythm {
        let (c, v) = (&self.config, &self.vitals);
        let window_start = v.last_beat_block.saturating_sub(c.bucket_blocks);
        let recent = v
            .beats
            .iter()
            .filter(|b| b.block > window_start && b.block <= v.last_beat_block)
            .count() as u32;
        if recent >= c.sudden_beats {
            Rhythm::Vf
        } else if v.electrical_since_beat >= c.pea_min {
            Rhythm::Pea
        } else {
            Rhythm::Asystole
        }
    }

    // ── Time ───────────────────────────────────────────────────────────

    /// Applies every transition the block height has made due, each stamped
    /// with the block at which it became true. Idempotent, and
    /// `advance(t2)` after `advance(t1)` equals `advance(t2)` alone.
    pub(crate) fn advance<H: Host<Account = A>>(&mut self, host: &mut H, now: u64) {
        loop {
            let (at, due) = match &self.status {
                Status::Deceased { .. } => return,
                Status::Alive => {
                    let arrest_at = self.arrest_due_block();
                    self.rehab(host, now.min(arrest_at - 1));
                    if now < arrest_at {
                        return;
                    }
                    let rhythm = self.classify();
                    self.open_code(host, arrest_at, rhythm);
                    continue;
                }
                Status::Morgue { until_block, .. } => {
                    let until = *until_block;
                    if now < until {
                        return;
                    }
                    self.open_code(host, until, Rhythm::Asystole);
                    continue;
                }
                Status::Arrest(code) => {
                    let c = &self.config;
                    match (code.first_compression_block, code.phase) {
                        (None, _) => (code.arrest_block + c.no_flow_max_blocks + 1, Due::Death(DeathReason::NoFlow)),
                        (Some(_), Phase::Compressions) => {
                            (code.last_compression_block + c.max_pause_blocks + 1, Due::Death(DeathReason::Pause))
                        }
                        (Some(_), Phase::Ventilation { opened }) => (opened + c.vent_window_blocks + 1, Due::VentLapse),
                        (Some(_), Phase::AfterRound { opened, .. }) => {
                            (opened + c.vent_window_blocks + 1, Due::AfterRoundLapse)
                        }
                    }
                }
            };
            if now < at {
                return;
            }
            let code = self.take_code();
            match due {
                Due::Death(reason) => self.die(host, code, at, reason),
                Due::VentLapse => self.close_cycle(host, code, at),
                Due::AfterRoundLapse => self.finish_after_round(host, code, at),
            }
        }
    }

    /// Writes the derived state as of `now`. The contract has no such call:
    /// every method does this first. Tests use it to check that writing
    /// early never changes what a later call sees.
    #[doc(hidden)]
    pub fn settle<H: Host<Account = A>>(&mut self, host: &mut H, now: u64) {
        self.advance(host, now);
    }

    fn rehab<H: Host<Account = A>>(&mut self, host: &mut H, life_end: u64) {
        let step = self.config.rehab_blocks;
        while self.vitals.scars > 0 && life_end >= self.vitals.rehab_anchor_block + step {
            self.vitals.rehab_anchor_block += step;
            self.vitals.scars -= 1;
            host.emit(Event::Rehab { block: self.vitals.rehab_anchor_block, scars: self.vitals.scars });
        }
    }

    fn open_code<H: Host<Account = A>>(&mut self, host: &mut H, at: u64, rhythm: Rhythm) {
        self.totals.arrests += 1;
        let code_no = self.totals.arrests;
        self.status = Status::Arrest(Code {
            code_no,
            arrest_block: at,
            rhythm,
            rhythm_at_arrest: rhythm,
            called: false,
            first_compression_block: None,
            last_compression_block: at,
            round: 1,
            cycle: 1,
            phase: Phase::Compressions,
            cycle_compressions: 0,
            cycle_first_block: at,
            cycle_span: 0,
            cycle_breaths: 0,
            last_breath_block: None,
            cycle_lead: None,
            cycle_by: Vec::new(),
            round_by: Vec::new(),
            cycle_quality_bps: Vec::new(),
            round_quality_bps: 0,
            perfusion: 0,
            target: 0,
            prev_lead_compressor: None,
            units: Vec::new(),
            rounds: Vec::new(),
            noflow_burn: 0,
            epi_burn: 0,
        });
        host.emit(Event::Arrest { block: at, code_no, rhythm });
    }

    fn take_code(&mut self) -> Code<A> {
        match core::mem::replace(&mut self.status, Status::Alive) {
            Status::Arrest(code) => code,
            _ => unreachable!("no code is running"),
        }
    }

    // ── Cycle, round, outcome ──────────────────────────────────────────

    fn close_cycle<H: Host<Account = A>>(&mut self, host: &mut H, mut code: Code<A>, at: u64) {
        let c = &self.config;
        let bps = BPS;
        // compression_part = min(1, 30 / blocks_from_first_to_30th_compression)
        let compression_part = (c.compressions_per_cycle as u64 * bps / code.cycle_span.max(1)).min(bps);
        // cycle_quality = compression_part * (0.70 + 0.15 * breaths_given)
        let breath_part = c.hands_only_bps as u64 + c.breath_bps as u64 * code.cycle_breaths as u64;
        let quality = compression_part * breath_part / bps;
        code.cycle_quality_bps.push(quality as u16);

        code.cycle_compressions = 0;
        code.cycle_span = 0;
        code.cycle_breaths = 0;
        code.last_breath_block = None;
        code.cycle_lead = None;
        code.cycle_by.clear();

        if code.cycle >= c.cycles_per_round {
            self.end_round(host, code, at);
        } else {
            code.cycle += 1;
            code.phase = Phase::Compressions;
            self.status = Status::Arrest(code);
        }
    }

    fn end_round<H: Host<Account = A>>(&mut self, host: &mut H, mut code: Code<A>, at: u64) {
        let c = &self.config;
        let cycles = code.cycle_quality_bps.len() as u64;
        let mean = code.cycle_quality_bps.iter().map(|q| *q as u64).sum::<u64>() / cycles.max(1);
        let lead = leader(&code.round_by);
        // fatigue = 0.80 if the lead compressor also led the previous round, else 1
        let fatigued = lead.is_some() && lead == code.prev_lead_compressor;
        let quality = if fatigued { mean * c.fatigue_bps as u64 / BPS } else { mean };

        // perfusion += 100 * round_quality. One basis point of quality is one
        // hundredth of a point.
        code.perfusion += quality as u32;
        code.round_quality_bps = quality as u16;
        code.rounds.push(RoundLog {
            round: code.round,
            end_block: at,
            quality_bps: quality as u16,
            fatigued,
            lead: lead.clone(),
            shock: None,
            epi: false,
            perfusion_after: code.perfusion,
        });
        host.emit(Event::Round {
            block: at,
            code_no: code.code_no,
            round: code.round,
            quality_bps: quality as u16,
            fatigued,
            perfusion: code.perfusion,
            target: code.target,
        });
        code.prev_lead_compressor = lead;
        code.round_by.clear();
        code.cycle_quality_bps.clear();

        if code.perfusion >= code.target {
            return self.rosc(host, code, at);
        }
        let shock_open = code.rhythm == Rhythm::Vf;
        let epi_open = self.epi_round(code.round) && self.pools.cart >= self.config.epi_dose;
        if shock_open || epi_open {
            code.phase = Phase::AfterRound { opened: at, shock_open, epi_open };
            self.status = Status::Arrest(code);
        } else {
            self.finish_after_round(host, code, at);
        }
    }

    fn finish_after_round<H: Host<Account = A>>(&mut self, host: &mut H, mut code: Code<A>, at: u64) {
        let c = &self.config;
        if code.round >= c.max_rounds {
            return self.die(host, code, at, DeathReason::RoundsExhausted);
        }
        code.round += 1;
        code.cycle = 1;
        code.phase = Phase::Compressions;
        if code.rhythm == Rhythm::Vf && code.round >= c.vf_degrade_round {
            code.rhythm = Rhythm::Asystole;
        }
        self.status = Status::Arrest(code);
    }

    fn sheet(&self, code: Code<A>, end_block: u64, outcome: Outcome) -> CodeSheet<A> {
        CodeSheet {
            code_no: code.code_no,
            arrest_block: code.arrest_block,
            rhythm_at_arrest: code.rhythm_at_arrest,
            first_compression_block: code.first_compression_block,
            end_block,
            outcome,
            target: code.target,
            perfusion: code.perfusion,
            rounds: code.rounds,
            units: code.units,
            bounty: 0,
            shares: Vec::new(),
            noflow_burn: code.noflow_burn,
            epi_burn: code.epi_burn,
            death_burn: 0,
            scars_after: self.vitals.scars,
            deaths_after: self.vitals.deaths,
        }
    }

    /// Return of spontaneous circulation at block `at`.
    fn rosc<H: Host<Account = A>>(&mut self, host: &mut H, code: Code<A>, at: u64) {
        let c = &self.config;
        // bounty = cart * BOUNTY_BPS * EF / EF_START, with the damage as it
        // stood during the code.
        let pool = mul_div(
            mul_div(self.pools.cart, c.bounty_bps as u64, BPS),
            self.ejection_fraction() as u64,
            c.ef_start as u64,
        );
        let total_units: u64 = code.units.iter().map(|(_, u)| *u as u64).sum();
        let mut shares = Vec::with_capacity(code.units.len());
        let mut paid = 0u128;
        if total_units > 0 {
            for (who, units) in &code.units {
                let share = mul_div(pool, *units as u64, total_units);
                if share == 0 {
                    continue;
                }
                paid += share;
                host.set_claimable_of(who, host.claimable_of(who) + share);
                host.emit(Event::BountyCredited { block: at, account: who.clone(), amount: share });
                shares.push((who.clone(), U128(share)));
            }
        }
        // Rounding dust stays in the cart.
        self.pools.cart -= paid;
        self.pools.claimable += paid;
        self.totals.bounties += paid;
        self.totals.saves += 1;

        let v = &mut self.vitals;
        v.scars = v.scars.saturating_add(c.rosc_scars);
        v.last_beat_block = at;
        v.alive_since_block = at;
        v.rehab_anchor_block = at;
        v.electrical_since_beat = 0;
        self.status = Status::Alive;

        let mut sheet = self.sheet(code, at, Outcome::Rosc);
        sheet.bounty = paid;
        sheet.shares = shares;
        host.emit(Event::Rosc { block: at, sheet });
    }

    /// Time of death at block `at`.
    fn die<H: Host<Account = A>>(&mut self, host: &mut H, code: Code<A>, at: u64, reason: DeathReason) {
        let c = &self.config;
        let (final_at, lazarus, burn_bps) = (c.max_deaths, c.lazarus_blocks, c.death_burn_bps);
        let v = &mut self.vitals;
        v.deaths = v.deaths.saturating_add(1);
        v.scars = v.scars.saturating_add(c.death_scars);
        let deaths = v.deaths;

        let burn = mul_div(self.pools.cart, burn_bps as u64, BPS);
        self.burn_cart(host, at, BurnKind::Death, burn);

        let final_death = deaths >= final_at;
        let mut sheet = self.sheet(code, at, Outcome::Death(reason));
        sheet.death_burn = burn;
        host.emit(Event::TimeOfDeath { block: at, reason, deaths, sheet });

        if final_death {
            // The estate: whatever is left in the cart.
            let estate = self.pools.cart;
            self.burn_cart(host, at, BurnKind::Estate, estate);
            self.status = Status::Deceased { block: at };
            host.emit(Event::Deceased { block: at, estate });
        } else {
            self.status = Status::Morgue { since_block: at, until_block: at + lazarus };
        }
    }

    fn burn_cart<H: Host<Account = A>>(&mut self, host: &mut H, at: u64, kind: BurnKind, amount: u128) {
        if amount == 0 {
            return;
        }
        self.pools.cart -= amount;
        host.burn_from_contract(amount, kind);
        match kind {
            BurnKind::NoFlow => self.totals.burned_noflow += amount,
            BurnKind::Epinephrine => self.totals.burned_epi += amount,
            BurnKind::Death => self.totals.burned_death += amount,
            BurnKind::Estate => self.totals.burned_estate += amount,
            BurnKind::Oxygen => unreachable!("oxygen burns come from the sender"),
        }
        host.emit(Event::Burn { block: at, kind, amount });
    }

    // ── Calls ──────────────────────────────────────────────────────────

    /// Status and balance gate shared by every rescue call.
    fn rescuer<H: Host<Account = A>>(&self, host: &H, who: &A) -> Result<(), Error> {
        match &self.status {
            Status::Alive => Err(Error::HasPulse),
            Status::Morgue { until_block, .. } => Err(Error::InMorgue { until_block: *until_block }),
            Status::Deceased { block } => Err(Error::Deceased { block: *block }),
            Status::Arrest(_) => {
                let min_balance = self.config.min_rescuer_balance;
                if host.balance_of(who) < min_balance {
                    Err(Error::NotARescuer { min_balance })
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Fees and the beat test for a transfer of `amount` from `sender` to
    /// `receiver`. Call it before moving the amount itself; the host moves
    /// that. `to_cart` marks a transfer to the contract's own account, which
    /// is a donation to the crash cart.
    pub fn on_transfer<H: Host<Account = A>>(
        &mut self,
        host: &mut H,
        now: u64,
        sender: &A,
        receiver: &A,
        amount: u128,
        to_cart: bool,
    ) -> Result<Transfer, Error> {
        self.advance(host, now);
        match &self.status {
            Status::Alive => {}
            Status::Arrest(_) | Status::Morgue { .. } => return Err(Error::NoPulse),
            Status::Deceased { block } => {
                // Post-mortem transfers move tokens and nothing else.
                if !self.config.post_mortem_transfers {
                    return Err(Error::Deceased { block: *block });
                }
                if to_cart {
                    // Nothing spends the cart any more. This only keeps the books whole.
                    self.pools.cart += amount;
                    self.totals.donations += amount;
                }
                return Ok(Transfer::default());
            }
        }
        let c = &self.config;
        let oxygen_burn = mul_div(amount, c.o2_bps as u64, BPS);
        let premium = mul_div(amount, c.premium_bps as u64, BPS);
        let need = amount.saturating_add(oxygen_burn).saturating_add(premium);
        let have = host.balance_of(sender);
        if have < need {
            return Err(Error::InsufficientBalance { need, have });
        }
        if oxygen_burn > 0 {
            host.burn_from_holder(sender, oxygen_burn);
            self.totals.burned_oxygen += oxygen_burn;
            host.emit(Event::Burn { block: now, kind: BurnKind::Oxygen, amount: oxygen_burn });
        }
        if premium > 0 {
            host.holder_to_contract(sender, premium);
            self.pools.cart += premium;
            self.totals.premiums += premium;
        }
        if to_cart {
            self.pools.cart += amount;
            self.totals.donations += amount;
        }

        // A beat: enough stroke volume, two different accounts, and a sender
        // whose last beat is at least a refractory period old.
        let rested = match host.last_beat_of(sender) {
            Some(last) => now >= last + c.refractory_blocks,
            None => true,
        };
        let beat = amount >= c.min_stroke && sender != receiver && rested;

        let index = now / c.bucket_blocks;
        let slot = &mut self.vitals.buckets[(index % BUCKET_RING as u64) as usize];
        if slot.index != index {
            *slot = Bucket { index, beats: 0, electrical: 0 };
        }
        if beat {
            slot.beats += 1;
            self.vitals.last_beat_block = now;
            self.vitals.electrical_since_beat = 0;
            self.vitals.beats.push(Beat { block: now, tag: host.tag_of(sender) });
            self.totals.beats += 1;
            host.set_last_beat_of(sender, now);
        } else {
            // Electrical activity with no pulse. It does not reset the arrest clock.
            slot.electrical += 1;
            self.vitals.electrical_since_beat += 1;
            self.totals.electrical += 1;
        }
        Ok(Transfer { oxygen_burn, premium, beat })
    }

    /// Pays `faucet_amount` once per account until the pool is empty.
    pub fn faucet_claim<H: Host<Account = A>>(&mut self, host: &mut H, now: u64, who: &A) -> Result<u128, Error> {
        self.advance(host, now);
        if let Status::Deceased { block } = &self.status {
            return Err(Error::Deceased { block: *block });
        }
        if host.faucet_claimed(who) {
            return Err(Error::FaucetAlreadyClaimed);
        }
        let amount = self.config.faucet_amount.min(self.pools.faucet);
        if amount == 0 {
            return Err(Error::FaucetEmpty);
        }
        self.pools.faucet -= amount;
        self.totals.faucet_paid += amount;
        host.set_faucet_claimed(who);
        host.contract_to_holder(who, amount, PayKind::Faucet);
        host.emit(Event::Paid { block: now, kind: PayKind::Faucet, account: who.clone(), amount });
        Ok(amount)
    }

    /// Calls the code: the first rescue call of an arrest, before any
    /// compression. Earns `units_call_code`.
    pub fn call_code<H: Host<Account = A>>(&mut self, host: &mut H, now: u64, who: &A) -> Result<Rhythm, Error> {
        self.advance(host, now);
        self.rescuer(host, who)?;
        let units = self.config.units_call_code;
        let Status::Arrest(code) = &mut self.status else { unreachable!() };
        if code.called {
            return Err(Error::CodeAlreadyCalled);
        }
        code.called = true;
        bump(&mut code.units, who, units);
        Ok(code.rhythm)
    }

    /// One chest compression. At most one counts per block, whoever sends it.
    pub fn compress<H: Host<Account = A>>(&mut self, host: &mut H, now: u64, who: &A) -> Result<Tap, Error> {
        self.advance(host, now);
        self.rescuer(host, who)?;
        let mut code = self.take_code();
        if code.phase != Phase::Compressions {
            self.status = Status::Arrest(code);
            return Ok(Tap::HandsOff);
        }
        if code.first_compression_block.is_some() && code.last_compression_block == now {
            self.status = Status::Arrest(code);
            return Ok(Tap::AlreadyThisBlock);
        }
        if code.first_compression_block.is_none() {
            // The first compression ends the no-flow period, fixes the target
            // and burns the cost of the delay from the crash cart.
            let no_flow = now - code.arrest_block;
            code.first_compression_block = Some(now);
            code.called = true;
            code.target = self.target_for(no_flow);
            let c = &self.config;
            let per_step = mul_div(self.pools.cart, c.noflow_burn_bps as u64, BPS);
            let burn = mul_div(per_step, no_flow, c.noflow_burn_blocks).min(self.pools.cart);
            code.noflow_burn = burn;
            self.burn_cart(host, now, BurnKind::NoFlow, burn);
        }
        let c = &self.config;
        if code.cycle_compressions == 0 {
            code.cycle_first_block = now;
        }
        code.cycle_compressions += 1;
        code.last_compression_block = now;
        bump(&mut code.cycle_by, who, 1);
        bump(&mut code.round_by, who, 1);
        bump(&mut code.units, who, c.units_compression);

        if code.cycle_compressions >= c.compressions_per_cycle {
            code.cycle_span = now - code.cycle_first_block + 1;
            code.cycle_lead = leader(&code.cycle_by);
            if c.breaths_per_cycle == 0 {
                self.close_cycle(host, code, now);
                return Ok(Tap::Counted);
            }
            code.phase = Phase::Ventilation { opened: now };
        }
        self.status = Status::Arrest(code);
        Ok(Tap::Counted)
    }

    /// One breath, inside the ventilation window, from anyone but the cycle's
    /// lead compressor. At most one counts per block.
    pub fn breathe<H: Host<Account = A>>(&mut self, host: &mut H, now: u64, who: &A) -> Result<Tap, Error> {
        self.advance(host, now);
        self.rescuer(host, who)?;
        let Status::Arrest(code) = &self.status else { unreachable!() };
        if !matches!(code.phase, Phase::Ventilation { .. }) {
            return Err(Error::AirwayClosed);
        }
        if code.cycle_lead.as_ref() == Some(who) {
            return Err(Error::OnTheChest);
        }
        if code.last_breath_block == Some(now) {
            return Ok(Tap::AlreadyThisBlock);
        }
        let mut code = self.take_code();
        code.cycle_breaths += 1;
        code.last_breath_block = Some(now);
        bump(&mut code.units, who, self.config.units_breath);
        if code.cycle_breaths >= self.config.breaths_per_cycle {
            self.close_cycle(host, code, now);
        } else {
            self.status = Status::Arrest(code);
        }
        Ok(Tap::Counted)
    }

    /// One shock per round, at the rhythm check, on VF only. Returns whether
    /// it added perfusion: it does when the round's quality was high enough.
    pub fn shock<H: Host<Account = A>>(&mut self, host: &mut H, now: u64, who: &A) -> Result<bool, Error> {
        self.advance(host, now);
        self.rescuer(host, who)?;
        let Status::Arrest(code) = &self.status else { unreachable!() };
        if code.rhythm != Rhythm::Vf {
            return Err(Error::NotShockable);
        }
        let Phase::AfterRound { opened, shock_open: true, epi_open } = code.phase else {
            return Err(Error::ShockNotNow);
        };
        let c = &self.config;
        let (bonus, units) = (c.shock_bonus * CENTI, c.units_shock);
        let mut code = self.take_code();
        let success = code.round_quality_bps >= self.config.shock_min_quality_bps;
        if success {
            code.perfusion += bonus;
            bump(&mut code.units, who, units);
        }
        if let Some(log) = code.rounds.last_mut() {
            log.shock = Some(success);
            log.perfusion_after = code.perfusion;
        }
        host.emit(Event::Shock { block: now, round: code.round, success, perfusion: code.perfusion });
        code.phase = Phase::AfterRound { opened, shock_open: false, epi_open };
        self.after_intervention(host, code, now, epi_open);
        Ok(success)
    }

    /// One dose per window: burns `epi_dose` from the cart, adds `epi_bonus`.
    pub fn push_epi<H: Host<Account = A>>(&mut self, host: &mut H, now: u64, who: &A) -> Result<(), Error> {
        self.advance(host, now);
        self.rescuer(host, who)?;
        let Status::Arrest(code) = &self.status else { unreachable!() };
        let Phase::AfterRound { opened, shock_open, epi_open: true } = code.phase else {
            return Err(Error::EpiNotYet);
        };
        let c = &self.config;
        let (dose, bonus) = (c.epi_dose, c.epi_bonus * CENTI);
        if self.pools.cart < dose {
            return Err(Error::CartEmpty);
        }
        let mut code = self.take_code();
        code.epi_burn += dose;
        self.burn_cart(host, now, BurnKind::Epinephrine, dose);
        code.perfusion += bonus;
        if let Some(log) = code.rounds.last_mut() {
            log.epi = true;
            log.perfusion_after = code.perfusion;
        }
        host.emit(Event::Epi { block: now, round: code.round, perfusion: code.perfusion });
        code.phase = Phase::AfterRound { opened, shock_open, epi_open: false };
        self.after_intervention(host, code, now, shock_open);
        Ok(())
    }

    /// ROSC test after a change to perfusion, then close the rhythm check
    /// once nothing is left to give.
    fn after_intervention<H: Host<Account = A>>(&mut self, host: &mut H, code: Code<A>, now: u64, more_open: bool) {
        if code.perfusion >= code.target {
            self.rosc(host, code, now);
        } else if more_open {
            self.status = Status::Arrest(code);
        } else {
            self.finish_after_round(host, code, now);
        }
    }

    /// Writes a death whose condition already holds. Anyone may call it.
    /// Returns the death count.
    pub fn call_time_of_death<H: Host<Account = A>>(&mut self, host: &mut H, now: u64) -> Result<u8, Error> {
        let before = self.vitals.deaths;
        if let Status::Deceased { block } = &self.status {
            return Err(Error::Deceased { block: *block });
        }
        self.advance(host, now);
        if self.vitals.deaths > before {
            return Ok(self.vitals.deaths);
        }
        Err(match &self.status {
            Status::Alive => Error::NoDeathToCall,
            Status::Arrest(_) => Error::StillInCode,
            Status::Morgue { until_block, .. } => Error::InMorgue { until_block: *until_block },
            Status::Deceased { block } => Error::Deceased { block: *block },
        })
    }

    /// Pays the caller's claimable bounty. No fee, not a beat.
    pub fn claim_bounty<H: Host<Account = A>>(&mut self, host: &mut H, now: u64, who: &A) -> Result<u128, Error> {
        self.advance(host, now);
        if let Status::Deceased { block } = &self.status {
            if !self.config.post_mortem_transfers {
                return Err(Error::Deceased { block: *block });
            }
        }
        let amount = host.claimable_of(who);
        if amount == 0 {
            return Err(Error::NothingToClaim);
        }
        host.set_claimable_of(who, 0);
        self.pools.claimable -= amount;
        host.contract_to_holder(who, amount, PayKind::Bounty);
        host.emit(Event::Paid { block: now, kind: PayKind::Bounty, account: who.clone(), amount });
        Ok(amount)
    }
}
