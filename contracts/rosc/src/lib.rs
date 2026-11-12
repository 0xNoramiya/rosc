//! Code Blue ($ROSC): a memecoin with a pulse.
//!
//! Transfers are heartbeats. When holders stop transacting the coin goes
//! into cardiac arrest, transfers freeze, and holders bring it back with CPR
//! played block by block. Three deaths and it is gone for good.
//!
//! One contract holds the token and the whole mechanic. Every rule comes
//! from `codeblue-core`, the same crate the simulator runs. This file wires
//! it to NEAR: who is calling, what block it is, where state is stored.
//!
//! There is no owner and no upgrade method. `Config` is written once, in
//! `new`. After setup the account has no access keys, and the wasm imports
//! no host function that deploys code, adds keys or deletes the account.
//!
//! Testnet toy. Not an investment, not a medical device, not CPR training.
mod host;
mod storage;
mod store;
mod token;

use codeblue_core::{Config, Error};
use near_sdk::json_types::U128;
use near_sdk::serde_json::{self, Value, json};
use near_sdk::{AccountId, PanicOnDefault, env, near, require};
use near_sdk_contract_tools::standard::nep141::{Nep141Controller, Nep141Mint};

use host::{ChainHost, ViewHost, rhythm_name};
use store::{LEDGER_RING, Rules, SHEET_RING};

#[derive(PanicOnDefault)]
#[near(contract_state)]
pub struct Contract {}

impl Contract {
    /// Loads the rulebook, runs one call against it and stores the result.
    /// A refusal panics with the rulebook's message, which reverts the call.
    fn with_rules<R>(&mut self, call: impl FnOnce(&mut Rules, &mut ChainHost<'_>) -> Result<R, Error>) -> R {
        let mut slot = store::rules();
        let mut rules = slot.read().unwrap_or_else(|| env::panic_str("The contract is not initialized"));
        let mut host = ChainHost::new(self, rules.config.bucket_blocks);
        let out = call(&mut rules, &mut host).unwrap_or_else(|e| env::panic_str(&e.to_string()));
        host.finish();
        slot.write(&rules);
        out
    }

    fn rules(&self) -> Rules {
        store::rules().read().unwrap_or_else(|| env::panic_str("The contract is not initialized"))
    }
}

fn to_json<T: near_sdk::serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or_else(|e| env::panic_str(&e.to_string()))
}

#[near]
impl Contract {
    /// Launches the coin: stores every parameter and mints the whole supply
    /// into the contract's own balance, where the rulebook splits it into
    /// the crash cart and the faucet. Called once, in the deploy transaction.
    #[init]
    #[private]
    pub fn new(config: Value) -> Self {
        let config: Config =
            serde_json::from_value(config).unwrap_or_else(|e| env::panic_str(&format!("Invalid config: {e}")));
        let supply = config.total_supply;
        let rules = Rules::new(config, env::block_height()).unwrap_or_else(|e| env::panic_str(e));
        let mut contract = Self {};
        Nep141Controller::mint(&mut contract, &Nep141Mint::new(supply, env::current_account_id()).memo("launch"))
            .unwrap_or_else(|e| env::panic_str(&e.to_string()));
        store::rules().write(&rules);
        contract
    }

    // ── Calls ──────────────────────────────────────────────────────────

    /// Pays `FAUCET_AMOUNT` once per registered account until the pool is empty.
    pub fn faucet_claim(&mut self) -> U128 {
        let who = env::predecessor_account_id();
        require!(store::account(&who).exists(), format!("The account {who} is not registered"));
        self.with_rules(|rules, host| rules.faucet_claim(host, env::block_height(), &who)).into()
    }

    /// Records a due arrest and returns its rhythm. Earns 10 work units if
    /// it comes before the first compression.
    pub fn call_code(&mut self) -> String {
        let who = env::predecessor_account_id();
        let rhythm = self.with_rules(|rules, host| rules.call_code(host, env::block_height(), &who));
        rhythm_name(rhythm).to_string()
    }

    /// One chest compression. Returns `counted`, `already_this_block` or `hands_off`.
    pub fn compress(&mut self) -> String {
        let who = env::predecessor_account_id();
        self.with_rules(|rules, host| rules.compress(host, env::block_height(), &who)).as_str().to_string()
    }

    /// One breath, inside the ventilation window, from anyone but the lead
    /// compressor. Returns `counted` or `already_this_block`.
    pub fn breathe(&mut self) -> String {
        let who = env::predecessor_account_id();
        self.with_rules(|rules, host| rules.breathe(host, env::block_height(), &who)).as_str().to_string()
    }

    /// After a round, on VF only. Returns whether the shock added perfusion.
    pub fn shock(&mut self) -> bool {
        let who = env::predecessor_account_id();
        self.with_rules(|rules, host| rules.shock(host, env::block_height(), &who))
    }

    /// After round 2 and every second round. Burns one dose from the cart.
    pub fn push_epi(&mut self) {
        let who = env::predecessor_account_id();
        self.with_rules(|rules, host| rules.push_epi(host, env::block_height(), &who))
    }

    /// Writes a death whose condition holds. Anyone may call it. Returns the death count.
    pub fn call_time_of_death(&mut self) -> u8 {
        self.with_rules(|rules, host| rules.call_time_of_death(host, env::block_height()))
    }

    /// Pays the caller's claimable bounty.
    pub fn claim_bounty(&mut self) -> U128 {
        let who = env::predecessor_account_id();
        self.with_rules(|rules, host| rules.claim_bounty(host, env::block_height(), &who)).into()
    }

    // ── Views ──────────────────────────────────────────────────────────

    /// Every parameter, as set at launch.
    pub fn get_config(&self) -> Value {
        to_json(&self.rules().config)
    }

    /// Everything the page draws, in one call, as of the current block.
    pub fn get_monitor(&self) -> Value {
        to_json(&self.rules().monitor(&ViewHost(self), env::block_height()))
    }

    /// The last 16 code sheets, oldest first.
    pub fn get_code_sheets(&self) -> Value {
        let filed = store::shelf().read().unwrap_or_default().sheets;
        let sheets: Vec<Value> = (filed.saturating_sub(SHEET_RING)..filed)
            .filter_map(|seq| store::sheet(seq).read())
            .map(|sheet| to_json(&sheet))
            .collect();
        Value::Array(sheets)
    }

    /// Burns and payments from the ledger ring, oldest first. Lines are
    /// numbered from the launch; the ring keeps the newest 256. `next` is
    /// the number to pass as `from` for the following page.
    pub fn get_ledger(&self, from: Option<u32>, limit: Option<u32>) -> Value {
        let filed = store::shelf().read().unwrap_or_default().ledger;
        let oldest = filed.saturating_sub(LEDGER_RING);
        let start = from.unwrap_or(oldest).clamp(oldest, filed);
        let end = start.saturating_add(limit.unwrap_or(64).min(128)).min(filed);
        let entries: Vec<Value> = (start..end)
            .filter_map(|seq| store::ledger_line(seq).read().map(|line| json!({ "seq": seq, "line": to_json(&line) })))
            .collect();
        json!({ "filed": filed, "oldest": oldest, "next": end, "entries": entries })
    }

    /// What `account_id` could claim now, including a bounty no call has written yet.
    pub fn get_claimable(&self, account_id: AccountId) -> U128 {
        self.rules().claimable_at(&ViewHost(self), env::block_height(), &account_id).into()
    }
}
