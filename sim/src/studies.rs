//! The side studies: the design-goal check, the pair timing and the sweep.
use std::fmt::Write as _;
use std::path::Path;
use std::{fs, io};

use codeblue_core::Config;

use crate::report::{quantile, Summary};
use crate::scenario::{self, Scenario, DAY};
use crate::{engine, parallel, simulate, Options};

fn find<'a>(scenarios: &[Scenario], summaries: &'a [Summary], name: &str) -> Option<&'a Summary> {
    scenarios.iter().position(|sc| sc.name == name).map(|i| &summaries[i])
}

/// The four design goals from the spec, checked against this run.
pub fn write_goals(
    dir: &Path,
    scenarios: &[Scenario],
    summaries: &[Summary],
    config: &Config,
    opts: &Options,
) -> io::Result<()> {
    let mut text = String::from("goal,target,measured,verdict\n");
    let mut row = |goal: &str, target: &str, measured: String, pass: bool| {
        writeln!(text, "{goal},{target},{measured},{}", if pass { "pass" } else { "fail" }).unwrap();
    };
    if let Some(s) = find(scenarios, summaries, "healthy") {
        row(
            "A healthy coin has zero arrests in 60 days",
            "100% of runs",
            format!("{:.1}% of runs", 100.0 * s.arrest_free_share),
            s.arrest_free_share >= 1.0,
        );
    }
    if let Some(s) = find(scenarios, summaries, "fading") {
        row(
            "A fading coin's median death falls between day 20 and day 50",
            "day 20 to 50",
            s.median_death_day.map_or("not within 60 days".into(), |d| format!("day {d:.0}")),
            s.median_death_day.is_some_and(|d| (20.0..=50.0).contains(&d)),
        );
    }
    // A realistic pair: two rescuers, 30 compressions in 40 blocks, both
    // breaths, epinephrine when offered. Timed from first compression to ROSC.
    let minutes = |scars: u8| {
        let mut blocks: Vec<u64> = parallel(1_000, |i| engine::pair_save(config, scars, 40.0, true, i as u64 + 1))
            .into_iter()
            .flatten()
            .map(|r| r.0)
            .collect();
        blocks.sort_unstable();
        quantile(&blocks, 0.5).map(|b| b as f64 * opts.block_ms / 60_000.0)
    };
    let (fresh, scarred) = (minutes(0), minutes(5));
    row(
        "A realistic pair needs 4 to 12 minutes for a save",
        "4 to 12 min",
        match (fresh, scarred) {
            (Some(a), Some(b)) => format!("{a:.1} min with no scars; {b:.1} min with 5 scars"),
            _ => "no saves".into(),
        },
        fresh.is_some_and(|m| (4.0..=12.0).contains(&m)) && scarred.is_some_and(|m| (4.0..=12.0).contains(&m)),
    );
    if let Some(s) = find(scenarios, summaries, "farmed") {
        row(
            "A serial rescuer takes under 25% of the cart",
            "under 25%",
            format!("{:.1}% (worst run {:.1}%)", 100.0 * s.take_share_p50, 100.0 * s.take_share_max),
            s.take_share_max < 0.25,
        );
    }
    fs::write(dir.join("goals.csv"), text)
}

/// Minutes and rounds a pair needs for a save, by the scars the coin
/// already carries. One line per scar count and pace.
pub fn pair(opts: &Options, config: &Config) -> io::Result<()> {
    fs::create_dir_all(&opts.out)?;
    let trials = opts.runs.min(1_000) as usize;
    let minutes = |blocks: u64| blocks as f64 * opts.block_ms / 60_000.0;
    let mut text = String::from(
        "team,blocks_per_30,scars,trials,saved_share,minutes_p10,minutes_p50,minutes_p90,rounds_p50\n",
    );
    // (label, blocks per 30 compressions, second rescuer on the airway)
    let teams: [(&str, f64, bool); 3] = [("perfect pair", 30.0, true), ("realistic pair", 40.0, true), ("alone hands-only", 40.0, false)];
    for (label, pace, airway) in teams {
        for scars in 0..=24u8 {
            let results = parallel(trials, |i| {
                if airway {
                    engine::pair_save(config, scars, pace, true, i as u64 + 1)
                } else {
                    engine::lone_save(config, scars, pace, i as u64 + 1)
                }
            });
            let mut blocks: Vec<u64> = results.iter().flatten().map(|r| r.0).collect();
            let mut rounds: Vec<u8> = results.iter().flatten().map(|r| r.1).collect();
            blocks.sort_unstable();
            rounds.sort_unstable();
            let cell = |p: f64| quantile(&blocks, p).map_or(String::new(), |b| format!("{:.2}", minutes(b)));
            writeln!(
                text,
                "{label},{pace},{scars},{trials},{:.4},{},{},{},{}",
                blocks.len() as f64 / trials as f64,
                cell(0.1),
                cell(0.5),
                cell(0.9),
                quantile(&rounds, 0.5).map_or(String::new(), |r| r.to_string()),
            )
            .unwrap();
        }
    }
    fs::write(opts.out.join("pair.csv"), text)
}

/// The two numbers that decide most outcomes are how busy the community is
/// and how long a silence stops the heart. This runs the healthy and the
/// fading community across both.
pub fn sweep(opts: &Options) -> io::Result<()> {
    fs::create_dir_all(&opts.out)?;
    let runs = (opts.runs / 5).max(50);
    let base = scenario::all();
    let healthy = base.iter().find(|sc| sc.name == "healthy").unwrap();
    let fading = base.iter().find(|sc| sc.name == "fading").unwrap();
    let mut text = String::from(
        "arrest_hours,arrest_blocks,launch_beats_per_day,runs,healthy_runs_with_arrest,fading_first_arrest_day_median,\
fading_death_day_median,fading_dead_by_end\n",
    );
    for hours in [1u64, 3, 6, 12, 24, 48] {
        let arrest_blocks = hours * DAY / 24;
        let live = Config::live();
        // Only the window moves. The bucket shrinks with it where it must.
        let config = Config { arrest_blocks, bucket_blocks: live.bucket_blocks.min(arrest_blocks), ..live };
        for beats in [5.0f64, 10.0, 20.0, 40.0, 80.0, 160.0, 320.0, 640.0] {
            let with_rate = |sc: &Scenario| Scenario { base_rate: beats / sc.holders as f64, ..sc.clone() };
            let h = simulate(&with_rate(healthy), &config, runs, opts.days);
            let f = simulate(&with_rate(fading), &config, runs, opts.days);
            let arrested = h.iter().filter(|r| r.arrests > 0).count() as f64 / runs as f64;
            let half = (runs as usize + 1) / 2;
            let mut first: Vec<u64> = f.iter().filter_map(|r| r.first_arrest).collect();
            first.sort_unstable();
            let mut dead: Vec<u64> = f.iter().filter_map(|r| r.deceased).collect();
            dead.sort_unstable();
            let day = |v: &[u64]| v.get(half - 1).map_or(String::new(), |b| format!("{:.1}", *b as f64 / DAY as f64));
            writeln!(
                text,
                "{hours},{arrest_blocks},{beats},{runs},{arrested:.4},{},{},{:.4}",
                day(&first),
                day(&dead),
                dead.len() as f64 / runs as f64,
            )
            .unwrap();
        }
        eprintln!("sweep: ARREST_BLOCKS = {hours} h done");
    }
    fs::write(opts.out.join("sweep.csv"), text)
}
