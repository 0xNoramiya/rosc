//! Named parameter sets. `live` is the preset chosen from the simulation;
//! the others are the candidates it was compared against.
use codeblue_core::Config;

use crate::scenario::DAY;

/// Name, what it is, and the value.
pub fn candidates() -> Vec<(&'static str, &'static str, Config)> {
    vec![
        ("live", "The live preset: 12 hours of silence stops the heart", Config::live()),
        (
            "live-24h",
            "The spec's first placeholder: ARREST_BLOCKS 144,000, 24 hours",
            Config { arrest_blocks: DAY, ..Config::live() },
        ),
        (
            "live-6h",
            "ARREST_BLOCKS 36,000: 6 hours",
            Config { arrest_blocks: DAY / 4, ..Config::live() },
        ),
        ("demo", "Demo preset: a whole life in minutes", Config::demo()),
    ]
}

pub fn names() -> Vec<&'static str> {
    candidates().into_iter().map(|(name, _, _)| name).collect()
}

pub fn get(name: &str) -> Option<Config> {
    candidates().into_iter().find(|(n, _, _)| *n == name).map(|(_, _, config)| config)
}
