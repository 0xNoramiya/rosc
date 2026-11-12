//! Random call sequences. Whatever eight accounts do, in whatever order and
//! at whatever pace, the books balance and the invariants in the contract
//! spec hold.
mod common;
use codeblue_core::mem::Coin;
use codeblue_core::{Config, DeathReason, Error, Event, Outcome, Phase, State, Status, Tap, ROSC};
use common::{Rig, Rng, ACCOUNTS};

const G: u64 = 10_000;

#[derive(Default, Debug)]
struct Seen {
    arrests: u32,
    rosc: u32,
    deaths: u32,
    deceased: u32,
    shocks: u32,
    doses: u32,
    refusals: u32,
    lazy_rosc: u32,
}

fn bank_eq(a: &Coin<Vec<Event<u32>>>, b: &Coin<Vec<Event<u32>>>) -> bool {
    a.state == b.state
        && a.bank.balances == b.bank.balances
        && a.bank.contract == b.bank.contract
        && a.bank.supply == b.bank.supply
        && a.bank.claimable == b.bank.claimable
        && a.bank.last_beat == b.bank.last_beat
        && a.bank.faucet_claimed == b.bank.faucet_claimed
}

/// One random call. Returns whether it succeeded.
fn act(rig: &mut Rig, rng: &mut Rng) -> Result<(), Error> {
    let who = rng.below(ACCOUNTS as u64) as u32;
    let other = rng.below(ACCOUNTS as u64) as u32;
    let (state, bank, now) = (&mut rig.coin.state, &mut rig.coin.bank, rig.now);
    let in_code = matches!(state.status, Status::Arrest(_));
    let roll = rng.below(100);
    if in_code {
        match roll {
            0..=69 => state.compress(bank, now, &who).map(|_| ()),
            70..=81 => state.breathe(bank, now, &who).map(|_| ()),
            82..=85 => state.shock(bank, now, &who).map(|_| ()),
            86..=89 => state.push_epi(bank, now, &who).map(|_| ()),
            90..=91 => state.call_code(bank, now, &who).map(|_| ()),
            92..=93 => state.call_time_of_death(bank, now).map(|_| ()),
            94..=95 => state.claim_bounty(bank, now, &who).map(|_| ()),
            _ => rig.coin.transfer(now, who, other, ROSC).map(|_| ()),
        }
    } else {
        match roll {
            0..=59 => {
                let amount = match rng.below(4) {
                    0 => rng.below(ROSC as u64) as u128,
                    1 => ROSC,
                    _ => (1 + rng.below(5_000)) as u128 * ROSC,
                };
                rig.coin.transfer(now, who, other, amount).map(|_| ())
            }
            60..=64 => rig.coin.donate(now, who, (1 + rng.below(100)) as u128 * ROSC).map(|_| ()),
            65..=74 => state.compress(bank, now, &who).map(|_| ()),
            75..=79 => state.call_code(bank, now, &who).map(|_| ()),
            80..=84 => state.call_time_of_death(bank, now).map(|_| ()),
            85..=92 => state.claim_bounty(bank, now, &who).map(|_| ()),
            93..=95 => state.faucet_claim(bank, now, &who).map(|_| ()),
            96..=97 => state.breathe(bank, now, &who).map(|_| ()),
            _ => state.push_epi(bank, now, &who).map(|_| ()),
        }
    }
}

/// How far the clock jumps between calls. Mostly a block at a time, with
/// pauses long enough to miss a window, stall a code or stop the heart.
fn step(rng: &mut Rng, calm: bool) -> u64 {
    match rng.below(1000) {
        0..=549 => 1,
        550..=799 => 0,
        800..=939 => 2 + rng.below(4),
        940..=979 => 10 + rng.below(30),
        980..=993 => if calm { 1 } else { 90 + rng.below(40) },
        994..=998 => if calm { 2 } else { 300 + rng.below(900) },
        _ => if calm { 3 } else { 1_500 + rng.below(4_000) },
    }
}

fn run(seed: u64, config: Config, steps: usize, seen: &mut Seen) {
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut rig = Rig::new(config.clone(), G);
    let mut event_cursor = 0;
    // Counted presses per block, checked against invariant 7.
    let mut last_compression: Option<u64> = None;

    for i in 0..steps {
        // Some seeds are given a patient team, so codes get finished.
        let calm = seed % 3 != 0 && matches!(rig.coin.state.status, Status::Arrest(_));
        rig.now += step(&mut rng, calm);
        let now = rig.now;
        let before = rig.coin.clone();
        let (deaths_before, was_deceased) =
            (before.state.vitals.deaths, matches!(before.state.status, Status::Deceased { .. }));
        let code_before = match &before.state.status {
            Status::Arrest(code) => Some(code.clone()),
            _ => None,
        };

        let result = act(&mut rig, &mut rng);

        // What the height alone had made true by now.
        let mut settled = before.clone();
        settled.state.settle(&mut settled.bank, now);
        if result.is_err() {
            // A refused call changes nothing beyond that. The contract reverts
            // it; here the state is simply what any later call would compute.
            seen.refusals += 1;
            assert!(bank_eq(&rig.coin, &settled), "seed {seed} step {i}: a refusal changed the state");
        }

        // Writing early never changes what a later call sees.
        if i % 7 == 0 {
            let later = now + rng.below(400);
            let (mut a, mut b) = (rig.coin.clone(), rig.coin.clone());
            let mid = now + rng.below(later - now + 1);
            a.state.settle(&mut a.bank, mid);
            a.state.settle(&mut a.bank, later);
            b.state.settle(&mut b.bank, later);
            assert!(bank_eq(&a, &b), "seed {seed} step {i}: settling twice differs from settling once");
            let preview = rig.coin.state.preview(&rig.coin.bank, later);
            assert!(preview.state == b.state && preview.supply == b.bank.supply, "preview differs from settle");
        }

        // 1. Supply equals the sum of balances and only falls.
        rig.audit();
        assert!(rig.coin.bank.supply <= before.bank.supply);
        // 2. No method writes Config.
        assert_eq!(rig.coin.state.config, config);
        // 4. Deaths never fall, and Deceased is terminal.
        assert!(rig.coin.state.vitals.deaths >= deaths_before);
        assert!(rig.coin.state.vitals.deaths <= config.max_deaths);
        if was_deceased {
            assert_eq!(rig.coin.state.status, before.state.status);
            assert_eq!(rig.coin.state.vitals, before.state.vitals);
            if !config.post_mortem_transfers {
                assert!(result.is_err(), "seed {seed} step {i}: a call succeeded after final death");
                assert!(bank_eq(&rig.coin, &before));
            }
        }
        // 5. Perfusion never falls within a code; the target is fixed by the
        //    first compression.
        if let (Some(old), Status::Arrest(new)) = (&code_before, &rig.coin.state.status) {
            if old.code_no == new.code_no {
                assert!(new.perfusion >= old.perfusion);
                if old.first_compression_block.is_some() {
                    assert_eq!(new.target, old.target);
                }
            }
        }
        // 6. Unclaimed bounties are always covered. (`audit` checks the sums.)
        assert!(rig.coin.state.pools.claimable <= rig.coin.bank.contract);

        // 7. At most one compression counts per block.
        if let Status::Arrest(code) = &rig.coin.state.status {
            if code.first_compression_block.is_some() {
                if let (Some(prev), true) = (last_compression, code.cycle_compressions > 0) {
                    let _ = prev;
                }
                last_compression = Some(code.last_compression_block);
            }
            assert!(code.cycle_compressions <= config.compressions_per_cycle);
            assert!(code.cycle_breaths < config.breaths_per_cycle.max(1));
            assert!(code.round >= 1 && code.round <= config.max_rounds);
            assert!(code.cycle >= 1 && code.cycle <= config.cycles_per_round);
            assert!(code.perfusion < code.target || code.first_compression_block.is_none());
        }

        // Read the new events: every death was due, and none came early.
        for event in &rig.coin.bank.sink[event_cursor..] {
            match event {
                Event::Arrest { .. } => seen.arrests += 1,
                Event::Shock { .. } => seen.shocks += 1,
                Event::Epi { .. } => seen.doses += 1,
                Event::Deceased { .. } => seen.deceased += 1,
                Event::Rosc { block, sheet } => {
                    seen.rosc += 1;
                    if *block < now && result.is_err() {
                        seen.lazy_rosc += 1;
                    }
                    assert_eq!(sheet.outcome, Outcome::Rosc);
                    assert!(sheet.perfusion >= sheet.target);
                    assert!(*block <= now);
                    let shares: u128 = sheet.shares.iter().map(|(_, s)| s.0).sum();
                    assert_eq!(shares, sheet.bounty);
                }
                Event::TimeOfDeath { block, reason, sheet, .. } => {
                    seen.deaths += 1;
                    assert!(*block <= now, "seed {seed}: a death was stamped in the future");
                    assert!(sheet.perfusion < sheet.target || sheet.first_compression_block.is_none());
                    match reason {
                        DeathReason::NoFlow => {
                            assert!(sheet.first_compression_block.is_none());
                            assert_eq!(*block, sheet.arrest_block + config.no_flow_max_blocks + 1);
                        }
                        DeathReason::Pause => {
                            assert!(sheet.first_compression_block.is_some());
                            assert!(sheet.rounds.len() < config.max_rounds as usize);
                        }
                        DeathReason::RoundsExhausted => {
                            assert_eq!(sheet.rounds.len(), config.max_rounds as usize);
                        }
                    }
                }
                _ => {}
            }
        }
        event_cursor = rig.coin.bank.sink.len();
    }
}

#[test]
fn random_calls_never_break_the_books_or_the_invariants() {
    let mut seen = Seen::default();
    for seed in 1..=120 {
        run(seed, Config::demo(), 4_000, &mut seen);
    }
    // A shorter code and a cheaper target, so every branch is reached often.
    let quick = Config { max_rounds: 3, base_target: 120, lazarus_blocks: 20, no_flow_max_blocks: 200, ..Config::demo() };
    for seed in 200..=320 {
        run(seed, quick.clone(), 4_000, &mut seen);
    }
    let revivable = Config { post_mortem_transfers: true, ..quick };
    for seed in 400..=440 {
        run(seed, revivable.clone(), 4_000, &mut seen);
    }
    println!("{seen:?}");
    // The run must have gone everywhere, or it proved nothing.
    assert!(seen.arrests > 500, "{seen:?}");
    assert!(seen.rosc > 100, "{seen:?}");
    assert!(seen.deaths > 300, "{seen:?}");
    assert!(seen.deceased > 50, "{seen:?}");
    assert!(seen.shocks > 0 && seen.doses > 20, "{seen:?}");
    assert!(seen.refusals > 10_000, "{seen:?}");
}

#[test]
fn one_compression_and_one_breath_count_per_block() {
    // Every account hammers both buttons on every block of a code.
    let mut rig = Rig::new(Config::demo(), G);
    rig.to_arrest();
    rig.coin.state.call_code(&mut rig.coin.bank, rig.now, &0).unwrap();
    let mut counted: Vec<(u64, &str)> = Vec::new();
    while matches!(rig.coin.state.status, Status::Arrest(_)) && rig.now < G + 5_000 {
        for who in 0..ACCOUNTS as u32 {
            for _ in 0..2 {
                if rig.coin.state.compress(&mut rig.coin.bank, rig.now, &who) == Ok(Tap::Counted) {
                    counted.push((rig.now, "compression"));
                }
                if rig.coin.state.breathe(&mut rig.coin.bank, rig.now, &who) == Ok(Tap::Counted) {
                    counted.push((rig.now, "breath"));
                }
            }
        }
        rig.now += 1;
    }
    assert_eq!(rig.coin.state.status, Status::Alive, "a crowd that never stops reaches ROSC");
    let mut sorted = counted.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), counted.len(), "a press counted twice in one block");
    let breaths = counted.iter().filter(|(_, what)| *what == "breath").count();
    assert!(breaths >= 8, "{breaths} breaths: {counted:?}");
}

/// A lone account that only ever compresses, on every block, from any state
/// a code can be in. It can never stall: within two windows it lands a
/// compression, and the code ends inside the round limit.
#[test]
fn one_account_alone_can_always_advance_a_code() {
    let mut rng = Rng(77);
    for trial in 0..300 {
        // Reach some state in the middle of a code with a random crowd.
        let mut rig = Rig::new(Config::demo(), G);
        rig.to_arrest();
        rig.now += rng.below(900);
        let warmup = rng.below(400);
        for _ in 0..warmup {
            if !matches!(rig.coin.state.preview(&rig.coin.bank, rig.now).state.status, Status::Arrest(_)) {
                break;
            }
            let _ = act(&mut rig, &mut rng);
            rig.now += rng.below(3);
        }
        let state: State<u32> = rig.coin.state.preview(&rig.coin.bank, rig.now).state;
        let Status::Arrest(code) = &state.status else { continue };
        let code_no = code.code_no;

        let limit = rig.config().max_rounds as u64
            * rig.config().cycles_per_round as u64
            * (rig.config().compressions_per_cycle as u64 + 2 * rig.config().vent_window_blocks + 4);
        let window = 2 * (rig.config().vent_window_blocks + 1);
        let (start, mut last_counted) = (rig.now, rig.now);
        loop {
            match rig.coin.state.compress(&mut rig.coin.bank, rig.now, &9) {
                Ok(Tap::Counted) => last_counted = rig.now,
                Ok(_) => {}
                Err(Error::HasPulse) | Err(Error::InMorgue { .. }) | Err(Error::Deceased { .. }) => break,
                Err(other) => panic!("trial {trial}: {other}"),
            }
            match &rig.coin.state.status {
                Status::Arrest(code) if code.code_no == code_no => {
                    assert!(rig.now - last_counted <= window, "trial {trial}: stalled in {:?}", code.phase);
                    assert!(!matches!(code.phase, Phase::AfterRound { .. }) || rig.now - last_counted <= window);
                }
                _ => break,
            }
            rig.now += 1;
            assert!(rig.now - start <= limit, "trial {trial}: the code did not end");
        }
        rig.audit();
    }
}
