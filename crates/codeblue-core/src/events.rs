//! What the rulebook reports to its host. The contract turns these into
//! NEP-297 logs, the ledger ring and the code sheets; the simulator counts them.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Rhythm {
    /// Arrest that followed a busy final bucket. The only shockable rhythm.
    Vf,
    /// Arrest with transfers that carried no pulse during the silence.
    Pea,
    /// Any other arrest.
    Asystole,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DeathReason {
    /// No compression arrived within `no_flow_max_blocks` of the arrest.
    NoFlow,
    /// Compressions paused for more than `max_pause_blocks` during the code.
    Pause,
    /// Round `max_rounds` ended below target.
    RoundsExhausted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BurnKind {
    Oxygen,
    NoFlow,
    Epinephrine,
    Death,
    Estate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PayKind {
    Faucet,
    Bounty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Outcome {
    Rosc,
    Death(DeathReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RoundLog<A> {
    pub round: u8,
    pub end_block: u64,
    /// Round quality after fatigue, in basis points.
    pub quality_bps: u16,
    pub fatigued: bool,
    pub lead: Option<A>,
    /// `Some(true)` for a shock that added perfusion, `Some(false)` for one that did not.
    pub shock: Option<bool>,
    pub epi: bool,
    /// Perfusion in hundredths of a point once the round's interventions were in.
    pub perfusion_after: u32,
}

/// One page per code: downtime, rounds, who did what and the bounty split.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CodeSheet<A> {
    pub code_no: u32,
    pub arrest_block: u64,
    pub rhythm_at_arrest: Rhythm,
    pub first_compression_block: Option<u64>,
    pub end_block: u64,
    pub outcome: Outcome,
    pub target: u32,
    pub perfusion: u32,
    pub rounds: Vec<RoundLog<A>>,
    pub units: Vec<(A, u32)>,
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub bounty: u128,
    pub shares: Vec<(A, crate::U128)>,
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub noflow_burn: u128,
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub epi_burn: u128,
    #[cfg_attr(feature = "serde", serde(with = "crate::u128_string"))]
    pub death_burn: u128,
    pub scars_after: u8,
    pub deaths_after: u8,
}

impl<A> CodeSheet<A> {
    /// Blocks from arrest to the end of the code.
    pub fn downtime_blocks(&self) -> u64 {
        self.end_block - self.arrest_block
    }

    /// Blocks from arrest to the first compression, if one ever landed.
    pub fn no_flow_blocks(&self) -> Option<u64> {
        self.first_compression_block.map(|b| b - self.arrest_block)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event<A> {
    Arrest { block: u64, code_no: u32, rhythm: Rhythm },
    Round { block: u64, code_no: u32, round: u8, quality_bps: u16, fatigued: bool, perfusion: u32, target: u32 },
    Shock { block: u64, round: u8, success: bool, perfusion: u32 },
    Epi { block: u64, round: u8, perfusion: u32 },
    Rosc { block: u64, sheet: CodeSheet<A> },
    TimeOfDeath { block: u64, reason: DeathReason, deaths: u8, sheet: CodeSheet<A> },
    Deceased { block: u64, estate: u128 },
    Rehab { block: u64, scars: u8 },
    /// Every burn, whatever caused it. The contract also emits NEP-141 `ft_burn`.
    Burn { block: u64, kind: BurnKind, amount: u128 },
    /// A bounty share credited to a rescuer's claimable balance.
    BountyCredited { block: u64, account: A, amount: u128 },
    /// Tokens that left the contract for a holder.
    Paid { block: u64, kind: PayKind, account: A, amount: u128 },
}
