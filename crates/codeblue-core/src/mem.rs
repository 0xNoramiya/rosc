//! An in-memory host. The tests and the simulator run the rulebook on it.
use crate::error::Error;
use crate::events::{BurnKind, Event, PayKind};
use crate::host::Host;
use crate::state::{State, Transfer};
use crate::Config;

/// Receives every event the rulebook emits.
pub trait Sink {
    fn on_event(&mut self, event: Event<u32>);
}

impl Sink for Vec<Event<u32>> {
    fn on_event(&mut self, event: Event<u32>) {
        self.push(event);
    }
}

/// Drops every event.
pub struct Quiet;

impl Sink for Quiet {
    fn on_event(&mut self, _: Event<u32>) {}
}

const NEVER: u64 = u64::MAX;

/// Token balances and per-account records, with accounts numbered from zero.
#[derive(Clone, Debug)]
pub struct Bank<S> {
    pub balances: Vec<u128>,
    /// The contract's own balance: crash cart, faucet pool and unclaimed bounties.
    pub contract: u128,
    pub supply: u128,
    pub last_beat: Vec<u64>,
    pub claimable: Vec<u128>,
    pub faucet_claimed: Vec<bool>,
    pub sink: S,
}

impl<S: Sink> Bank<S> {
    pub fn new(accounts: usize, supply: u128, sink: S) -> Self {
        Bank {
            balances: vec![0; accounts],
            contract: supply,
            supply,
            last_beat: vec![NEVER; accounts],
            claimable: vec![0; accounts],
            faucet_claimed: vec![false; accounts],
            sink,
        }
    }

    /// Sum of every balance, the contract's included. Must equal `supply`.
    pub fn sum_of_balances(&self) -> u128 {
        self.balances.iter().sum::<u128>() + self.contract
    }

    pub fn sum_of_claimable(&self) -> u128 {
        self.claimable.iter().sum()
    }
}

impl<S: Sink> Host for Bank<S> {
    type Account = u32;

    fn balance_of(&self, who: &u32) -> u128 {
        self.balances[*who as usize]
    }
    fn total_supply(&self) -> u128 {
        self.supply
    }
    fn burn_from_holder(&mut self, who: &u32, amount: u128) {
        self.balances[*who as usize] -= amount;
        self.supply -= amount;
    }
    fn holder_to_contract(&mut self, who: &u32, amount: u128) {
        self.balances[*who as usize] -= amount;
        self.contract += amount;
    }
    fn burn_from_contract(&mut self, amount: u128, _: BurnKind) {
        self.contract -= amount;
        self.supply -= amount;
    }
    fn contract_to_holder(&mut self, who: &u32, amount: u128, _: PayKind) {
        self.contract -= amount;
        self.balances[*who as usize] += amount;
    }
    fn last_beat_of(&self, who: &u32) -> Option<u64> {
        let block = self.last_beat[*who as usize];
        (block != NEVER).then_some(block)
    }
    fn set_last_beat_of(&mut self, who: &u32, block: u64) {
        self.last_beat[*who as usize] = block;
    }
    fn claimable_of(&self, who: &u32) -> u128 {
        self.claimable[*who as usize]
    }
    fn set_claimable_of(&mut self, who: &u32, amount: u128) {
        self.claimable[*who as usize] = amount;
    }
    fn faucet_claimed(&self, who: &u32) -> bool {
        self.faucet_claimed[*who as usize]
    }
    fn set_faucet_claimed(&mut self, who: &u32) {
        self.faucet_claimed[*who as usize] = true;
    }
    fn tag_of(&self, who: &u32) -> u32 {
        *who
    }
    fn emit(&mut self, event: Event<u32>) {
        self.sink.on_event(event);
    }
}

/// The rulebook and a bank, wired the way the contract wires them.
#[derive(Clone, Debug)]
pub struct Coin<S> {
    pub state: State<u32>,
    pub bank: Bank<S>,
}

impl<S: Sink> Coin<S> {
    pub fn launch(config: Config, genesis_block: u64, accounts: usize, sink: S) -> Self {
        let supply = config.total_supply;
        let state = State::new(config, genesis_block).expect("valid config");
        Coin { state, bank: Bank::new(accounts, supply, sink) }
    }

    /// `ft_transfer`: fees and the beat test, then the amount itself.
    pub fn transfer(&mut self, now: u64, from: u32, to: u32, amount: u128) -> Result<Transfer, Error> {
        let outcome = self.state.on_transfer(&mut self.bank, now, &from, &to, amount, false)?;
        self.bank.balances[from as usize] -= amount;
        self.bank.balances[to as usize] += amount;
        Ok(outcome)
    }

    /// A transfer to the contract's own account: a donation to the crash cart.
    pub fn donate(&mut self, now: u64, from: u32, amount: u128) -> Result<Transfer, Error> {
        // The contract is not a holder account, so the sender can never equal it.
        let outcome = self.state.on_transfer(&mut self.bank, now, &from, &u32::MAX, amount, true)?;
        self.bank.balances[from as usize] -= amount;
        self.bank.contract += amount;
        Ok(outcome)
    }
}
