//! Every tunable number. Written once at launch; no rule ever writes it.

/// One ROSC in base units (18 decimals).
pub const ROSC: u128 = 1_000_000_000_000_000_000;
/// Basis-point denominator.
pub const BPS: u64 = 10_000;
/// Perfusion and targets are stored in hundredths of a point so that
/// `perfusion += 100 * round_quality` is exact in integers.
pub const CENTI: u32 = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Config {
    // ── Vitals ─────────────────────────────────────────────────────────
    /// Silence that stops the heart: arrest when `block - last_beat > arrest_blocks`.
    pub arrest_blocks: u64,
    /// Smallest transfer that is a beat.
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub min_stroke: u128,
    /// Gap between two beats from one sender.
    pub refractory_blocks: u64,
    /// Width of one monitor bucket.
    pub bucket_blocks: u64,

    // ── Arrest rhythm ──────────────────────────────────────────────────
    /// Beats in the final bucket-width before the silence that make the arrest VF.
    pub sudden_beats: u32,
    /// Non-beat transfers during the silence that make the arrest PEA.
    pub pea_min: u32,
    /// A VF arrest that reaches this round without ROSC degrades to asystole.
    pub vf_degrade_round: u8,

    // ── Cycle and round ────────────────────────────────────────────────
    pub compressions_per_cycle: u8,
    pub breaths_per_cycle: u8,
    /// Cycles between rhythm checks.
    pub cycles_per_round: u8,
    /// Ventilation window after the last compression of a cycle, and the
    /// shock / epinephrine window after a round.
    pub vent_window_blocks: u64,
    /// Compressions paused longer than this during a code: death may be called.
    pub max_pause_blocks: u64,
    pub max_rounds: u8,
    /// Arrest with no compressions for longer than this: death may be called.
    pub no_flow_max_blocks: u64,
    /// Wait after a death before a new code opens.
    pub lazarus_blocks: u64,
    /// Unbroken life that heals one scar.
    pub rehab_blocks: u64,

    // ── Scoring (points; game design, not medicine) ────────────────────
    pub base_target: u32,
    pub scar_target: u32,
    pub noflow_target_cap: u32,
    /// Blocks of no-flow per extra target point.
    pub noflow_target_blocks: u64,
    /// Score of a cycle with no breaths, as a share of a full cycle.
    pub hands_only_bps: u16,
    /// Extra share per counted breath.
    pub breath_bps: u16,
    /// Round multiplier when the lead compressor also led the previous round.
    pub fatigue_bps: u16,
    pub epi_bonus: u32,
    /// First round after which epinephrine may be pushed, and the spacing after that.
    pub epi_first_round: u8,
    pub epi_every_rounds: u8,
    pub shock_bonus: u32,
    pub shock_min_quality_bps: u16,

    // ── Fees on a live transfer (paid by the sender on top of the amount)
    pub o2_bps: u16,
    pub premium_bps: u16,

    // ── Arrest costs ───────────────────────────────────────────────────
    /// Share of the cart burned per `noflow_burn_blocks` without compressions.
    pub noflow_burn_bps: u16,
    pub noflow_burn_blocks: u64,
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub epi_dose: u128,
    pub death_burn_bps: u16,
    pub max_deaths: u8,
    /// Scars added by a ROSC and by a time of death.
    pub rosc_scars: u8,
    pub death_scars: u8,

    // ── Bounty ─────────────────────────────────────────────────────────
    pub bounty_bps: u16,
    pub ef_start: u8,
    pub ef_step: u8,
    pub ef_floor: u8,
    pub units_compression: u32,
    pub units_breath: u32,
    pub units_shock: u32,
    pub units_call_code: u32,

    // ── Supply and access ──────────────────────────────────────────────
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub total_supply: u128,
    /// Share of the supply minted into the crash cart. The rest is the faucet.
    pub cart_bps: u16,
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub faucet_amount: u128,
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub min_rescuer_balance: u128,
    /// Launch-time flag: reopen plain transfers after final death, every mechanic off.
    pub post_mortem_transfers: bool,
}

impl Config {
    /// Values shared by both presets, with the demo timings.
    pub fn demo() -> Self {
        Config {
            arrest_blocks: 100,
            min_stroke: ROSC,
            refractory_blocks: 5,
            bucket_blocks: 10,

            sudden_beats: 8,
            pea_min: 3,
            vf_degrade_round: 4,

            compressions_per_cycle: 30,
            breaths_per_cycle: 2,
            cycles_per_round: 2,
            vent_window_blocks: 16,
            max_pause_blocks: 100,
            max_rounds: 10,
            no_flow_max_blocks: 1_000,
            lazarus_blocks: 50,
            rehab_blocks: 1_000,

            base_target: 200,
            scar_target: 50,
            noflow_target_cap: 200,
            noflow_target_blocks: 10,
            hands_only_bps: 7_000,
            breath_bps: 1_500,
            fatigue_bps: 8_000,
            epi_bonus: 50,
            epi_first_round: 2,
            epi_every_rounds: 2,
            shock_bonus: 150,
            shock_min_quality_bps: 6_000,

            o2_bps: 50,
            premium_bps: 50,

            noflow_burn_bps: 1,
            noflow_burn_blocks: 100,
            epi_dose: 1_000_000 * ROSC,
            death_burn_bps: 1_000,
            max_deaths: 3,
            rosc_scars: 1,
            death_scars: 2,

            bounty_bps: 200,
            ef_start: 60,
            ef_step: 5,
            ef_floor: 20,
            units_compression: 1,
            units_breath: 5,
            units_shock: 15,
            units_call_code: 10,

            total_supply: 2_500_000_000 * ROSC,
            cart_bps: 4_000,
            faucet_amount: 1_000_000 * ROSC,
            min_rescuer_balance: 1_000 * ROSC,
            post_mortem_transfers: false,
        }
    }

    /// The live preset, chosen on 12 Nov 2026 from the simulation: 12 hours
    /// of silence stops the heart. `docs/params.md` gives the reason for each
    /// value. Block counts assume 600 ms blocks.
    pub fn live() -> Self {
        Config {
            arrest_blocks: 72_000,
            min_stroke: 100 * ROSC,
            refractory_blocks: 100,
            bucket_blocks: 6_000,
            no_flow_max_blocks: 12_000,
            cycles_per_round: 5,
            lazarus_blocks: 6_000,
            rehab_blocks: 1_008_000,
            ..Self::demo()
        }
    }

    /// Rejects configurations under which the rules would contradict each other.
    pub fn validate(&self) -> Result<(), &'static str> {
        let bps = BPS as u32;
        if self.arrest_blocks == 0 || self.bucket_blocks == 0 {
            return Err("arrest_blocks and bucket_blocks must be positive");
        }
        if self.bucket_blocks > self.arrest_blocks {
            return Err("bucket_blocks must not exceed arrest_blocks");
        }
        if self.compressions_per_cycle == 0 || self.cycles_per_round == 0 || self.max_rounds == 0 {
            return Err("a cycle, a round and a code must each have at least one step");
        }
        if self.hands_only_bps as u32 + self.breath_bps as u32 * self.breaths_per_cycle as u32 > bps {
            return Err("a full cycle must not score above 100%");
        }
        if self.fatigue_bps as u32 > bps || self.shock_min_quality_bps as u32 > bps {
            return Err("fatigue and shock quality are shares of 100%");
        }
        // The forced hands-off time between two compressions is one ventilation
        // window plus one after-round window. It must stay below the pause limit,
        // or a code could die while every rule was being followed.
        if 2 * (self.vent_window_blocks + 1) >= self.max_pause_blocks {
            return Err("max_pause_blocks must exceed two ventilation windows");
        }
        if self.o2_bps as u32 + self.premium_bps as u32 > bps
            || self.death_burn_bps as u32 > bps
            || self.bounty_bps as u32 > bps
            || self.cart_bps as u32 > bps
        {
            return Err("basis-point shares must not exceed 100%");
        }
        if self.noflow_burn_blocks == 0 || self.noflow_target_blocks == 0 {
            return Err("no-flow divisors must be positive");
        }
        let worst = self.noflow_burn_bps as u128 * self.no_flow_max_blocks as u128
            / self.noflow_burn_blocks as u128;
        if worst > BPS as u128 {
            return Err("the no-flow burn could exceed the whole cart");
        }
        if self.ef_start == 0 || self.ef_floor > self.ef_start {
            return Err("ejection fraction needs 0 < floor <= start");
        }
        if self.max_deaths == 0 {
            return Err("max_deaths must be positive");
        }
        if self.epi_first_round == 0 || self.epi_every_rounds == 0 {
            return Err("epinephrine rounds must be positive");
        }
        if self.rehab_blocks == 0 {
            return Err("rehab_blocks must be positive");
        }
        Ok(())
    }

    /// Tokens minted into the crash cart at launch.
    pub fn cart_supply(&self) -> u128 {
        crate::math::mul_div(self.total_supply, self.cart_bps as u64, BPS)
    }

    /// Tokens minted into the faucet pool at launch.
    pub fn faucet_supply(&self) -> u128 {
        self.total_supply - self.cart_supply()
    }
}
