//! `sim`: runs the Code Blue rulebook under simulated people.
//!
//!   sim run     six scenarios, written to sim/out/
//!   sim sweep   how the answers move with community size and ARREST_BLOCKS
//!   sim pair    how long a pair needs for a save, by old damage
//!   sim charts  SVG charts drawn from the files above
//!   sim all     everything (the default)
mod engine;
mod presets;
mod report;
mod rng;
mod scenario;
mod studies;

use std::path::{Path, PathBuf};
use std::{fs, io, thread};

use codeblue_core::Config;

use engine::RunOutput;
use scenario::Scenario;

pub struct Options {
    pub preset: String,
    pub runs: u64,
    pub days: u64,
    pub out: PathBuf,
    pub block_ms: f64,
    pub only: Option<String>,
}

/// Runs `job(i)` for `i` in `0..count` on every core. The result is in
/// index order, so the output never depends on the thread count.
pub fn parallel<T: Send>(count: usize, job: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let threads = thread::available_parallelism().map_or(4, |n| n.get()).min(count.max(1));
    let mut slots: Vec<Option<T>> = (0..count).map(|_| None).collect();
    let chunk = count.div_ceil(threads).max(1);
    thread::scope(|scope| {
        for (c, part) in slots.chunks_mut(chunk).enumerate() {
            let job = &job;
            scope.spawn(move || {
                for (i, slot) in part.iter_mut().enumerate() {
                    *slot = Some(job(c * chunk + i));
                }
            });
        }
    });
    slots.into_iter().map(|slot| slot.expect("every job ran")).collect()
}

/// The fixed seed list: 1, 2, 3, ... Each scenario draws its own stream from it.
pub fn simulate(sc: &Scenario, config: &Config, runs: u64, days: u64) -> Vec<RunOutput> {
    parallel(runs as usize, |i| engine::run(sc, config, i as u64 + 1, days))
}

fn run_scenarios(opts: &Options, config: &Config) -> io::Result<()> {
    let scenarios: Vec<Scenario> =
        scenario::all().into_iter().filter(|sc| opts.only.as_deref().is_none_or(|only| only == sc.name)).collect();
    fs::create_dir_all(&opts.out)?;
    let mut summaries = Vec::new();
    for sc in &scenarios {
        let started = std::time::Instant::now();
        let runs = simulate(sc, config, opts.runs, opts.days);
        let summary = report::write_scenario(&opts.out, sc, config, &runs, opts.days, opts.block_ms)?;
        eprintln!(
            "{:<12} {} runs in {:.1}s  no arrest {:.1}%  dead {:.1}%  first arrest day {}  median death day {}",
            sc.name,
            runs.len(),
            started.elapsed().as_secs_f64(),
            100.0 * summary.arrest_free_share,
            100.0 * summary.dead_share,
            summary.median_first_arrest_day.map_or("-".into(), |d| format!("{d:.1}")),
            summary.median_death_day.map_or("-".into(), |d| format!("{d:.0}")),
        );
        summaries.push(summary);
    }
    if opts.only.is_none() {
        report::write_summary(&opts.out, &summaries)?;
        report::write_scenarios(&opts.out, &scenarios)?;
        report::write_params(&opts.out, &opts.preset, config, opts.block_ms)?;
        studies::write_goals(&opts.out, &scenarios, &summaries, config, opts)?;
    }
    Ok(())
}

fn usage() -> ! {
    eprintln!(
        "usage: sim [run|sweep|pair|charts|all] [--preset NAME] [--runs N] [--days N] [--out DIR] \
[--block-ms MS] [--only SCENARIO]\npresets: {}",
        presets::names().join(", ")
    );
    std::process::exit(2)
}

fn main() -> io::Result<()> {
    let mut args = std::env::args().skip(1).peekable();
    let command = match args.peek() {
        Some(word) if !word.starts_with("--") => args.next().unwrap(),
        _ => "all".to_string(),
    };
    let mut opts = Options {
        preset: "live".into(),
        runs: 1_000,
        days: 60,
        out: PathBuf::new(),
        block_ms: 600.0,
        only: None,
    };
    let mut out: Option<PathBuf> = None;
    while let Some(flag) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| usage());
        match flag.as_str() {
            "--preset" => opts.preset = value(),
            "--runs" => opts.runs = value().parse().unwrap_or_else(|_| usage()),
            "--days" => opts.days = value().parse().unwrap_or_else(|_| usage()),
            "--out" => out = Some(PathBuf::from(value())),
            "--block-ms" => opts.block_ms = value().parse().unwrap_or_else(|_| usage()),
            "--only" => opts.only = Some(value()),
            _ => usage(),
        }
    }
    let Some(config) = presets::get(&opts.preset) else { usage() };
    config.validate().expect("preset is valid");
    // The chosen live preset writes to sim/out/; candidates sit beside it.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("out");
    opts.out = out.unwrap_or_else(|| if opts.preset == "live" { root.clone() } else { root.join("presets").join(&opts.preset) });

    match command.as_str() {
        "run" => run_scenarios(&opts, &config),
        "sweep" => studies::sweep(&opts),
        "pair" => studies::pair(&opts, &config),
        "all" => {
            run_scenarios(&opts, &config)?;
            studies::pair(&opts, &config)?;
            studies::sweep(&opts)
        }
        _ => usage(),
    }
}
