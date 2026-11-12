//! One test per rule in the spec's Vitals, Running the code, Token economics
//! and Contract spec sections.
mod common;
use codeblue_core::{
    BurnKind, Config, DeathReason, Error, Event, Outcome, Phase, Rhythm, Status, Tap, BPS, ROSC,
};
use common::Rig;

const G: u64 = 5_000;

fn demo() -> Rig {
    Rig::new(Config::demo(), G)
}

fn derived(rig: &Rig) -> codeblue_core::State<u32> {
    rig.coin.state.preview(&rig.coin.bank, rig.now).state
}

// ── Vitals ─────────────────────────────────────────────────────────────

#[test]
fn a_beat_needs_stroke_volume_two_accounts_and_a_rested_sender() {
    let mut rig = demo();
    let c = rig.config().clone();

    let beat = rig.coin.transfer(G + 1, 0, 1, c.min_stroke).unwrap();
    assert!(beat.beat);
    assert_eq!(rig.coin.state.vitals.last_beat_block, G + 1);

    // 1. Below the minimum stroke.
    let small = rig.coin.transfer(G + 2, 2, 3, c.min_stroke - 1).unwrap();
    assert!(!small.beat);
    // 2. Sender and receiver are the same account.
    let own = rig.coin.transfer(G + 3, 4, 4, 10 * c.min_stroke).unwrap();
    assert!(!own.beat);
    // 3. The sender's last beat is too recent.
    let tired = rig.coin.transfer(G + 1 + c.refractory_blocks - 1, 0, 1, c.min_stroke).unwrap();
    assert!(!tired.beat);
    assert_eq!(rig.coin.state.vitals.last_beat_block, G + 1, "none of them reset the clock");
    assert_eq!(rig.coin.state.vitals.electrical_since_beat, 3);

    let rested = rig.coin.transfer(G + 1 + c.refractory_blocks, 0, 1, c.min_stroke).unwrap();
    assert!(rested.beat);
    assert_eq!(rig.coin.state.vitals.beats.len(), 2);
    assert_eq!(rig.coin.state.vitals.electrical_since_beat, 0);
    rig.audit();
}

#[test]
fn a_transfer_that_is_not_a_beat_still_goes_through() {
    let mut rig = demo();
    let before = rig.coin.bank.balances[3];
    rig.coin.transfer(G + 2, 2, 3, ROSC / 2).unwrap();
    assert_eq!(rig.coin.bank.balances[3], before + ROSC / 2);
}

#[test]
fn arrest_starts_on_the_first_block_past_the_threshold() {
    let mut rig = demo();
    let last_alive = G + rig.config().arrest_blocks;
    // A dust transfer on the last living block goes through and is no beat.
    assert!(!rig.coin.transfer(last_alive, 0, 1, 1).unwrap().beat);
    // Nothing called anything, and the next block has no pulse.
    assert_eq!(rig.coin.transfer(last_alive + 1, 0, 1, ROSC), Err(Error::NoPulse));
    assert_eq!(rig.coin.state.arrest_due_block(), last_alive + 1);
    assert!(matches!(rig.coin.state.status, Status::Arrest(_)), "the refused call computed it");
}

#[test]
fn the_sender_pays_the_fees_on_top_and_the_receiver_gets_the_amount() {
    let mut rig = demo();
    let amount = 1_000 * ROSC;
    let (sender, receiver) = (rig.coin.bank.balances[0], rig.coin.bank.balances[1]);
    let (supply, cart) = (rig.coin.bank.supply, rig.coin.state.pools.cart);

    let out = rig.coin.transfer(G + 1, 0, 1, amount).unwrap();
    assert_eq!(out.oxygen_burn, 5 * ROSC);
    assert_eq!(out.premium, 5 * ROSC);
    assert_eq!(rig.coin.bank.balances[1], receiver + amount);
    assert_eq!(rig.coin.bank.balances[0], sender - amount - 10 * ROSC);
    assert_eq!(rig.coin.bank.supply, supply - 5 * ROSC);
    assert_eq!(rig.coin.state.pools.cart, cart + 5 * ROSC);
    rig.audit();

    let all = rig.coin.bank.balances[0];
    assert!(matches!(rig.coin.transfer(G + 2, 0, 1, all), Err(Error::InsufficientBalance { .. })));
}

#[test]
fn a_transfer_to_the_contract_fills_the_crash_cart() {
    let mut rig = demo();
    let cart = rig.coin.state.pools.cart;
    rig.coin.donate(G + 1, 0, 1_000 * ROSC).unwrap();
    assert_eq!(rig.coin.state.pools.cart, cart + 1_005 * ROSC);
    rig.audit();
}

#[test]
fn the_monitor_rings_record_beats_and_electrical_activity() {
    let mut rig = demo();
    let bucket = rig.config().bucket_blocks;
    rig.coin.transfer(G + 1, 0, 1, ROSC).unwrap();
    rig.coin.transfer(G + 2, 2, 3, 1).unwrap();
    rig.coin.transfer(G + 1 + bucket, 4, 5, ROSC).unwrap();
    let monitor = rig.coin.state.monitor(&rig.coin.bank, G + 2 + bucket);
    assert_eq!(monitor.beats.len(), 2);
    assert_eq!(monitor.beats[0].block, G + 1);
    assert_eq!(monitor.beats[0].tag, 0);
    assert_eq!(monitor.buckets.len(), 2);
    assert_eq!((monitor.buckets[0].beats, monitor.buckets[0].electrical), (1, 1));
    assert_eq!((monitor.buckets[1].beats, monitor.buckets[1].electrical), (1, 0));

    // The beat ring keeps the newest 64.
    let mut now = G + 100;
    for i in 0..200u64 {
        now += 1;
        rig.coin.transfer(now, (i % 12) as u32, ((i + 1) % 12) as u32, ROSC).unwrap();
    }
    assert_eq!(rig.coin.state.vitals.beats.len(), 64);
}

// ── Rhythms ────────────────────────────────────────────────────────────

fn busy_final_bucket(rig: &mut Rig) {
    for sender in 0..rig.config().sudden_beats {
        assert!(rig.coin.transfer(G + 1 + sender as u64, sender, sender + 1, ROSC).unwrap().beat);
    }
}

#[test]
fn a_busy_final_bucket_then_silence_is_vf() {
    let mut rig = demo();
    busy_final_bucket(&mut rig);
    rig.to_arrest();
    assert_eq!(rig.coin.state.call_code(&mut rig.coin.bank, rig.now, &0), Ok(Rhythm::Vf));
}

#[test]
fn transfers_without_a_pulse_during_the_silence_are_pea() {
    let mut rig = demo();
    rig.coin.transfer(G + 1, 0, 1, ROSC).unwrap();
    for i in 0..rig.config().pea_min as u64 {
        rig.coin.transfer(G + 10 + i, 2, 3, 1).unwrap();
    }
    rig.to_arrest();
    assert_eq!(rig.coin.state.call_code(&mut rig.coin.bank, rig.now, &0), Ok(Rhythm::Pea));
}

#[test]
fn any_other_arrest_is_asystole() {
    let mut rig = demo();
    rig.coin.transfer(G + 1, 0, 1, ROSC).unwrap();
    rig.coin.transfer(G + 10, 2, 3, 1).unwrap();
    rig.to_arrest();
    assert_eq!(rig.coin.state.call_code(&mut rig.coin.bank, rig.now, &0), Ok(Rhythm::Asystole));
}

#[test]
fn vf_that_reaches_round_four_degrades_to_asystole() {
    // Slow hands-only CPR: three rounds pass without ROSC.
    let mut rig = demo();
    busy_final_bucket(&mut rig);
    rig.to_arrest();
    for round in 1..=3u8 {
        assert_eq!(derived(&rig).status_rhythm(), Some(Rhythm::Vf), "round {round}");
        for _ in 0..rig.config().cycles_per_round {
            rig.cycle(0, 0, 90, 0);
        }
        // Let the rhythm check lapse unused.
        rig.now += rig.config().vent_window_blocks + 1;
    }
    let state = derived(&rig);
    let Status::Arrest(code) = &state.status else { panic!("still in the code") };
    assert_eq!((code.round, code.rhythm), (4, Rhythm::Asystole));
    assert_eq!(rig.coin.state.shock(&mut rig.coin.bank, rig.now, &0), Err(Error::NotShockable));
}

trait RhythmOf {
    fn status_rhythm(&self) -> Option<Rhythm>;
}

impl RhythmOf for codeblue_core::State<u32> {
    fn status_rhythm(&self) -> Option<Rhythm> {
        match &self.status {
            Status::Arrest(code) => Some(code.rhythm),
            _ => None,
        }
    }
}

// ── Compressions and breaths ───────────────────────────────────────────

#[test]
fn compress_while_alive_is_refused() {
    let mut rig = demo();
    assert_eq!(rig.coin.state.compress(&mut rig.coin.bank, G + 1, &0), Err(Error::HasPulse));
}

#[test]
fn only_one_compression_counts_per_block_whoever_sends_it() {
    let mut rig = demo();
    rig.to_arrest();
    assert_eq!(rig.compress(0), Tap::Counted);
    let before = rig.coin.state.clone();
    assert_eq!(rig.compress(1), Tap::AlreadyThisBlock);
    assert_eq!(rig.compress(0), Tap::AlreadyThisBlock);
    assert_eq!(rig.coin.state, before, "a second call in the block changes nothing");
    rig.now += 1;
    assert_eq!(rig.compress(1), Tap::Counted);
    assert_eq!(rig.code().cycle_compressions, 2);
}

#[test]
fn the_first_compression_fixes_the_target_and_burns_for_the_delay() {
    let mut rig = demo();
    rig.to_arrest();
    rig.now += 500;
    let cart = rig.coin.state.pools.cart;
    assert_eq!(rig.compress(0), Tap::Counted);
    // target = BASE_TARGET + min(NOFLOW_TARGET_CAP, no_flow_blocks / 10) = 200 + 50
    assert_eq!(rig.code().target, 250 * 100);
    // NOFLOW_BURN_BPS of the cart per 100 blocks: 1 bps x 5.
    let burn = cart * 5 / BPS as u128;
    assert_eq!(rig.code().noflow_burn, burn);
    assert_eq!(rig.coin.state.pools.cart, cart - burn);
    // Later compressions move neither.
    rig.now += 1;
    rig.compress(0);
    assert_eq!(rig.code().target, 250 * 100);
    assert_eq!(rig.coin.state.pools.cart, cart - burn);
    rig.audit();
}

#[test]
fn the_no_flow_part_of_the_target_is_capped() {
    let rig = Rig::new(Config::live(), G);
    assert_eq!(rig.coin.state.target_for(0), 200 * 100);
    assert_eq!(rig.coin.state.target_for(1_999), 200 * 100 + 19_990);
    assert_eq!(rig.coin.state.target_for(2_000), 400 * 100);
    assert_eq!(rig.coin.state.target_for(12_000), 400 * 100);
}

#[test]
fn thirty_compressions_open_the_airway_and_close_the_chest() {
    let mut rig = demo();
    rig.to_arrest();
    for _ in 0..30 {
        assert_eq!(rig.compress(0), Tap::Counted);
        rig.now += 1;
    }
    assert!(matches!(rig.code().phase, Phase::Ventilation { .. }));
    assert_eq!(rig.compress(0), Tap::HandsOff, "a compression during the window is refused");
    assert_eq!(rig.compress(1), Tap::HandsOff);
    assert_eq!(rig.code().cycle_compressions, 30);
}

#[test]
fn breaths_come_from_someone_else_once_per_block_up_to_two() {
    let mut rig = demo();
    rig.to_arrest();
    assert_eq!(rig.coin.state.breathe(&mut rig.coin.bank, rig.now, &1), Err(Error::AirwayClosed));
    for _ in 0..30 {
        rig.compress(0);
        rig.now += 1;
    }
    assert_eq!(rig.coin.state.breathe(&mut rig.coin.bank, rig.now, &0), Err(Error::OnTheChest));
    assert_eq!(rig.breathe(1), Tap::Counted);
    assert_eq!(rig.breathe(2), Tap::AlreadyThisBlock);
    assert_eq!(rig.code().cycle_breaths, 1);
    rig.now += 1;
    assert_eq!(rig.breathe(2), Tap::Counted);
    // Two breaths close the cycle at once: quality 1.00, next cycle open.
    assert_eq!(rig.code().cycle_quality_bps, vec![10_000]);
    assert_eq!((rig.code().cycle, rig.code().phase), (2, Phase::Compressions));
    assert_eq!(rig.compress(0), Tap::Counted, "same block as the second breath");
}

#[test]
fn the_lead_compressor_is_whoever_did_the_most() {
    let mut rig = demo();
    rig.to_arrest();
    for i in 0..30 {
        rig.compress(if i < 16 { 3 } else { 4 });
        rig.now += 1;
    }
    assert_eq!(rig.code().cycle_lead, Some(3));
    assert_eq!(rig.coin.state.breathe(&mut rig.coin.bank, rig.now, &3), Err(Error::OnTheChest));
    assert_eq!(rig.breathe(4), Tap::Counted, "the other compressor may take the airway");
}

#[test]
fn a_lapsed_window_starts_a_new_cycle_with_the_breaths_that_arrived() {
    let mut rig = demo();
    rig.to_arrest();
    for _ in 0..30 {
        rig.compress(0);
        rig.now += 1;
    }
    let opened = rig.now - 1;
    assert_eq!(rig.breathe(1), Tap::Counted);
    // Last block of the window: still hands off.
    rig.now = opened + rig.config().vent_window_blocks;
    assert_eq!(rig.compress(0), Tap::HandsOff);
    // First block after it: the call advances the phase, then counts.
    rig.now += 1;
    assert_eq!(rig.compress(0), Tap::Counted);
    // One breath: 0.70 + 0.15.
    assert_eq!(rig.code().cycle_quality_bps, vec![8_500]);
    assert_eq!((rig.code().cycle, rig.code().cycle_compressions), (2, 1));
}

#[test]
fn hands_only_cpr_scores_seventy_percent_and_slow_cycles_score_lower() {
    let mut rig = demo();
    rig.to_arrest();
    rig.cycle(0, 0, 30, 0);
    rig.compress(0);
    assert_eq!(rig.code().cycle_quality_bps, vec![7_000]);

    let mut rig = demo();
    rig.to_arrest();
    rig.cycle(0, 1, 45, 2);
    // compression_part = 30 / 45
    assert_eq!(rig.code().cycle_quality_bps, vec![6_666]);
}

#[test]
fn the_same_lead_two_rounds_running_scores_eighty_percent() {
    let mut rig = Rig::new(Config { base_target: 1_000, ..Config::demo() }, G);
    rig.to_arrest();
    rig.round(0, 1, 30, 2);
    rig.round(0, 1, 30, 2);
    rig.round(1, 0, 30, 2);
    let state = derived(&rig);
    let Status::Arrest(code) = &state.status else { panic!() };
    let quality: Vec<(u16, bool)> = code.rounds.iter().map(|r| (r.quality_bps, r.fatigued)).collect();
    assert_eq!(quality, vec![(10_000, false), (8_000, true), (10_000, false)]);
    // 100 + 80 + epinephrine after round 2 + 100
    assert_eq!(code.perfusion, 33_000);
}

// ── After the round ────────────────────────────────────────────────────

#[test]
fn a_shock_adds_perfusion_when_the_round_was_good_enough() {
    let mut rig = demo();
    busy_final_bucket(&mut rig);
    rig.to_arrest();
    for _ in 0..rig.config().cycles_per_round {
        rig.cycle(0, 1, 30, 2);
    }
    assert!(matches!(rig.code().phase, Phase::AfterRound { shock_open: true, epi_open: false, .. }));
    assert_eq!(rig.compress(0), Tap::HandsOff, "hands off during the rhythm check");
    assert_eq!(rig.coin.state.shock(&mut rig.coin.bank, rig.now, &2), Ok(true));
    // 100 from the round + 150 from the shock clears the 200 target.
    assert_eq!(rig.coin.state.status, Status::Alive);
    let sheet = last_sheet(&rig);
    assert_eq!(sheet.perfusion, 25_000);
    assert_eq!(sheet.rounds[0].shock, Some(true));
    assert!(sheet.units.contains(&(2, 15)), "a successful shock earns 15 units");
}

#[test]
fn a_shock_after_a_poor_round_adds_nothing_and_uses_the_rounds_shock() {
    let mut rig = demo();
    busy_final_bucket(&mut rig);
    rig.to_arrest();
    for _ in 0..rig.config().cycles_per_round {
        rig.cycle(0, 0, 60, 0);
    }
    let state = derived(&rig);
    let Status::Arrest(code) = &state.status else { panic!() };
    assert_eq!(code.round_quality_bps, 3_500);
    assert_eq!(rig.coin.state.shock(&mut rig.coin.bank, rig.now, &1), Ok(false));
    assert_eq!(rig.code().perfusion, 3_500);
    assert!(!rig.code().units.iter().any(|(who, _)| *who == 1), "no units for a shock that failed");
    // The window closed with the shock: round 2 is open.
    assert_eq!((rig.code().round, rig.code().phase), (2, Phase::Compressions));
    assert_eq!(rig.coin.state.shock(&mut rig.coin.bank, rig.now, &1), Err(Error::ShockNotNow));
}

#[test]
fn a_flatline_cannot_be_shocked() {
    let mut rig = demo();
    rig.to_arrest();
    assert_eq!(rig.coin.state.shock(&mut rig.coin.bank, rig.now, &0), Err(Error::NotShockable));
    for _ in 0..rig.config().cycles_per_round {
        rig.cycle(0, 0, 60, 0);
    }
    assert_eq!(rig.coin.state.shock(&mut rig.coin.bank, rig.now, &0), Err(Error::NotShockable));
}

#[test]
fn epinephrine_opens_after_round_two_and_every_second_round() {
    let mut rig = Rig::new(Config { base_target: 5_000, ..Config::demo() }, G);
    rig.to_arrest();
    let dose = rig.config().epi_dose;
    let mut doses = Vec::new();
    for round in 1..=rig.config().max_rounds {
        for _ in 0..rig.config().cycles_per_round {
            rig.cycle(0, 1, 30, 2);
        }
        let burned = rig.coin.state.totals.burned_epi;
        match rig.coin.state.push_epi(&mut rig.coin.bank, rig.now, &1) {
            Ok(()) => {
                doses.push(round);
                assert_eq!(rig.coin.state.totals.burned_epi, burned + dose, "one dose burned from the cart");
                assert_eq!(
                    rig.coin.state.push_epi(&mut rig.coin.bank, rig.now, &1),
                    Err(if round == rig.config().max_rounds {
                        // The last round is over and so is the code.
                        Error::InMorgue { until_block: rig.now + rig.config().lazarus_blocks }
                    } else {
                        Error::EpiNotYet
                    }),
                    "one dose per window"
                );
            }
            Err(Error::EpiNotYet) => {}
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(doses, vec![2, 4, 6, 8, 10], "at most five a code");
    assert_eq!(rig.coin.state.totals.burned_epi, 5 * dose);
    rig.audit();
}

// ── ROSC ───────────────────────────────────────────────────────────────

fn last_sheet(rig: &Rig) -> codeblue_core::CodeSheet<u32> {
    rig.coin
        .bank
        .sink
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::Rosc { sheet, .. } | Event::TimeOfDeath { sheet, .. } => Some(sheet.clone()),
            _ => None,
        })
        .expect("a code has ended")
}

#[test]
fn rosc_restores_the_pulse_scars_the_coin_and_pays_by_work_units() {
    let mut rig = demo();
    rig.to_arrest();
    assert_eq!(rig.coin.state.call_code(&mut rig.coin.bank, rig.now, &5), Ok(Rhythm::Asystole));
    assert_eq!(
        rig.coin.state.call_code(&mut rig.coin.bank, rig.now, &6),
        Err(Error::CodeAlreadyCalled)
    );
    let cart = rig.coin.state.pools.cart;
    rig.round(0, 1, 30, 2);
    rig.round(1, 0, 30, 2);
    assert_eq!(rig.coin.state.status, Status::Alive);

    let rosc_block = rig.now - 1;
    let v = &rig.coin.state.vitals;
    assert_eq!((v.last_beat_block, v.alive_since_block, v.scars), (rosc_block, rosc_block, 1));
    assert_eq!(rig.coin.state.ejection_fraction(), 55, "EF falls by EF_STEP");

    // 4 cycles: 120 compressions and 8 breaths, split evenly; 10 for the call.
    let sheet = last_sheet(&rig);
    assert_eq!(sheet.outcome, Outcome::Rosc);
    assert_eq!(sheet.units, vec![(5, 10), (0, 60 + 20), (1, 20 + 60)]);
    // bounty = cart * BOUNTY_BPS * EF / EF_START, with EF still at 60.
    let pool = cart * 200 / BPS as u128;
    let shares: Vec<(u32, u128)> = sheet.shares.iter().map(|(who, s)| (*who, s.0)).collect();
    assert_eq!(shares, vec![(5, pool * 10 / 170), (0, pool * 80 / 170), (1, pool * 80 / 170)]);
    let paid: u128 = shares.iter().map(|(_, s)| s).sum();
    assert!(paid <= pool && pool - paid < 3, "rounding dust stays in the cart");
    assert_eq!(rig.coin.state.pools.cart, cart - paid);
    rig.audit();

    // Claims pay in full, carry no fee and are not beats.
    let (before, supply) = (rig.coin.bank.balances[0], rig.coin.bank.supply);
    assert_eq!(rig.coin.state.claim_bounty(&mut rig.coin.bank, rig.now, &0), Ok(pool * 80 / 170));
    assert_eq!(rig.coin.bank.balances[0], before + pool * 80 / 170);
    assert_eq!(rig.coin.bank.supply, supply);
    assert_eq!(rig.coin.state.vitals.last_beat_block, rosc_block);
    assert_eq!(rig.coin.state.claim_bounty(&mut rig.coin.bank, rig.now, &0), Err(Error::NothingToClaim));
    rig.audit();

    // Transfers work again.
    assert!(rig.coin.transfer(rig.now, 0, 1, ROSC).unwrap().beat);
}

#[test]
fn a_rosc_that_no_call_wrote_still_restores_the_pulse() {
    // A lone rescuer cannot give breaths, so the last cycle closes when its
    // window lapses. Nobody has to call anything for the coin to be alive.
    let mut rig = demo();
    rig.to_arrest();
    let mut last_compression = 0;
    for round in 1..=3 {
        for cycle in 0..rig.config().cycles_per_round {
            let start = rig.now;
            for i in 0..30 {
                rig.now = start + i;
                rig.compress(0);
            }
            last_compression = rig.now;
            rig.now += rig.config().vent_window_blocks + 1;
            if round == 3 && cycle + 1 == rig.config().cycles_per_round {
                break;
            }
        }
        if round == 2 {
            rig.coin.state.push_epi(&mut rig.coin.bank, rig.now, &0).unwrap();
        }
    }
    // 70 + 56 + 50 + 56 = 232 against 200. Stored state is still the arrest.
    assert!(rig.in_arrest());
    let lapse = last_compression + rig.config().vent_window_blocks + 1;
    assert_eq!(rig.coin.transfer(lapse - 1, 1, 2, ROSC), Err(Error::NoPulse));
    assert!(rig.in_arrest(), "one block early the window is still open");
    assert!(rig.coin.transfer(lapse + 5, 1, 2, ROSC).unwrap().beat);
    let sheet = last_sheet(&rig);
    assert_eq!((sheet.outcome, sheet.end_block, sheet.perfusion), (Outcome::Rosc, lapse, 23_200));
    assert_eq!(rig.coin.state.vitals.alive_since_block, lapse, "stamped when it became true");
    rig.audit();
}

#[test]
fn the_bounty_shrinks_with_every_scar() {
    // "What a serial rescuer can take": a perfect pair saves the coin every
    // time, compressing from the arrest block so nothing else is burned.
    let mut rig = Rig::new(Config::live(), G);
    let m = 1_000_000 * ROSC;
    let mut seen = Vec::new();
    for arrest in 1..=10u32 {
        rig.to_arrest();
        let mut round = 0;
        loop {
            round += 1;
            let (lead, airway) = if round % 2 == 1 { (0, 1) } else { (1, 0) };
            for _ in 0..rig.config().cycles_per_round {
                rig.cycle(lead, airway, 30, 2);
            }
            match derived(&rig).status {
                Status::Alive => break,
                // Leave the epinephrine in the cart: let the rhythm check lapse.
                Status::Arrest(_) => rig.now += rig.config().vent_window_blocks + 1,
                other => panic!("{other:?}"),
            }
        }
        rig.coin.state.settle(&mut rig.coin.bank, rig.now);
        let sheet = last_sheet(&rig);
        seen.push((arrest, sheet.bounty, rig.coin.state.pools.cart + dust(&rig)));
    }
    let tenths = |x: u128| (x + m / 20) / (m / 10);
    // Arrest, bounty and cart in tenths of a million ROSC, from the spec's table.
    for (arrest, bounty, cart) in [(1u32, 200u128, 9_800u128), (5, 124, 9_194), (10, 59, 8_802)] {
        let row = seen[arrest as usize - 1];
        assert_eq!((tenths(row.1), tenths(row.2)), (bounty, cart), "arrest {arrest}");
    }
    rig.audit();
}

/// The table counts the cart net of the bounty; premiums and dust are zero here.
fn dust(_: &Rig) -> u128 {
    0
}

// ── Time of death ──────────────────────────────────────────────────────

#[test]
fn death_for_no_flow_cannot_be_called_early() {
    let mut rig = demo();
    rig.to_arrest();
    let arrest = rig.now;
    let limit = arrest + rig.config().no_flow_max_blocks;
    assert_eq!(rig.coin.state.call_time_of_death(&mut rig.coin.bank, limit), Err(Error::StillInCode));
    // A compression on the last block is still in time.
    let mut saved = Rig::new(Config::demo(), G);
    saved.now = limit;
    assert_eq!(saved.compress(0), Tap::Counted);

    let cart = rig.coin.state.pools.cart;
    assert_eq!(rig.coin.state.call_time_of_death(&mut rig.coin.bank, limit + 1), Ok(1));
    let v = &rig.coin.state.vitals;
    assert_eq!((v.deaths, v.scars), (1, 2), "one death and two scars");
    let burn = cart * 1_000 / BPS as u128;
    assert_eq!(rig.coin.state.pools.cart, cart - burn, "DEATH_BURN_BPS of the cart");
    let lazarus = rig.config().lazarus_blocks;
    assert_eq!(
        rig.coin.state.status,
        Status::Morgue { since_block: limit + 1, until_block: limit + 1 + lazarus }
    );
    let sheet = last_sheet(&rig);
    assert_eq!(sheet.outcome, Outcome::Death(DeathReason::NoFlow));
    rig.audit();

    // Nothing to do in the morgue, and no pulse either.
    let until = limit + 1 + lazarus;
    assert_eq!(
        rig.coin.state.compress(&mut rig.coin.bank, until - 1, &0),
        Err(Error::InMorgue { until_block: until })
    );
    assert_eq!(rig.coin.transfer(until - 1, 0, 1, ROSC), Err(Error::NoPulse));
    assert_eq!(
        rig.coin.state.call_time_of_death(&mut rig.coin.bank, until - 1),
        Err(Error::InMorgue { until_block: until })
    );
    // After LAZARUS_BLOCKS a new code opens, with asystole as the rhythm.
    assert_eq!(rig.coin.state.call_code(&mut rig.coin.bank, until, &0), Ok(Rhythm::Asystole));
    assert_eq!(rig.code().arrest_block, until);
}

#[test]
fn death_for_a_long_pause_cannot_be_called_early() {
    let mut rig = demo();
    rig.to_arrest();
    rig.compress(0);
    let limit = rig.now + rig.config().max_pause_blocks;
    assert_eq!(rig.coin.state.call_time_of_death(&mut rig.coin.bank, limit), Err(Error::StillInCode));
    let mut resumed = Rig::new(Config::demo(), G);
    resumed.to_arrest();
    resumed.compress(0);
    resumed.now = limit;
    assert_eq!(resumed.compress(0), Tap::Counted, "the last block of the pause is in time");

    assert_eq!(rig.coin.state.call_time_of_death(&mut rig.coin.bank, limit + 1), Ok(1));
    assert_eq!(last_sheet(&rig).outcome, Outcome::Death(DeathReason::Pause));
    assert_eq!(last_sheet(&rig).end_block, limit + 1);
}

#[test]
fn death_after_the_last_round_cannot_be_called_early() {
    let mut rig = Rig::new(Config { base_target: 5_000, max_rounds: 3, ..Config::demo() }, G);
    rig.to_arrest();
    for round in 1..=3 {
        assert_eq!(
            rig.coin.state.call_time_of_death(&mut rig.coin.bank, rig.now),
            Err(Error::StillInCode),
            "round {round}"
        );
        rig.round(0, 1, 30, 2);
    }
    // Round 3 ended below target and offered nothing, so the code is over.
    let sheet = last_sheet(&rig);
    assert_eq!(sheet.outcome, Outcome::Death(DeathReason::RoundsExhausted));
    assert_eq!(sheet.rounds.len(), 3);
    assert!(sheet.perfusion < sheet.target);
    assert!(matches!(rig.coin.state.status, Status::Morgue { .. }));
}

#[test]
fn time_of_death_while_alive_is_refused() {
    let mut rig = demo();
    assert_eq!(rig.coin.state.call_time_of_death(&mut rig.coin.bank, G + 5), Err(Error::NoDeathToCall));
}

#[test]
fn three_unanswered_codes_end_in_deceased_and_nothing_works_after() {
    let mut rig = demo();
    let c = rig.config().clone();
    rig.coin.transfer(G + 1, 0, 1, 1_000 * ROSC).unwrap();
    let first_arrest = G + 1 + c.arrest_blocks + 1;
    let death_1 = first_arrest + c.no_flow_max_blocks + 1;
    let death_2 = death_1 + c.lazarus_blocks + c.no_flow_max_blocks + 1;
    let death_3 = death_2 + c.lazarus_blocks + c.no_flow_max_blocks + 1;

    // One call, long after, writes all of it with the blocks it happened at.
    assert_eq!(rig.coin.state.call_time_of_death(&mut rig.coin.bank, death_3 + 10_000), Ok(3));
    assert_eq!(rig.coin.state.status, Status::Deceased { block: death_3 });
    let deaths: Vec<u64> = rig
        .coin
        .bank
        .sink
        .iter()
        .filter_map(|e| match e {
            Event::TimeOfDeath { block, .. } => Some(*block),
            _ => None,
        })
        .collect();
    assert_eq!(deaths, vec![death_1, death_2, death_3]);
    assert_eq!(rig.coin.state.vitals.scars, 6);

    // The estate: the whole remaining cart is burned.
    assert_eq!(rig.coin.state.pools.cart, 0);
    assert!(rig.coin.bank.sink.iter().any(|e| matches!(e, Event::Burn { kind: BurnKind::Estate, .. })));
    rig.audit();

    let now = death_3 + 20_000;
    let dead = Error::Deceased { block: death_3 };
    assert_eq!(dead.to_string(), format!("Time of death was called at block {death_3}."));
    let (state, bank) = (&mut rig.coin.state, &mut rig.coin.bank);
    assert_eq!(state.compress(bank, now, &0), Err(dead.clone()));
    assert_eq!(state.breathe(bank, now, &0), Err(dead.clone()));
    assert_eq!(state.shock(bank, now, &0), Err(dead.clone()));
    assert_eq!(state.push_epi(bank, now, &0), Err(dead.clone()));
    assert_eq!(state.call_code(bank, now, &0), Err(dead.clone()));
    assert_eq!(state.call_time_of_death(bank, now), Err(dead.clone()));
    assert_eq!(state.claim_bounty(bank, now, &0), Err(dead.clone()));
    assert_eq!(state.faucet_claim(bank, now, &0), Err(dead.clone()));
    assert_eq!(rig.coin.transfer(now, 0, 1, ROSC), Err(dead));
}

#[test]
fn post_mortem_transfers_move_tokens_and_nothing_else() {
    let mut rig = Rig::new(Config { post_mortem_transfers: true, ..Config::demo() }, G);
    rig.coin.state.call_time_of_death(&mut rig.coin.bank, G + 1_000_000).unwrap();
    assert!(matches!(rig.coin.state.status, Status::Deceased { .. }));
    let (supply, before) = (rig.coin.bank.supply, rig.coin.state.clone());
    let out = rig.coin.transfer(G + 1_000_001, 0, 1, 1_000 * ROSC).unwrap();
    assert_eq!((out.oxygen_burn, out.premium, out.beat), (0, 0, false));
    assert_eq!(rig.coin.bank.supply, supply);
    assert_eq!(rig.coin.state, before, "every mechanic stays off");
    assert!(matches!(
        rig.coin.state.compress(&mut rig.coin.bank, G + 1_000_002, &0),
        Err(Error::Deceased { .. })
    ));
}

// ── Rehab ──────────────────────────────────────────────────────────────

#[test]
fn unbroken_life_heals_one_scar_per_rehab_period() {
    let mut rig = demo();
    rig.to_arrest();
    rig.round(0, 1, 30, 2);
    rig.round(1, 0, 30, 2);
    let alive_since = rig.coin.state.vitals.alive_since_block;
    rig.coin.state.vitals.scars = 3;
    let rehab = rig.config().rehab_blocks;

    // Keep a pulse going for two and a half rehab periods.
    let mut now = alive_since;
    while now < alive_since + rehab * 5 / 2 {
        now += rig.config().arrest_blocks / 2;
        rig.coin.transfer(now, (now % 7) as u32, 8, ROSC).unwrap();
    }
    assert_eq!(rig.coin.state.vitals.scars, 1);
    assert_eq!(rig.coin.state.ejection_fraction(), 55, "rehab heals the bounty as well");
    let healed: Vec<u64> = rig
        .coin
        .bank
        .sink
        .iter()
        .filter_map(|e| match e {
            Event::Rehab { block, .. } => Some(*block),
            _ => None,
        })
        .collect();
    assert_eq!(healed, vec![alive_since + rehab, alive_since + 2 * rehab]);

    // An arrest breaks the stretch: the count starts again at the next ROSC.
    rig.now = now;
    rig.to_arrest();
    // One scar left: target 250, reached by two perfect rounds and a dose.
    rig.round(0, 1, 30, 2);
    rig.round(1, 0, 30, 2);
    assert_eq!(rig.coin.state.status, Status::Alive);
    assert_eq!(rig.coin.state.vitals.scars, 2);
    assert_eq!(rig.coin.state.vitals.rehab_anchor_block, rig.coin.state.vitals.alive_since_block);
}

// ── Access ─────────────────────────────────────────────────────────────

#[test]
fn rescuers_hold_the_minimum_balance() {
    let mut coin = codeblue_core::mem::Coin::launch(Config::demo(), G, 4, Vec::new());
    coin.state.faucet_claim(&mut coin.bank, G, &0).unwrap();
    let min_balance = coin.state.config.min_rescuer_balance;
    coin.transfer(G + 1, 0, 1, min_balance - 1).unwrap();
    let now = coin.state.arrest_due_block();
    let refused = Err(Error::NotARescuer { min_balance });
    assert_eq!(coin.state.compress(&mut coin.bank, now, &1), refused);
    assert_eq!(coin.state.call_code(&mut coin.bank, now, &1).map(|_| ()), refused.clone().map(|_: Tap| ()));
    assert_eq!(coin.state.compress(&mut coin.bank, now, &0), Ok(Tap::Counted));
    // The faucet still pays during an arrest, so a newcomer can scrub in.
    coin.state.faucet_claim(&mut coin.bank, now, &2).unwrap();
    assert_eq!(coin.state.compress(&mut coin.bank, now + 1, &2), Ok(Tap::Counted));
}

#[test]
fn the_faucet_pays_once_per_account_until_it_is_empty() {
    let config = Config { faucet_amount: 600_000_000 * ROSC, ..Config::demo() };
    let mut coin = codeblue_core::mem::Coin::launch(config, G, 4, Vec::new());
    assert_eq!(coin.state.faucet_claim(&mut coin.bank, G, &0), Ok(600_000_000 * ROSC));
    assert_eq!(coin.state.faucet_claim(&mut coin.bank, G, &0), Err(Error::FaucetAlreadyClaimed));
    assert_eq!(coin.state.faucet_claim(&mut coin.bank, G, &1), Ok(600_000_000 * ROSC));
    assert_eq!(coin.state.faucet_claim(&mut coin.bank, G, &2), Ok(300_000_000 * ROSC), "what is left");
    assert_eq!(coin.state.faucet_claim(&mut coin.bank, G, &3), Err(Error::FaucetEmpty));
    assert_eq!(coin.state.pools.faucet, 0);
    assert_eq!(coin.state.vitals.last_beat_block, G, "faucet payments are not beats");
}

#[test]
fn the_supply_splits_into_the_cart_and_the_faucet() {
    let c = Config::live();
    assert_eq!(c.total_supply, 2_500_000_000 * ROSC);
    assert_eq!(c.cart_supply(), 1_000_000_000 * ROSC);
    assert_eq!(c.faucet_supply(), 1_500_000_000 * ROSC);
    assert_eq!(Config::demo().validate(), Ok(()));
    assert_eq!(Config::live().validate(), Ok(()));
}

// ── Messages ───────────────────────────────────────────────────────────

#[test]
fn the_messages_match_the_spec_character_for_character() {
    let table = [
        (Error::NoPulse, "No pulse. Tokens cannot circulate without circulation. Start compressions."),
        (Error::HasPulse, "Patient has a pulse. Stop compressions."),
        (Error::NotShockable, "Not a shockable rhythm. Resume compressions."),
        (Error::OnTheChest, "You are on the chest. Someone else takes the airway."),
        (Error::EpiNotYet, "Epinephrine is every 3 to 5 minutes. Not yet."),
        (Error::StillInCode, "Still in the code. Keep going."),
        (Error::Deceased { block: 123_456_789 }, "Time of death was called at block 123456789."),
    ];
    for (error, message) in table {
        assert_eq!(error.to_string(), message);
    }
}
