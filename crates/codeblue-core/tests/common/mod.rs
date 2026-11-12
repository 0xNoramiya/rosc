//! A small rig for driving the rulebook in tests.
#![allow(dead_code)]
use codeblue_core::mem::Coin;
use codeblue_core::{Config, Event, Phase, Status, Tap};

pub const ACCOUNTS: usize = 12;

pub struct Rig {
    pub coin: Coin<Vec<Event<u32>>>,
    pub now: u64,
}

impl Rig {
    /// A coin launched at `genesis` with every account holding a faucet claim.
    pub fn new(config: Config, genesis: u64) -> Self {
        let mut coin = Coin::launch(config, genesis, ACCOUNTS, Vec::new());
        for who in 0..ACCOUNTS as u32 {
            coin.state.faucet_claim(&mut coin.bank, genesis, &who).unwrap();
        }
        Rig { coin, now: genesis }
    }

    pub fn config(&self) -> &Config {
        &self.coin.state.config
    }

    /// Jumps to the first block of the arrest that is due.
    pub fn to_arrest(&mut self) {
        self.now = self.coin.state.arrest_due_block();
    }

    pub fn compress(&mut self, who: u32) -> Tap {
        self.coin.state.compress(&mut self.coin.bank, self.now, &who).unwrap()
    }

    pub fn breathe(&mut self, who: u32) -> Tap {
        self.coin.state.breathe(&mut self.coin.bank, self.now, &who).unwrap()
    }

    pub fn in_arrest(&self) -> bool {
        matches!(self.coin.state.status, Status::Arrest(_))
    }

    pub fn code(&self) -> &codeblue_core::Code<u32> {
        match &self.coin.state.status {
            Status::Arrest(code) => code,
            other => panic!("no code running: {other:?}"),
        }
    }

    /// One cycle: the cycle's compressions spread over `span` blocks, the
    /// first of them at the current block, then `breaths` breaths from
    /// `airway`. With fewer breaths than a full cycle the ventilation window
    /// is left to lapse. Ends on the first block of whatever comes next.
    pub fn cycle(&mut self, lead: u32, airway: u32, span: u64, breaths: u8) {
        let per_cycle = self.config().compressions_per_cycle as u64;
        let full = self.config().breaths_per_cycle;
        assert!(span >= per_cycle);
        let start = self.now;
        for i in 0..per_cycle - 1 {
            self.now = start + i;
            assert_eq!(self.compress(lead), Tap::Counted);
        }
        self.now = start + span - 1;
        assert_eq!(self.compress(lead), Tap::Counted);
        for _ in 0..breaths {
            self.now += 1;
            assert_eq!(self.breathe(airway), Tap::Counted);
        }
        if breaths < full {
            self.now += self.config().vent_window_blocks + 1 - breaths as u64;
        } else {
            self.now += 1;
        }
    }

    /// One round of cycles like [`Rig::cycle`], then every intervention the
    /// rhythm check offers. Returns false once the code has ended.
    pub fn round(&mut self, lead: u32, airway: u32, span: u64, breaths: u8) -> bool {
        for _ in 0..self.config().cycles_per_round {
            self.cycle(lead, airway, span, breaths);
        }
        // Make the derived state current without writing anything.
        let view = self.coin.state.preview(&self.coin.bank, self.now).state;
        let Status::Arrest(code) = &view.status else { return false };
        if let Phase::AfterRound { shock_open, epi_open, .. } = code.phase {
            if shock_open {
                self.coin.state.shock(&mut self.coin.bank, self.now, &lead).unwrap();
            }
            if epi_open && self.in_arrest() {
                self.coin.state.push_epi(&mut self.coin.bank, self.now, &lead).unwrap();
            }
            self.now += 1;
        }
        self.in_arrest()
    }

    /// Checks the books: supply, the contract's balance and the claimable total.
    pub fn audit(&self) {
        let (state, bank) = (&self.coin.state, &self.coin.bank);
        assert_eq!(bank.sum_of_balances(), bank.supply, "supply equals the sum of balances");
        assert!(bank.supply <= state.config.total_supply, "supply only falls");
        assert_eq!(
            bank.contract,
            state.pools.cart + state.pools.faucet + state.pools.claimable,
            "the contract's balance is the cart, the faucet and unclaimed bounties"
        );
        assert_eq!(bank.sum_of_claimable(), state.pools.claimable, "claimable balances add up");
        assert_eq!(
            state.config.total_supply - bank.supply,
            state.totals.burned(),
            "every missing token was burned by a rule"
        );
    }
}

/// xorshift64*. Deterministic, good enough to shuffle test actions.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}
