//! The spec's "How the defaults play out" table, row by row.
mod common;
use codeblue_core::{Config, DeathReason, Event, Outcome, Rhythm, Status, ROSC};
use common::Rig;

struct Row {
    rescuers: u8,
    /// Blocks taken by the 30 compressions of each cycle.
    span: u64,
    scars: u8,
    no_flow: u64,
    vf: bool,
    cycle_quality_bps: u16,
    target: u32,
    /// `None` when ROSC is out of reach inside the code.
    rounds: Option<u8>,
}

fn play(config: Config, row: &Row) {
    let genesis = 1_000_000;
    let mut rig = Rig::new(config, genesis);
    if row.vf {
        // A busy final bucket, then silence.
        let need = rig.config().sudden_beats;
        for sender in 0..need {
            rig.now = genesis + 10 + sender as u64;
            let out = rig.coin.transfer(rig.now, sender, sender + 1, 500 * ROSC).unwrap();
            assert!(out.beat);
        }
    }
    rig.coin.state.vitals.scars = row.scars;
    rig.to_arrest();
    rig.now += row.no_flow;

    let (a, b) = (0u32, if row.rescuers > 1 { 1u32 } else { 0u32 });
    let breaths = if row.rescuers > 1 { 2 } else { 0 };
    let mut round = 0u8;
    loop {
        round += 1;
        // Two rescuers swap the chest every round. One rescuer cannot.
        let (lead, airway) = if round % 2 == 1 { (a, b) } else { (b, a) };
        rig.round(lead, airway, row.span, breaths);
        // A lapsed window may have ended the code without a call writing it.
        if !matches!(rig.coin.state.preview(&rig.coin.bank, rig.now).state.status, Status::Arrest(_)) {
            break;
        }
        assert!(round < rig.config().max_rounds, "the code must end");
    }

    // Write whatever the height has made true, then read the sheet.
    let _ = rig.coin.state.call_time_of_death(&mut rig.coin.bank, rig.now);
    rig.audit();
    let sheet = rig
        .coin
        .bank
        .sink
        .iter()
        .find_map(|e| match e {
            Event::Rosc { sheet, .. } | Event::TimeOfDeath { sheet, .. } => Some(sheet.clone()),
            _ => None,
        })
        .expect("the code ended");
    assert_eq!(sheet.rhythm_at_arrest, if row.vf { Rhythm::Vf } else { Rhythm::Asystole });
    assert_eq!(sheet.target, row.target * 100, "target");
    // Round 1 has no fatigue and identical cycles, so its quality is the cycle quality.
    assert_eq!(sheet.rounds[0].quality_bps, row.cycle_quality_bps, "cycle quality");
    match row.rounds {
        Some(rounds) => {
            assert_eq!(sheet.outcome, Outcome::Rosc);
            assert_eq!(sheet.rounds.len() as u8, rounds, "rounds to ROSC");
            assert_eq!(round, rounds);
        }
        None => {
            assert_eq!(sheet.outcome, Outcome::Death(DeathReason::RoundsExhausted));
            assert_eq!(sheet.rounds.len() as u8, rig.config().max_rounds);
        }
    }
}

fn rows() -> Vec<Row> {
    vec![
        // Two rescuers, a compression every block, first arrest.
        Row { rescuers: 2, span: 30, scars: 0, no_flow: 0, vf: false, cycle_quality_bps: 10_000, target: 200, rounds: Some(2) },
        // Two rescuers, 30 compressions in 40 blocks.
        Row { rescuers: 2, span: 40, scars: 0, no_flow: 100, vf: false, cycle_quality_bps: 7_500, target: 210, rounds: Some(3) },
        // The same team on a VF arrest.
        Row { rescuers: 2, span: 40, scars: 0, no_flow: 100, vf: true, cycle_quality_bps: 7_500, target: 210, rounds: Some(1) },
        // One rescuer, hands-only, 30 in 40 blocks. The table prints 0.53 for 0.525.
        Row { rescuers: 1, span: 40, scars: 0, no_flow: 100, vf: false, cycle_quality_bps: 5_250, target: 210, rounds: Some(4) },
        // Two rescuers, 6 scars, 2,000 blocks of no-flow.
        Row { rescuers: 2, span: 40, scars: 6, no_flow: 2_000, vf: false, cycle_quality_bps: 7_500, target: 700, rounds: Some(8) },
        // One rescuer, hands-only, same damage.
        Row { rescuers: 1, span: 40, scars: 6, no_flow: 2_000, vf: false, cycle_quality_bps: 5_250, target: 700, rounds: None },
    ]
}

#[test]
fn the_six_rows_hold_on_the_live_preset() {
    for row in rows() {
        play(Config::live(), &row);
    }
}

#[test]
fn the_six_rows_hold_with_two_cycles_a_round() {
    // The demo preset shortens the round, not the scoring. Its no-flow limit
    // is too short for the damaged rows, so only that limit is lifted here.
    for row in rows() {
        let config = Config { no_flow_max_blocks: 12_000, ..Config::demo() };
        play(config, &row);
    }
}

#[test]
fn rounds_a_coin_can_still_be_saved_from() {
    // "Inside 10 rounds, a perfect team can still save a coin with 21 scars,
    // a realistic pair one with 16, and a lone hands-only rescuer one with 9."
    let cases: [(u8, u64, u8); 3] = [(2, 30, 21), (2, 40, 16), (1, 40, 9)];
    for (rescuers, span, scars) in cases {
        let ok = Row { rescuers, span, scars, no_flow: 0, vf: false, cycle_quality_bps: 0, target: 0, rounds: None };
        assert!(reaches_rosc(&ok), "{rescuers} rescuer(s), {scars} scars should be saveable");
        let too_many = Row { scars: scars + 1, ..ok };
        assert!(!reaches_rosc(&too_many), "{rescuers} rescuer(s), {} scars should not be", scars + 1);
    }
}

fn reaches_rosc(row: &Row) -> bool {
    let mut rig = Rig::new(Config::live(), 1_000_000);
    rig.coin.state.vitals.scars = row.scars;
    rig.to_arrest();
    let (a, b) = (0u32, if row.rescuers > 1 { 1u32 } else { 0u32 });
    let breaths = if row.rescuers > 1 { 2 } else { 0 };
    let mut round = 0;
    loop {
        round += 1;
        let (lead, airway) = if round % 2 == 1 { (a, b) } else { (b, a) };
        rig.round(lead, airway, row.span, breaths);
        match &rig.coin.state.preview(&rig.coin.bank, rig.now).state.status {
            Status::Arrest(_) => continue,
            Status::Alive => return true,
            _ => return false,
        }
    }
}
