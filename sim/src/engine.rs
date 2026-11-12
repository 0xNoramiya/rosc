//! One simulated life of the coin.
//!
//! The engine owns the people: when holders transfer, who answers a code and
//! how well they press. Every consequence comes from `codeblue-core`, through
//! the same calls the contract exposes.
use codeblue_core::mem::{Coin, Sink};
use codeblue_core::{
    CodeSheet, Config, DeathReason, Event, Outcome, Phase, Rhythm, Status, BPS, ROSC,
};

use crate::rng::{hash, Rng};
use crate::scenario::{Scenario, DAY};

/// What the simulator keeps from the rulebook's event stream.
pub struct Tape {
    pub sheets: Vec<CodeSheet<u32>>,
    pub bounty_by: Vec<u128>,
    pub first_arrest: Option<u64>,
    pub deceased: Option<u64>,
}

impl Sink for Tape {
    fn on_event(&mut self, event: Event<u32>) {
        match event {
            Event::Arrest { block, .. } => {
                self.first_arrest.get_or_insert(block);
            }
            Event::Rosc { sheet, .. } | Event::TimeOfDeath { sheet, .. } => self.sheets.push(sheet),
            Event::BountyCredited { account, amount, .. } => self.bounty_by[account as usize] += amount,
            Event::Deceased { block, .. } => self.deceased = Some(block),
            _ => {}
        }
    }
}

/// The coin at the start of a day.
#[derive(Clone, Copy)]
pub struct Day {
    pub supply: u128,
    pub cart: u128,
    /// 0 pulse, 1 arrest, 2 morgue, 3 deceased.
    pub status: u8,
}

pub struct CodeRow {
    pub code_no: u32,
    pub arrest_block: u64,
    pub night: bool,
    pub rhythm: Rhythm,
    pub responders: u32,
    pub response_blocks: Option<u64>,
    pub no_flow_blocks: Option<u64>,
    pub rounds: u8,
    pub outcome: Outcome,
    pub downtime_blocks: u64,
    /// First compression to ROSC.
    pub save_blocks: Option<u64>,
    pub bounty: u128,
}

pub struct RunOutput {
    pub seed: u64,
    pub days: Vec<Day>,
    pub codes: Vec<CodeRow>,
    pub arrests: u32,
    pub saves: u32,
    pub deaths: u8,
    pub first_arrest: Option<u64>,
    pub deceased: Option<u64>,
    pub bounty_total: u128,
    pub bounty_top: u128,
    pub pacer_fees: u128,
    pub pacer_beats: u64,
    pub totals: codeblue_core::Totals,
    pub final_supply: u128,
    pub final_cart: u128,
}

struct Meta {
    code_no: u32,
    night: bool,
    responders: u32,
    response_blocks: Option<u64>,
}

struct Run<'a> {
    sc: &'a Scenario,
    coin: Coin<Tape>,
    rng: Rng,
    horizon: u64,
    max_days: u64,
    days: Vec<Day>,
    holders: u32,
    rescuers: Vec<u32>,
    pacer: Option<(u32, u32)>,
    serial: Option<(u32, u32)>,
    next_holder: Option<u64>,
    next_pacer: Option<u64>,
    pacer_interval: u64,
    pacer_flip: bool,
    pacer_fees: u128,
    pacer_beats: u64,
    meta: Vec<Meta>,
    /// Block the last code episode ended on.
    clock: u64,
}

/// A metronome-driven press lands in nearly every block.
const SERIAL_HIT: f64 = 0.95;
/// Blocks a bot needs to notice an arrest.
const SERIAL_DELAY: u64 = 3;

pub fn run(sc: &Scenario, config: &Config, seed: u64, days: u64) -> RunOutput {
    let holders = sc.holders;
    let mut next_account = holders;
    let mut take = |n: u32| {
        let first = next_account;
        next_account += n;
        first
    };
    let rescuers: Vec<u32> = if holders >= sc.rescuers {
        (0..sc.rescuers).collect()
    } else {
        let first = take(sc.rescuers);
        (first..first + sc.rescuers).collect()
    };
    let pacer = sc.pacemaker.map(|_| {
        let first = take(2);
        (first, first + 1)
    });
    let serial = sc.serial_rescuer.then(|| {
        let first = take(2);
        (first, first + 1)
    });
    let accounts = next_account as usize;

    let tape = Tape { sheets: Vec::new(), bounty_by: vec![0; accounts], first_arrest: None, deceased: None };
    let mut coin = Coin::launch(config.clone(), 0, accounts, tape);
    for who in 0..accounts as u32 {
        coin.state.faucet_claim(&mut coin.bank, 0, &who).expect("the faucet covers every agent");
    }

    let pacer_interval = sc.pacemaker.map_or(0, |share| ((config.arrest_blocks as f64 * share) as u64).max(1));
    let mut run = Run {
        sc,
        coin,
        rng: Rng::new(seed, hash(sc.name)),
        horizon: days * DAY,
        max_days: days,
        days: Vec::with_capacity(days as usize + 1),
        holders,
        rescuers,
        pacer,
        serial,
        next_holder: None,
        next_pacer: pacer.map(|_| pacer_interval),
        pacer_interval,
        pacer_flip: false,
        pacer_fees: 0,
        pacer_beats: 0,
        meta: Vec::new(),
        clock: 0,
    };
    run.play();
    run.finish(seed)
}

impl Run<'_> {
    // ── Holders ────────────────────────────────────────────────────────

    /// Transfers per block across all holders at block `t`.
    fn rate(&self, t: f64) -> f64 {
        let day = t / DAY as f64;
        self.holders as f64 * self.sc.base_rate / DAY as f64 * self.sc.attention(day) * self.sc.daylight(day.fract())
    }

    /// An upper bound on `rate` from block `t` on. Attention never rises.
    fn rate_bound(&self, t: f64) -> f64 {
        let day = t / DAY as f64;
        self.holders as f64 * self.sc.base_rate / DAY as f64 * self.sc.attention(day) * (1.0 + self.sc.swing)
    }

    /// The block of the next holder transfer after `t`, by thinning.
    fn next_holder_after(&mut self, t: u64) -> Option<u64> {
        if self.holders < 2 {
            return None;
        }
        let mut x = t as f64;
        loop {
            let bound = self.rate_bound(x);
            if bound <= 0.0 {
                return None;
            }
            x += self.rng.exponential(bound);
            if x > self.horizon as f64 {
                return None;
            }
            if self.rng.unit() * bound < self.rate(x) {
                return Some((x.ceil() as u64).max(t + 1));
            }
        }
    }

    fn holder_transfer(&mut self, now: u64) {
        let n = self.holders as u64;
        let sender = self.rng.below(n) as u32;
        let mut receiver = self.rng.below(n - 1) as u32;
        if receiver >= sender {
            receiver += 1;
        }
        let c = &self.coin.state.config;
        let fees = (c.o2_bps + c.premium_bps) as u128;
        let spendable = self.coin.bank.balances[sender as usize] * BPS as u128 / (BPS as u128 + fees);
        let want = if self.rng.chance(self.sc.dust_share) {
            c.min_stroke / 2
        } else {
            (self.rng.lognormal(self.sc.amount_median, self.sc.amount_sigma).round() as u128).max(1) * ROSC
        };
        let amount = want.min(spendable);
        if amount > 0 {
            self.coin.transfer(now, sender, receiver, amount).expect("a living coin takes a covered transfer");
        }
    }

    fn pacer_transfer(&mut self, now: u64) {
        let Some((a, b)) = self.pacer else { return };
        let (from, to) = if self.pacer_flip { (b, a) } else { (a, b) };
        self.pacer_flip = !self.pacer_flip;
        let amount = self.coin.state.config.min_stroke;
        if let Ok(out) = self.coin.transfer(now, from, to, amount) {
            self.pacer_fees += out.oxygen_burn + out.premium;
            self.pacer_beats += out.beat as u64;
        }
    }

    // ── Sampling ───────────────────────────────────────────────────────

    /// Records the coin at every day boundary up to block `t`. Uses the
    /// derived state, so nothing is written.
    fn sample_to(&mut self, t: u64) {
        while (self.days.len() as u64) <= self.max_days && self.days.len() as u64 * DAY <= t {
            let block = self.days.len() as u64 * DAY;
            let view = self.coin.state.preview(&self.coin.bank, block);
            let status = match view.state.status {
                Status::Alive => 0,
                Status::Arrest(_) => 1,
                Status::Morgue { .. } => 2,
                Status::Deceased { .. } => 3,
            };
            self.days.push(Day { supply: view.supply, cart: view.state.pools.cart, status });
        }
    }

    // ── The code ───────────────────────────────────────────────────────

    /// Plays the code that opens at `opens_at`, to its end or to the horizon.
    fn run_code(&mut self, opens_at: u64) {
        self.sample_to(opens_at);
        self.coin.state.settle(&mut self.coin.bank, opens_at);
        let Status::Arrest(code) = &self.coin.state.status else { return };
        let code_no = code.code_no;
        let limit = code.arrest_block + self.coin.state.config.no_flow_max_blocks;

        // Who shows up, and when.
        let day = opens_at as f64 / DAY as f64;
        let night = Scenario::is_night(day.fract());
        let mut team: Vec<(u64, u32)> = Vec::new();
        if let Some((a, b)) = self.serial {
            team.push((opens_at + SERIAL_DELAY, a));
            team.push((opens_at + SERIAL_DELAY, b));
        } else if !self.sc.leave_day.is_some_and(|leave| day >= leave) {
            for i in 0..self.rescuers.len() {
                if self.rng.chance(self.sc.show_up) {
                    let mut delay = self.rng.lognormal(self.sc.delay_median, self.sc.delay_sigma);
                    if night {
                        delay *= self.sc.night_delay;
                    }
                    team.push((opens_at + delay.round() as u64, self.rescuers[i]));
                }
            }
        }
        team.sort();
        let first = team.first().map(|(at, _)| *at).filter(|at| *at <= limit);
        self.meta.push(Meta {
            code_no,
            night,
            responders: team.iter().filter(|(at, _)| *at <= limit).count() as u32,
            response_blocks: first.map(|at| at - opens_at),
        });

        let Some(first) = first else {
            // Nobody in time. The death becomes true one block past the limit.
            let end = limit + 1;
            self.sample_to(end.min(self.horizon));
            if end <= self.horizon {
                self.coin.state.settle(&mut self.coin.bank, end);
            }
            self.clock = end;
            return;
        };

        let (hit, airway_on) = if self.serial.is_some() {
            (SERIAL_HIT, true)
        } else {
            ((30.0 / self.sc.blocks_per_30).min(1.0), self.rng.chance(self.sc.airway))
        };
        let mut t = first;
        let mut called = false;
        // Roles are set at the start of each round: the chest rotates.
        let mut roles: (u8, u32, Option<u32>) = (0, 0, None);
        while t <= self.horizon {
            self.sample_to(t);
            self.coin.state.settle(&mut self.coin.bank, t);
            let Status::Arrest(code) = &self.coin.state.status else { break };
            if code.code_no != code_no {
                break;
            }
            let (round, phase) = (code.round, code.phase);
            if roles.0 != round {
                let present = team.iter().take_while(|(at, _)| *at <= t).count();
                let lead = team[(round as usize - 1) % present].1;
                let airway = (present >= 2 && airway_on).then(|| team[round as usize % present].1);
                roles = (round, lead, airway);
            }
            let (_, lead, airway) = roles;
            let (state, bank) = (&mut self.coin.state, &mut self.coin.bank);
            if !called {
                called = true;
                let _ = state.call_code(bank, t, &team[0].1);
            }
            if self.rng.chance(hit) {
                // A press that lands in this block. Refusals are wasted presses.
                let _ = match phase {
                    Phase::Compressions => state.compress(bank, t, &lead).map(|_| ()),
                    Phase::Ventilation { .. } => match airway {
                        Some(airway) => state.breathe(bank, t, &airway).map(|_| ()),
                        None => Ok(()),
                    },
                    Phase::AfterRound { shock_open: true, .. } => state.shock(bank, t, &lead).map(|_| ()),
                    Phase::AfterRound { .. } => state.push_epi(bank, t, &lead),
                };
            }
            t += 1;
        }
        self.clock = t.min(self.horizon);

        // Rescuers collect what they earned.
        if matches!(self.coin.state.status, Status::Alive) {
            for (_, who) in &team {
                let _ = self.coin.state.claim_bounty(&mut self.coin.bank, self.clock, who);
            }
        }
    }

    fn reschedule(&mut self) {
        self.next_holder = self.next_holder_after(self.clock);
        if self.pacer.is_some() {
            self.next_pacer = Some((self.clock / self.pacer_interval + 1) * self.pacer_interval);
        }
    }

    // ── The life ───────────────────────────────────────────────────────

    fn play(&mut self) {
        self.next_holder = self.next_holder_after(0);
        loop {
            match &self.coin.state.status {
                Status::Deceased { .. } | Status::Arrest(_) => break,
                Status::Morgue { until_block, .. } => {
                    let until = *until_block;
                    if until > self.horizon {
                        break;
                    }
                    self.run_code(until);
                    self.reschedule();
                }
                Status::Alive => {
                    let arrest_at = self.coin.state.arrest_due_block();
                    let holder = self.next_holder.unwrap_or(u64::MAX);
                    let pacer = self.next_pacer.unwrap_or(u64::MAX);
                    let next = holder.min(pacer);
                    if next < arrest_at {
                        if next > self.horizon {
                            break;
                        }
                        self.sample_to(next);
                        if holder <= pacer {
                            self.holder_transfer(next);
                            self.next_holder = self.next_holder_after(next);
                        } else {
                            self.pacer_transfer(next);
                            self.next_pacer = Some(next + self.pacer_interval);
                        }
                    } else {
                        if arrest_at > self.horizon {
                            break;
                        }
                        self.run_code(arrest_at);
                        self.reschedule();
                    }
                }
            }
        }
        self.sample_to(self.horizon);
    }

    fn finish(self, seed: u64) -> RunOutput {
        let end = self.coin.state.preview(&self.coin.bank, self.horizon);
        let tape = &self.coin.bank.sink;
        let codes = tape
            .sheets
            .iter()
            .map(|sheet| {
                let meta = self.meta.iter().find(|m| m.code_no == sheet.code_no);
                CodeRow {
                    code_no: sheet.code_no,
                    arrest_block: sheet.arrest_block,
                    night: meta.is_some_and(|m| m.night),
                    rhythm: sheet.rhythm_at_arrest,
                    responders: meta.map_or(0, |m| m.responders),
                    response_blocks: meta.and_then(|m| m.response_blocks),
                    no_flow_blocks: sheet.no_flow_blocks(),
                    rounds: sheet.rounds.len() as u8,
                    outcome: sheet.outcome,
                    downtime_blocks: sheet.downtime_blocks(),
                    save_blocks: match (sheet.outcome, sheet.first_compression_block) {
                        (Outcome::Rosc, Some(first)) => Some(sheet.end_block - first),
                        _ => None,
                    },
                    bounty: sheet.bounty,
                }
            })
            .collect();
        RunOutput {
            seed,
            days: self.days,
            codes,
            arrests: end.state.totals.arrests,
            saves: end.state.totals.saves,
            deaths: end.state.vitals.deaths,
            first_arrest: tape.first_arrest,
            deceased: match end.state.status {
                Status::Deceased { block } => Some(block),
                _ => None,
            },
            bounty_total: tape.bounty_by.iter().sum(),
            bounty_top: tape.bounty_by.iter().copied().max().unwrap_or(0),
            pacer_fees: self.pacer_fees,
            pacer_beats: self.pacer_beats,
            final_supply: end.supply,
            final_cart: end.state.pools.cart,
            totals: end.state.totals,
        }
    }
}

pub fn outcome_name(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Rosc => "rosc",
        Outcome::Death(DeathReason::NoFlow) => "death_no_flow",
        Outcome::Death(DeathReason::Pause) => "death_pause",
        Outcome::Death(DeathReason::RoundsExhausted) => "death_rounds",
    }
}

pub fn rhythm_name(rhythm: Rhythm) -> &'static str {
    match rhythm {
        Rhythm::Vf => "vf",
        Rhythm::Pea => "pea",
        Rhythm::Asystole => "asystole",
    }
}

/// How long a pair needs for a save, as a function of old damage: the coin
/// arrests with `scars`, two rescuers start at once and press at `blocks_per_30`.
/// Returns, per trial, the blocks from first compression to ROSC and the rounds.
pub fn pair_save(config: &Config, scars: u8, blocks_per_30: f64, airway: bool, seed: u64) -> Option<(u64, u8)> {
    let tape = Tape { sheets: Vec::new(), bounty_by: vec![0; 2], first_arrest: None, deceased: None };
    let mut coin = Coin::launch(config.clone(), 0, 2, tape);
    for who in 0..2u32 {
        coin.state.faucet_claim(&mut coin.bank, 0, &who).unwrap();
    }
    coin.state.vitals.scars = scars;
    let mut rng = Rng::new(seed, hash("pair") ^ scars as u64);
    let hit = (30.0 / blocks_per_30).min(1.0);
    let mut t = coin.state.arrest_due_block();
    loop {
        coin.state.settle(&mut coin.bank, t);
        let Status::Arrest(code) = &coin.state.status else { break };
        let (lead, other) = if code.round % 2 == 1 { (0u32, 1u32) } else { (1, 0) };
        let phase = code.phase;
        let (state, bank) = (&mut coin.state, &mut coin.bank);
        if rng.chance(hit) {
            let _ = match phase {
                Phase::Compressions => state.compress(bank, t, &lead).map(|_| ()),
                Phase::Ventilation { .. } if airway => state.breathe(bank, t, &other).map(|_| ()),
                Phase::Ventilation { .. } => Ok(()),
                Phase::AfterRound { shock_open: true, .. } => state.shock(bank, t, &lead).map(|_| ()),
                Phase::AfterRound { .. } => state.push_epi(bank, t, &lead),
            };
        }
        t += 1;
    }
    let sheet = coin.bank.sink.sheets.first()?;
    match (sheet.outcome, sheet.first_compression_block) {
        (Outcome::Rosc, Some(first)) => Some((sheet.end_block - first, sheet.rounds.len() as u8)),
        _ => None,
    }
}

/// The same question for one rescuer who cannot give breaths.
pub fn lone_save(config: &Config, scars: u8, blocks_per_30: f64, seed: u64) -> Option<(u64, u8)> {
    let tape = Tape { sheets: Vec::new(), bounty_by: vec![0; 1], first_arrest: None, deceased: None };
    let mut coin = Coin::launch(config.clone(), 0, 1, tape);
    coin.state.faucet_claim(&mut coin.bank, 0, &0).unwrap();
    coin.state.vitals.scars = scars;
    let mut rng = Rng::new(seed, hash("lone") ^ scars as u64);
    let hit = (30.0 / blocks_per_30).min(1.0);
    let mut t = coin.state.arrest_due_block();
    loop {
        coin.state.settle(&mut coin.bank, t);
        let Status::Arrest(code) = &coin.state.status else { break };
        let phase = code.phase;
        let (state, bank) = (&mut coin.state, &mut coin.bank);
        if rng.chance(hit) {
            let _ = match phase {
                Phase::Compressions => state.compress(bank, t, &0).map(|_| ()),
                Phase::Ventilation { .. } => Ok(()),
                Phase::AfterRound { shock_open: true, .. } => state.shock(bank, t, &0).map(|_| ()),
                Phase::AfterRound { .. } => state.push_epi(bank, t, &0),
            };
        }
        t += 1;
    }
    let sheet = coin.bank.sink.sheets.first()?;
    match (sheet.outcome, sheet.first_compression_block) {
        (Outcome::Rosc, Some(first)) => Some((sheet.end_block - first, sheet.rounds.len() as u8)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario;

    fn fingerprint(out: &RunOutput) -> Vec<u128> {
        let mut v: Vec<u128> = out.days.iter().flat_map(|d| [d.supply, d.cart, d.status as u128]).collect();
        v.extend(out.codes.iter().flat_map(|c| [c.arrest_block as u128, c.downtime_blocks as u128, c.bounty]));
        v.extend([out.arrests as u128, out.saves as u128, out.deaths as u128, out.bounty_total, out.final_supply]);
        v
    }

    #[test]
    fn the_same_seed_gives_the_same_life() {
        let config = Config::live();
        for sc in scenario::all() {
            let (a, b) = (run(&sc, &config, 7, 60), run(&sc, &config, 7, 60));
            assert_eq!(fingerprint(&a), fingerprint(&b), "{}", sc.name);
        }
        let fading = scenario::all().into_iter().find(|sc| sc.name == "fading").unwrap();
        assert_ne!(fingerprint(&run(&fading, &config, 7, 60)), fingerprint(&run(&fading, &config, 8, 60)));
    }

    #[test]
    fn every_run_keeps_the_books() {
        // The simulator drives the rulebook much harder than the unit tests do.
        let config = Config::live();
        for sc in scenario::all() {
            for seed in 1..=20 {
                let out = run(&sc, &config, seed, 60);
                assert_eq!(out.days.len(), 61);
                assert!(out.days.windows(2).all(|w| w[1].supply <= w[0].supply), "{}: supply rose", sc.name);
                assert_eq!(config.total_supply - out.final_supply, out.totals.burned(), "{}", sc.name);
                assert!(out.deaths <= config.max_deaths);
            }
        }
    }
}
