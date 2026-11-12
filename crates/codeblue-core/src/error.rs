//! Refusals. The strings are part of the product: the first seven come from
//! the spec's message table and must match it character for character.
use core::fmt;

use crate::config::ROSC;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    // ── Spec message table ─────────────────────────────────────────────
    /// A transfer while the coin has no pulse.
    NoPulse,
    /// A rescue call while the coin is alive.
    HasPulse,
    /// `shock()` on asystole or PEA.
    NotShockable,
    /// `breathe()` by the cycle's lead compressor.
    OnTheChest,
    /// `push_epi()` outside its window.
    EpiNotYet,
    /// `call_time_of_death()` while no death condition holds.
    StillInCode,
    /// Any call after final death.
    Deceased { block: u64 },

    // ── Refusals the table does not cover ──────────────────────────────
    /// A rescue call between a time of death and the next code.
    InMorgue { until_block: u64 },
    /// `call_code()` after the code was already called or begun.
    CodeAlreadyCalled,
    /// `breathe()` outside the ventilation window.
    AirwayClosed,
    /// `shock()` on a shockable rhythm, but not at the rhythm check.
    ShockNotNow,
    /// `push_epi()` when the cart cannot cover a dose.
    CartEmpty,
    /// `call_time_of_death()` while the coin is alive.
    NoDeathToCall,
    /// A rescue call from an account below the minimum balance.
    NotARescuer { min_balance: u128 },
    /// A transfer the sender cannot cover once the fees are added.
    InsufficientBalance { need: u128, have: u128 },
    NothingToClaim,
    FaucetAlreadyClaimed,
    FaucetEmpty,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoPulse => f.write_str(
                "No pulse. Tokens cannot circulate without circulation. Start compressions.",
            ),
            Error::HasPulse => f.write_str("Patient has a pulse. Stop compressions."),
            Error::NotShockable => f.write_str("Not a shockable rhythm. Resume compressions."),
            Error::OnTheChest => {
                f.write_str("You are on the chest. Someone else takes the airway.")
            }
            Error::EpiNotYet => f.write_str("Epinephrine is every 3 to 5 minutes. Not yet."),
            Error::StillInCode => f.write_str("Still in the code. Keep going."),
            Error::Deceased { block } => write!(f, "Time of death was called at block {block}."),

            Error::InMorgue { until_block } => {
                write!(f, "Patient is in the morgue. The next code opens at block {until_block}.")
            }
            Error::CodeAlreadyCalled => f.write_str("The code is already called. Get on the chest."),
            Error::AirwayClosed => f.write_str("Airway is closed. Thirty compressions first."),
            Error::ShockNotNow => {
                f.write_str("Shocks follow the rhythm check. Resume compressions.")
            }
            Error::CartEmpty => f.write_str("The crash cart is out of epinephrine."),
            Error::NoDeathToCall => f.write_str("Patient has a pulse. There is no death to call."),
            Error::NotARescuer { min_balance } => {
                write!(f, "Rescuers hold at least {} ROSC.", min_balance / ROSC)
            }
            Error::InsufficientBalance { need, have } => {
                write!(f, "Balance {have} does not cover {need} with the oxygen burn and premium.")
            }
            Error::NothingToClaim => f.write_str("Nothing to claim."),
            Error::FaucetAlreadyClaimed => f.write_str("The faucet pays once per account."),
            Error::FaucetEmpty => f.write_str("The faucet is empty."),
        }
    }
}

impl std::error::Error for Error {}
