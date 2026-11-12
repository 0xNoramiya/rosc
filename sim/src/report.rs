//! Turns runs into the files under `sim/out/`. Charts are drawn from these
//! files and from nothing else.
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

use codeblue_core::{Config, Outcome, ROSC};

use crate::engine::{outcome_name, rhythm_name, RunOutput};
use crate::scenario::{Scenario, DAY};

/// Nearest-rank quantile of a sorted slice.
pub fn quantile<T: Copy>(sorted: &[T], p: f64) -> Option<T> {
    if sorted.is_empty() {
        return None;
    }
    let rank = (p * sorted.len() as f64).ceil() as usize;
    Some(sorted[rank.clamp(1, sorted.len()) - 1])
}

fn whole(amount: u128) -> u128 {
    amount / ROSC
}

fn day_of(block: u64) -> f64 {
    block as f64 / DAY as f64
}

fn opt<T: ToString>(value: Option<T>) -> String {
    value.map_or(String::new(), |v| v.to_string())
}

fn fixed(value: f64, places: usize) -> String {
    format!("{value:.places$}")
}

/// One line of `summary.csv`.
pub struct Metric {
    pub scenario: &'static str,
    pub name: &'static str,
    pub value: String,
    pub unit: &'static str,
}

pub struct Summary {
    pub metrics: Vec<Metric>,
    pub arrest_free_share: f64,
    pub dead_share: f64,
    /// Day on which half the runs were dead, if that happened.
    pub median_death_day: Option<f64>,
    pub median_first_arrest_day: Option<f64>,
    pub take_share_p50: f64,
    pub take_share_max: f64,
}

pub fn write_scenario(
    dir: &Path,
    sc: &Scenario,
    config: &Config,
    runs: &[RunOutput],
    days: u64,
    block_ms: f64,
) -> io::Result<Summary> {
    let n = runs.len() as f64;

    // ── <scenario>.csv: the coin over time ─────────────────────────────
    let mut text = String::from(
        "day,runs,supply_p10,supply_p50,supply_p90,cart_p10,cart_p50,cart_p90,alive_share,pulse_share\n",
    );
    let mut median_death_day = None;
    for day in 0..=days as usize {
        let mut supply: Vec<u128> = runs.iter().map(|r| whole(r.days[day].supply)).collect();
        let mut cart: Vec<u128> = runs.iter().map(|r| whole(r.days[day].cart)).collect();
        supply.sort_unstable();
        cart.sort_unstable();
        let alive = runs.iter().filter(|r| r.days[day].status != 3).count() as f64 / n;
        let pulse = runs.iter().filter(|r| r.days[day].status == 0).count() as f64 / n;
        if alive <= 0.5 && median_death_day.is_none() {
            median_death_day = Some(day as f64);
        }
        let q = |v: &[u128], p| quantile(v, p).unwrap();
        writeln!(
            text,
            "{day},{},{},{},{},{},{},{},{},{}",
            runs.len(),
            q(&supply, 0.1),
            q(&supply, 0.5),
            q(&supply, 0.9),
            q(&cart, 0.1),
            q(&cart, 0.5),
            q(&cart, 0.9),
            fixed(alive, 4),
            fixed(pulse, 4),
        )
        .unwrap();
    }
    fs::write(dir.join(format!("{}.csv", sc.name)), text)?;

    // ── <scenario>.runs.csv: one line per run ──────────────────────────
    let minutes = |blocks: u64| blocks as f64 * block_ms / 60_000.0;
    let mut text = String::from(
        "seed,arrests,saves,deaths,first_arrest_day,death_day,rounds_to_rosc_mean,save_minutes_mean,\
bounty_total,bounty_top_share,pacemaker_beats,pacemaker_cost_per_day,final_supply,final_cart,\
burned_oxygen,burned_noflow,burned_epi,burned_death,burned_estate,beats,electrical\n",
    );
    for r in runs {
        let saved: Vec<&crate::engine::CodeRow> = r.codes.iter().filter(|c| c.outcome == Outcome::Rosc).collect();
        let mean = |f: &dyn Fn(&crate::engine::CodeRow) -> f64| {
            (!saved.is_empty()).then(|| saved.iter().map(|c| f(c)).sum::<f64>() / saved.len() as f64)
        };
        writeln!(
            text,
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            r.seed,
            r.arrests,
            r.saves,
            r.deaths,
            opt(r.first_arrest.map(|b| fixed(day_of(b), 3))),
            opt(r.deceased.map(|b| fixed(day_of(b), 3))),
            opt(mean(&|c| c.rounds as f64).map(|v| fixed(v, 2))),
            opt(mean(&|c| minutes(c.save_blocks.unwrap_or(0))).map(|v| fixed(v, 2))),
            whole(r.bounty_total),
            opt((r.bounty_total > 0).then(|| fixed(r.bounty_top as f64 / r.bounty_total as f64, 4))),
            r.pacer_beats,
            fixed(r.pacer_fees as f64 / ROSC as f64 / days as f64, 4),
            whole(r.final_supply),
            whole(r.final_cart),
            whole(r.totals.burned_oxygen),
            whole(r.totals.burned_noflow),
            whole(r.totals.burned_epi),
            whole(r.totals.burned_death),
            whole(r.totals.burned_estate),
            r.totals.beats,
            r.totals.electrical,
        )
        .unwrap();
    }
    fs::write(dir.join(format!("{}.runs.csv", sc.name)), text)?;

    // ── <scenario>.codes.csv: one line per code that ended ─────────────
    let mut text = String::from(
        "seed,code,arrest_day,hour,night,rhythm,responders,response_blocks,no_flow_blocks,rounds,outcome,\
downtime_blocks,save_blocks,save_minutes,bounty\n",
    );
    for r in runs {
        for c in &r.codes {
            let day = day_of(c.arrest_block);
            writeln!(
                text,
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                r.seed,
                c.code_no,
                fixed(day, 3),
                fixed(day.fract() * 24.0, 2),
                c.night as u8,
                rhythm_name(c.rhythm),
                c.responders,
                opt(c.response_blocks),
                opt(c.no_flow_blocks),
                c.rounds,
                outcome_name(c.outcome),
                c.downtime_blocks,
                opt(c.save_blocks),
                opt(c.save_blocks.map(|b| fixed(minutes(b), 2))),
                whole(c.bounty),
            )
            .unwrap();
        }
    }
    fs::write(dir.join(format!("{}.codes.csv", sc.name)), text)?;

    // ── Summary numbers ────────────────────────────────────────────────
    let mut metrics = Vec::new();
    let mut put = |name: &'static str, value: String, unit: &'static str| {
        metrics.push(Metric { scenario: sc.name, name, value, unit })
    };
    let share = |count: usize| count as f64 / n;
    let arrest_free_share = share(runs.iter().filter(|r| r.arrests == 0).count());
    let dead_share = share(runs.iter().filter(|r| r.deceased.is_some()).count());
    put("runs", runs.len().to_string(), "");
    put("runs_with_no_arrest", fixed(arrest_free_share, 4), "share");
    put("runs_dead_by_end", fixed(dead_share, 4), "share");

    let mean = |f: &dyn Fn(&RunOutput) -> f64| runs.iter().map(f).sum::<f64>() / n;
    put("arrests_per_run", fixed(mean(&|r| r.arrests as f64), 2), "mean");
    put("saves_per_run", fixed(mean(&|r| r.saves as f64), 2), "mean");
    put("deaths_per_run", fixed(mean(&|r| r.deaths as f64), 2), "mean");

    let mut first: Vec<u64> = runs.iter().filter_map(|r| r.first_arrest).collect();
    first.sort_unstable();
    // A median over all runs: undefined unless half of them arrested.
    let median_first_arrest_day =
        (first.len() * 2 >= runs.len()).then(|| day_of(first[(runs.len() + 1) / 2 - 1]));
    put("first_arrest_day_median", opt(median_first_arrest_day.map(|d| fixed(d, 2))), "day");
    put("death_day_median", opt(median_death_day.map(|d| fixed(d, 0))), "day");
    let mut died: Vec<u64> = runs.iter().filter_map(|r| r.deceased).collect();
    died.sort_unstable();
    put("death_day_p10", opt(quantile(&died, 0.1).filter(|_| dead_share >= 0.1).map(|b| fixed(day_of(b), 2))), "day");
    let mut last_beat_to_death: Vec<u64> = runs
        .iter()
        .filter_map(|r| r.deceased.map(|d| d.saturating_sub(r.first_arrest.unwrap_or(d))))
        .collect();
    last_beat_to_death.sort_unstable();
    put(
        "first_arrest_to_death_days_median",
        opt(quantile(&last_beat_to_death, 0.5).map(|b| fixed(day_of(b), 2))),
        "day",
    );

    let codes: Vec<&crate::engine::CodeRow> = runs.iter().flat_map(|r| r.codes.iter()).collect();
    let saved: Vec<&&crate::engine::CodeRow> = codes.iter().filter(|c| c.outcome == Outcome::Rosc).collect();
    put("codes", codes.len().to_string(), "");
    put("codes_saved", fixed(saved.len() as f64 / codes.len().max(1) as f64, 4), "share");
    let answered: Vec<&&crate::engine::CodeRow> = codes.iter().filter(|c| c.no_flow_blocks.is_some()).collect();
    put(
        "answered_codes_saved",
        fixed(saved.len() as f64 / answered.len().max(1) as f64, 4),
        "share",
    );
    let mut rounds: Vec<u8> = saved.iter().map(|c| c.rounds).collect();
    rounds.sort_unstable();
    put("rounds_to_rosc_median", opt(quantile(&rounds, 0.5)), "rounds");
    let mut save: Vec<u64> = saved.iter().filter_map(|c| c.save_blocks).collect();
    save.sort_unstable();
    let save_minutes = quantile(&save, 0.5)
        .map(|p50| (minutes(quantile(&save, 0.1).unwrap()), minutes(p50), minutes(quantile(&save, 0.9).unwrap())));
    put("save_minutes_p10", opt(save_minutes.map(|s| fixed(s.0, 2))), "min");
    put("save_minutes_median", opt(save_minutes.map(|s| fixed(s.1, 2))), "min");
    put("save_minutes_p90", opt(save_minutes.map(|s| fixed(s.2, 2))), "min");

    // Night shift: where the arrests fall, and what the delay does.
    let night = codes.iter().filter(|c| c.night).count();
    put("codes_at_night", fixed(night as f64 / codes.len().max(1) as f64, 4), "share");
    put("night_share_of_the_day", fixed(8.0 / 24.0, 4), "share");
    let saved_share = |pick: &dyn Fn(&crate::engine::CodeRow) -> bool| {
        let group: Vec<&&crate::engine::CodeRow> = codes.iter().filter(|c| pick(c)).collect();
        (!group.is_empty())
            .then(|| group.iter().filter(|c| c.outcome == Outcome::Rosc).count() as f64 / group.len() as f64)
    };
    let quarter = config.no_flow_max_blocks / 4;
    put("saved_when_first_response_within_quarter_limit", opt(saved_share(&|c| c.response_blocks.is_some_and(|b| b <= quarter)).map(|v| fixed(v, 4))), "share");
    put("saved_when_first_response_later", opt(saved_share(&|c| c.response_blocks.is_some_and(|b| b > quarter)).map(|v| fixed(v, 4))), "share");
    put("saved_by_day", opt(saved_share(&|c| !c.night).map(|v| fixed(v, 4))), "share");
    put("saved_by_night", opt(saved_share(&|c| c.night).map(|v| fixed(v, 4))), "share");

    // Bounties.
    let cart0 = config.cart_supply() as f64;
    let mut take: Vec<f64> = runs.iter().map(|r| r.bounty_total as f64 / cart0).collect();
    take.sort_by(f64::total_cmp);
    let take_share_p50 = quantile(&take, 0.5).unwrap();
    let take_share_max = *take.last().unwrap();
    put("bounty_share_of_cart_median", fixed(take_share_p50, 4), "share");
    put("bounty_share_of_cart_max", fixed(take_share_max, 4), "share");
    let mut top: Vec<f64> =
        runs.iter().filter(|r| r.bounty_total > 0).map(|r| r.bounty_top as f64 / r.bounty_total as f64).collect();
    top.sort_by(f64::total_cmp);
    put("top_account_share_of_bounty_median", opt(quantile(&top, 0.5).map(|v| fixed(v, 4))), "share");
    put("top_account_share_of_bounty_p90", opt(quantile(&top, 0.9).map(|v| fixed(v, 4))), "share");

    // Pacemaker.
    let pacer_cost_per_day = mean(&|r| r.pacer_fees as f64 / ROSC as f64) / days as f64;
    put("pacemaker_cost_per_day", fixed(pacer_cost_per_day, 4), "ROSC");
    put("pacemaker_beats_per_day", fixed(mean(&|r| r.pacer_beats as f64) / days as f64, 2), "beats");

    // Where the tokens went.
    let end = days as usize;
    let mut supply: Vec<u128> = runs.iter().map(|r| whole(r.days[end].supply)).collect();
    supply.sort_unstable();
    let mut cart: Vec<u128> = runs.iter().map(|r| whole(r.days[end].cart)).collect();
    cart.sort_unstable();
    put("supply_end_p10", quantile(&supply, 0.1).unwrap().to_string(), "ROSC");
    put("supply_end_median", quantile(&supply, 0.5).unwrap().to_string(), "ROSC");
    put("supply_end_p90", quantile(&supply, 0.9).unwrap().to_string(), "ROSC");
    put("cart_end_median", quantile(&cart, 0.5).unwrap().to_string(), "ROSC");
    let burned = |f: &dyn Fn(&RunOutput) -> u128| fixed(mean(&|r| whole(f(r)) as f64), 0);
    put("burned_oxygen_mean", burned(&|r| r.totals.burned_oxygen), "ROSC");
    put("burned_noflow_mean", burned(&|r| r.totals.burned_noflow), "ROSC");
    put("burned_epinephrine_mean", burned(&|r| r.totals.burned_epi), "ROSC");
    put("burned_death_mean", burned(&|r| r.totals.burned_death), "ROSC");
    put("burned_estate_mean", burned(&|r| r.totals.burned_estate), "ROSC");
    put("beats_per_day_mean", fixed(mean(&|r| r.totals.beats as f64) / days as f64, 2), "beats");

    Ok(Summary {
        metrics,
        arrest_free_share,
        dead_share,
        median_death_day,
        median_first_arrest_day,
        take_share_p50,
        take_share_max,
    })
}

pub fn write_summary(dir: &Path, summaries: &[Summary]) -> io::Result<()> {
    let mut text = String::from("scenario,metric,value,unit\n");
    for summary in summaries {
        for m in &summary.metrics {
            writeln!(text, "{},{},{},{}", m.scenario, m.name, m.value, m.unit).unwrap();
        }
    }
    fs::write(dir.join("summary.csv"), text)
}

pub fn write_scenarios(dir: &Path, scenarios: &[Scenario]) -> io::Result<()> {
    let mut text = String::from("scenario,knob,value\n");
    for sc in scenarios {
        for (knob, value) in sc.knobs() {
            writeln!(text, "{},{},{}", sc.name, knob, value).unwrap();
        }
    }
    fs::write(dir.join("scenarios.csv"), text)
}

/// Every `Config` value the runs used.
pub fn write_params(dir: &Path, preset: &str, config: &Config, block_ms: f64) -> io::Result<()> {
    let c = config;
    let rosc = |v: u128| format!("{}", v / ROSC);
    let rows: Vec<(&str, String, &str)> = vec![
        ("ARREST_BLOCKS", c.arrest_blocks.to_string(), "blocks"),
        ("MIN_STROKE", rosc(c.min_stroke), "ROSC"),
        ("REFRACTORY_BLOCKS", c.refractory_blocks.to_string(), "blocks"),
        ("BUCKET_BLOCKS", c.bucket_blocks.to_string(), "blocks"),
        ("SUDDEN_BEATS", c.sudden_beats.to_string(), "beats"),
        ("PEA_MIN", c.pea_min.to_string(), "transfers"),
        ("VF_DEGRADE_ROUND", c.vf_degrade_round.to_string(), "round"),
        ("COMPRESSIONS_PER_CYCLE", c.compressions_per_cycle.to_string(), ""),
        ("BREATHS_PER_CYCLE", c.breaths_per_cycle.to_string(), ""),
        ("CYCLES_PER_ROUND", c.cycles_per_round.to_string(), ""),
        ("VENT_WINDOW_BLOCKS", c.vent_window_blocks.to_string(), "blocks"),
        ("MAX_PAUSE_BLOCKS", c.max_pause_blocks.to_string(), "blocks"),
        ("MAX_ROUNDS", c.max_rounds.to_string(), "rounds"),
        ("NO_FLOW_MAX_BLOCKS", c.no_flow_max_blocks.to_string(), "blocks"),
        ("LAZARUS_BLOCKS", c.lazarus_blocks.to_string(), "blocks"),
        ("REHAB_BLOCKS", c.rehab_blocks.to_string(), "blocks"),
        ("BASE_TARGET", c.base_target.to_string(), "points"),
        ("SCAR_TARGET", c.scar_target.to_string(), "points"),
        ("NOFLOW_TARGET_CAP", c.noflow_target_cap.to_string(), "points"),
        ("NOFLOW_TARGET_BLOCKS", c.noflow_target_blocks.to_string(), "blocks per point"),
        ("HANDS_ONLY_BPS", c.hands_only_bps.to_string(), "bps"),
        ("BREATH_BPS", c.breath_bps.to_string(), "bps"),
        ("FATIGUE_BPS", c.fatigue_bps.to_string(), "bps"),
        ("EPI_BONUS", c.epi_bonus.to_string(), "points"),
        ("EPI_FIRST_ROUND", c.epi_first_round.to_string(), "round"),
        ("EPI_EVERY_ROUNDS", c.epi_every_rounds.to_string(), "rounds"),
        ("SHOCK_BONUS", c.shock_bonus.to_string(), "points"),
        ("SHOCK_MIN_QUALITY_BPS", c.shock_min_quality_bps.to_string(), "bps"),
        ("O2_BPS", c.o2_bps.to_string(), "bps"),
        ("PREMIUM_BPS", c.premium_bps.to_string(), "bps"),
        ("NOFLOW_BURN_BPS", c.noflow_burn_bps.to_string(), "bps"),
        ("NOFLOW_BURN_BLOCKS", c.noflow_burn_blocks.to_string(), "blocks"),
        ("EPI_DOSE", rosc(c.epi_dose), "ROSC"),
        ("DEATH_BURN_BPS", c.death_burn_bps.to_string(), "bps"),
        ("MAX_DEATHS", c.max_deaths.to_string(), ""),
        ("ROSC_SCARS", c.rosc_scars.to_string(), "scars"),
        ("DEATH_SCARS", c.death_scars.to_string(), "scars"),
        ("BOUNTY_BPS", c.bounty_bps.to_string(), "bps"),
        ("EF_START", c.ef_start.to_string(), "%"),
        ("EF_STEP", c.ef_step.to_string(), "%"),
        ("EF_FLOOR", c.ef_floor.to_string(), "%"),
        ("UNITS_COMPRESSION", c.units_compression.to_string(), "units"),
        ("UNITS_BREATH", c.units_breath.to_string(), "units"),
        ("UNITS_SHOCK", c.units_shock.to_string(), "units"),
        ("UNITS_CALL_CODE", c.units_call_code.to_string(), "units"),
        ("TOTAL_SUPPLY", rosc(c.total_supply), "ROSC"),
        ("CART_BPS", c.cart_bps.to_string(), "bps"),
        ("FAUCET_AMOUNT", rosc(c.faucet_amount), "ROSC"),
        ("MIN_RESCUER_BALANCE", rosc(c.min_rescuer_balance), "ROSC"),
        ("POST_MORTEM_TRANSFERS", c.post_mortem_transfers.to_string(), ""),
    ];
    let mut text = String::from("preset,parameter,value,unit\n");
    for (name, value, unit) in rows {
        writeln!(text, "{preset},{name},{value},{unit}").unwrap();
    }
    writeln!(text, "{preset},SIM_BLOCK_MS,{block_ms},ms").unwrap();
    fs::write(dir.join("params.csv"), text)
}
