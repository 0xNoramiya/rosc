//! The agents' knobs and the six scenarios from the spec.
//!
//! These numbers describe people, not the contract. They are assumptions:
//! the point of the simulator is to see how the contract's parameters behave
//! under them, and `sim sweep` shows how the answers move when they change.

/// Blocks in a day at the nominal 600 ms block time.
pub const DAY: u64 = 144_000;

#[derive(Clone, Debug)]
pub struct Scenario {
    pub name: &'static str,
    pub title: &'static str,
    pub question: &'static str,

    // ── Holders: transfer at a rate that follows an attention curve ─────
    pub holders: u32,
    /// Transfers per holder per day at launch.
    pub base_rate: f64,
    /// Days for attention to halve. `None` is steady.
    pub half_life_days: Option<f64>,
    /// Day and night swing: the rate moves between `1 - swing` and `1 + swing`.
    pub swing: f64,
    /// Day on which every holder and rescuer leaves.
    pub leave_day: Option<f64>,
    /// Share of transfers too small to be a beat.
    pub dust_share: f64,
    /// Typical transfer, in ROSC, and its log-scale spread.
    pub amount_median: f64,
    pub amount_sigma: f64,

    // ── Rescuers: notice an arrest after a delay, then compress ─────────
    pub rescuers: u32,
    /// Chance that each rescuer shows up for a given code.
    pub show_up: f64,
    /// Blocks from the arrest to a rescuer's first call: median and spread.
    pub delay_median: f64,
    pub delay_sigma: f64,
    /// Multiplier on the delay for an arrest that starts at night.
    pub night_delay: f64,
    /// Blocks a rescuer needs for 30 compressions. 30 is one every block.
    pub blocks_per_30: f64,
    /// Chance that a second rescuer takes the airway when two are present.
    pub airway: f64,

    // ── Pacemaker: transfers at a fixed interval ────────────────────────
    /// Share of `ARREST_BLOCKS` between two pacemaker beats. `None` is off.
    pub pacemaker: Option<f64>,

    // ── Serial rescuer: never transfers, saves every arrest ─────────────
    /// One person with two keys and a metronome, on a coin nobody else touches.
    pub serial_rescuer: bool,
}

/// The reference community: a small testnet coin a few weeks after a hackathon.
fn community() -> Scenario {
    Scenario {
        name: "",
        title: "",
        question: "",
        holders: 120,
        base_rate: 0.5,
        half_life_days: None,
        swing: 0.5,
        leave_day: None,
        dust_share: 0.05,
        amount_median: 2_000.0,
        amount_sigma: 1.0,
        rescuers: 4,
        show_up: 0.5,
        delay_median: 1_000.0,
        delay_sigma: 1.0,
        night_delay: 1.0,
        blocks_per_30: 40.0,
        airway: 0.8,
        pacemaker: None,
        serial_rescuer: false,
    }
}

pub fn all() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "healthy",
            title: "Healthy",
            question: "Does the coin go 60 days with no arrest?",
            ..community()
        },
        Scenario {
            name: "fading",
            title: "Fading",
            question: "When is the first arrest, and when does it die?",
            half_life_days: Some(10.0),
            ..community()
        },
        Scenario {
            name: "abandoned",
            title: "Abandoned",
            question: "How long from the last beat to final death?",
            leave_day: Some(20.0),
            ..community()
        },
        Scenario {
            name: "paced",
            title: "Paced",
            question: "What does faking life cost per day?",
            holders: 0,
            rescuers: 0,
            pacemaker: Some(0.5),
            ..community()
        },
        Scenario {
            name: "farmed",
            title: "Farmed",
            question: "How much of the cart can one account take?",
            holders: 0,
            rescuers: 0,
            serial_rescuer: true,
            ..community()
        },
        Scenario {
            name: "night-shift",
            title: "Night shift",
            question: "Do arrests cluster at night, and does response time decide the outcome?",
            half_life_days: Some(10.0),
            swing: 0.95,
            night_delay: 8.0,
            ..community()
        },
    ]
}

impl Scenario {
    /// Share of launch attention left on `day`.
    pub fn attention(&self, day: f64) -> f64 {
        if self.leave_day.is_some_and(|leave| day >= leave) {
            return 0.0;
        }
        match self.half_life_days {
            Some(half_life) => 0.5f64.powf(day / half_life),
            None => 1.0,
        }
    }

    /// Day and night factor at a time of day in [0, 1). Peaks mid-afternoon.
    pub fn daylight(&self, time_of_day: f64) -> f64 {
        1.0 + self.swing * (std::f64::consts::TAU * (time_of_day - 0.6)).cos()
    }

    /// Night runs from 22:00 to 06:00.
    pub fn is_night(time_of_day: f64) -> bool {
        !(0.25..0.9167).contains(&time_of_day)
    }

    pub fn knobs(&self) -> Vec<(&'static str, String)> {
        vec![
            ("holders", self.holders.to_string()),
            ("transfers_per_holder_per_day", self.base_rate.to_string()),
            ("attention_half_life_days", self.half_life_days.map_or("steady".into(), |d| d.to_string())),
            ("day_night_swing", self.swing.to_string()),
            ("everyone_leaves_on_day", self.leave_day.map_or("never".into(), |d| d.to_string())),
            ("dust_share", self.dust_share.to_string()),
            ("transfer_median_rosc", self.amount_median.to_string()),
            ("transfer_log_sigma", self.amount_sigma.to_string()),
            ("rescuers", self.rescuers.to_string()),
            ("rescuer_show_up_chance", self.show_up.to_string()),
            ("rescuer_delay_median_blocks", self.delay_median.to_string()),
            ("rescuer_delay_log_sigma", self.delay_sigma.to_string()),
            ("night_delay_multiplier", self.night_delay.to_string()),
            ("blocks_per_30_compressions", self.blocks_per_30.to_string()),
            ("airway_partner_chance", self.airway.to_string()),
            ("pacemaker_interval_share_of_arrest", self.pacemaker.map_or("off".into(), |p| p.to_string())),
            ("serial_rescuer", self.serial_rescuer.to_string()),
        ]
    }
}
