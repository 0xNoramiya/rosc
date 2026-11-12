//! The Code Blue ($ROSC) rulebook.
//!
//! A memecoin with a pulse: transfers are heartbeats, silence is cardiac
//! arrest, and holders bring the coin back with CPR played block by block.
//! This crate is the whole rulebook (state machine, scoring, economics) and
//! nothing else. It has no chain dependency, no randomness and no floating
//! point. The contract and the simulator both run it, so the simulated coin
//! and the deployed coin cannot disagree.
//!
//! The scoring borrows the shape of the adult cardiac arrest algorithm and
//! nothing more. The numbers are game design, not medicine.
//!
//! # How time works
//!
//! State is a function of the calls that succeeded and the block height.
//! Nothing has to be poked: an arrest, a lapsed ventilation window, a death
//! whose condition holds and the reopening of a code after the morgue are all
//! *derived* from the height, stamped with the block at which they became
//! true, and written by whichever call comes next. [`State::preview`] shows
//! the derived state without writing it.
#![forbid(unsafe_code)]

pub mod config;
pub mod error;
pub mod events;
pub mod history;
pub mod host;
pub mod math;
pub mod mem;
pub mod monitor;
pub mod ring;
pub mod state;
#[cfg(feature = "serde")]
pub mod u128_string;

pub use config::{Config, BPS, CENTI, ROSC};
pub use error::Error;
pub use events::{BurnKind, CodeSheet, DeathReason, Event, Outcome, PayKind, Rhythm, RoundLog};
pub use host::Host;
pub use monitor::Monitor;
pub use state::{Beat, Bucket, Code, Phase, Pools, State, Status, Tap, Totals, Transfer, Vitals};

/// A token amount that crosses JSON as a decimal string.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
pub struct U128(pub u128);

#[cfg(feature = "serde")]
impl serde::Serialize for U128 {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        u128_string::serialize(&self.0, s)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for U128 {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        u128_string::deserialize(d).map(U128)
    }
}
