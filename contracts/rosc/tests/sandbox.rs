//! The contract on a local sandbox chain: the acceptance list from the spec.
//!
//! The rules themselves are tested in `codeblue-core`. These tests check the
//! wiring: storage, fees through the transfer hook, refusal messages as a
//! client sees them, refunds, keys and the wasm's imports.
//!
//! Timing. An awaited sandbox transaction spans about ten blocks, so thirty
//! compressions cannot land one per block here. Tests that play a whole code
//! use a config with one compression per cycle, which makes every cycle a
//! perfect one whatever the block timing, and wide windows. Everything else
//! is the demo preset. `cargo-near` must be on PATH: the wasm is built here.

use std::sync::{Arc, OnceLock};

use codeblue_core::{Config, ROSC};
use near_api::types::transaction::actions::{Action, FunctionCallAction};
use near_api::types::transaction::result::ExecutionFinalResult;
use near_api::{Account, AccountId, Contract, NearGas, NearToken, NetworkConfig, Signer, Transaction};
use near_sdk::serde::de::DeserializeOwned;
use near_sdk::serde_json::{self, Value, json};
use testresult::TestResult;

const NO_PULSE: &str = "No pulse. Tokens cannot circulate without circulation. Start compressions.";

fn wasm() -> &'static [u8] {
    static WASM: OnceLock<Vec<u8>> = OnceLock::new();
    WASM.get_or_init(|| {
        let path = cargo_near_build::build_with_cli(Default::default()).expect("cargo near build");
        std::fs::read(path).expect("read wasm")
    })
}

fn receiver_wasm() -> &'static [u8] {
    static WASM: OnceLock<Vec<u8>> = OnceLock::new();
    WASM.get_or_init(|| {
        let opts = cargo_near_build::BuildOpts { manifest_path: Some("../receiver/Cargo.toml".into()), ..Default::default() };
        let path = cargo_near_build::build_with_cli(opts).expect("cargo near build receiver");
        std::fs::read(path).expect("read wasm")
    })
}

fn yocto(n: u128) -> NearToken {
    NearToken::from_yoctonear(n)
}

/// The demo preset with a silence long enough to set a scene in.
fn demo() -> Config {
    Config { arrest_blocks: 300, bucket_blocks: 100, refractory_blocks: 1, ..Config::demo() }
}

/// One compression per cycle and wide windows: a whole code, at sandbox pace.
fn ward() -> Config {
    Config {
        compressions_per_cycle: 1,
        vent_window_blocks: 60,
        max_pause_blocks: 400,
        no_flow_max_blocks: 600,
        lazarus_blocks: 60,
        ..demo()
    }
}

/// Every failed outcome in a transaction, as text.
fn failure_text(r: &ExecutionFinalResult) -> String {
    r.failures()
        .into_iter()
        .map(|o| format!("{:?}", o.clone().into_result().unwrap_err()))
        .collect::<Vec<_>>()
        .join("\n")
}

struct World {
    sandbox: near_sandbox::Sandbox,
    net: NetworkConfig,
    signer: Arc<Signer>,
}

impl World {
    async fn start() -> TestResult<Self> {
        let sandbox = near_sandbox::Sandbox::start_sandbox().await?;
        let net = NetworkConfig::from_rpc_url("sandbox", sandbox.rpc_addr.parse()?);
        let signer = Signer::from_secret_key(near_sandbox::config::DEFAULT_GENESIS_ACCOUNT_PRIVATE_KEY.parse()?)?;
        Ok(Self { sandbox, net, signer })
    }

    async fn create(&self, name: &str) -> TestResult<AccountId> {
        self.sandbox.create_account(name.parse()?).initial_balance(NearToken::from_near(50)).send().await?;
        Ok(name.parse()?)
    }

    /// Deploys the coin to `name` with `config`, in one transaction.
    async fn coin(&self, name: &str, config: &Config) -> TestResult<AccountId> {
        let id = self.create(name).await?;
        Contract::deploy(id.clone())
            .use_code(wasm().to_vec())
            .with_init_call("new", json!({ "config": serde_json::to_value(config)? }))?
            .with_signer(self.signer.clone())
            .send_to(&self.net)
            .await?
            .assert_success();
        Ok(id)
    }

    async fn call(&self, who: &AccountId, contract: &AccountId, method: &str, args: Value, deposit: NearToken) -> TestResult<ExecutionFinalResult> {
        Ok(Contract(contract.clone())
            .call_function(method, args)
            .transaction()
            .gas(NearGas::from_tgas(100))
            .deposit(deposit)
            .with_signer(who.clone(), self.signer.clone())
            .send_to(&self.net)
            .await?
            .into_full()
            .expect("a final result"))
    }

    /// A call that must succeed. Returns its JSON result.
    async fn ok<T: DeserializeOwned>(&self, who: &AccountId, coin: &AccountId, method: &str) -> TestResult<T> {
        let r = self.call(who, coin, method, json!({}), yocto(0)).await?;
        assert!(r.is_success() && r.failures().is_empty(), "{method} failed: {}", failure_text(&r));
        Ok(r.json()?)
    }

    /// A call that must succeed and returns nothing.
    async fn run(&self, who: &AccountId, coin: &AccountId, method: &str) -> TestResult {
        let r = self.call(who, coin, method, json!({}), yocto(0)).await?;
        assert!(r.is_success() && r.failures().is_empty(), "{method} failed: {}", failure_text(&r));
        Ok(())
    }

    /// A call that must be refused. Returns the failure as a client sees it.
    async fn refused(&self, who: &AccountId, coin: &AccountId, method: &str) -> TestResult<String> {
        let r = self.call(who, coin, method, json!({}), yocto(0)).await?;
        assert!(!r.is_success(), "{method} should have been refused");
        Ok(failure_text(&r))
    }

    async fn view<T: DeserializeOwned + Send + Sync>(&self, contract: &AccountId, method: &str, args: Value) -> TestResult<T> {
        Ok(Contract(contract.clone()).call_function(method, args).read_only().fetch_from(&self.net).await?.data)
    }

    async fn monitor(&self, coin: &AccountId) -> TestResult<Value> {
        self.view(coin, "get_monitor", json!({})).await
    }

    async fn balance(&self, coin: &AccountId, who: &AccountId) -> TestResult<u128> {
        let s: String = self.view(coin, "ft_balance_of", json!({ "account_id": who })).await?;
        Ok(s.parse()?)
    }

    async fn supply(&self, coin: &AccountId) -> TestResult<u128> {
        let s: String = self.view(coin, "ft_total_supply", json!({})).await?;
        Ok(s.parse()?)
    }

    async fn register(&self, coin: &AccountId, who: &AccountId) -> TestResult {
        let bounds: Value = self.view(coin, "storage_balance_bounds", json!({})).await?;
        let min: u128 = bounds["min"].as_str().expect("min").parse()?;
        let r = self.call(who, coin, "storage_deposit", json!({}), yocto(min)).await?;
        assert!(r.is_success(), "storage_deposit failed: {}", failure_text(&r));
        Ok(())
    }

    /// Registers `who` with the coin and claims the faucet.
    async fn scrub_in(&self, coin: &AccountId, who: &AccountId) -> TestResult {
        self.register(coin, who).await?;
        let paid: String = self.ok(who, coin, "faucet_claim").await?;
        assert_eq!(paid.parse::<u128>()?, 1_000_000 * ROSC);
        Ok(())
    }

    async fn transfer(&self, coin: &AccountId, from: &AccountId, to: &AccountId, amount: u128) -> TestResult<ExecutionFinalResult> {
        self.call(from, coin, "ft_transfer", json!({ "receiver_id": to, "amount": amount.to_string() }), yocto(1)).await
    }

    /// Skips ahead until the coin's monitor reports `status`.
    async fn wait_for(&self, coin: &AccountId, status: &str, blocks: u64) -> TestResult<Value> {
        self.sandbox.fast_forward(blocks).await?;
        for _ in 0..40 {
            let m = self.monitor(coin).await?;
            if status_of(&m) == status {
                return Ok(m);
            }
            self.sandbox.fast_forward(20).await?;
        }
        panic!("the coin never reached {status}: {}", self.monitor(coin).await?);
    }
}

fn status_of(monitor: &Value) -> String {
    match &monitor["status"] {
        Value::String(s) => s.to_lowercase(),
        Value::Object(o) => o.keys().next().expect("a status").to_lowercase(),
        other => panic!("unexpected status {other}"),
    }
}

fn amount(value: &Value) -> u128 {
    value.as_str().expect("an amount string").parse().expect("a number")
}

// ── Vitals and fees ────────────────────────────────────────────────────

#[tokio::test]
async fn transfers_beat_pay_fees_and_stop_without_a_pulse() -> TestResult {
    let w = World::start().await?;
    let coin = w.coin("coin.sandbox", &demo()).await?;
    let (alice, bob) = (w.create("alice.sandbox").await?, w.create("bob.sandbox").await?);
    w.scrub_in(&coin, &alice).await?;
    w.scrub_in(&coin, &bob).await?;

    // Launch: the whole supply sits in the contract, split by the rulebook.
    let m = w.monitor(&coin).await?;
    assert_eq!(amount(&m["pools"]["cart"]), 1_000_000_000 * ROSC);
    assert_eq!(amount(&m["pools"]["faucet"]), (1_500_000_000 - 2_000_000) * ROSC);
    let config: Value = w.view(&coin, "get_config", json!({})).await?;
    assert_eq!(config, serde_json::to_value(demo())?);

    // The sender pays the oxygen burn and the premium on top; the receiver
    // gets exactly what was sent.
    let supply = w.supply(&coin).await?;
    let r = w.transfer(&coin, &alice, &bob, 1_000 * ROSC).await?;
    assert!(r.is_success(), "{}", failure_text(&r));
    println!("ft_transfer through the hook: {} gas", r.total_gas_burnt.as_gas());
    assert_eq!(w.balance(&coin, &bob).await?, 1_001_000 * ROSC);
    assert_eq!(w.balance(&coin, &alice).await?, (1_000_000 - 1_010) * ROSC);
    assert_eq!(w.supply(&coin).await?, supply - 5 * ROSC);
    let logs = r.logs().join("\n");
    assert!(logs.contains(r#""event":"ft_burn""#) && logs.contains(r#""memo":"oxygen""#), "{logs}");
    assert!(logs.contains(r#""memo":"premium""#), "{logs}");

    let m = w.monitor(&coin).await?;
    assert_eq!(status_of(&m), "alive");
    assert_eq!(amount(&m["pools"]["cart"]), 1_000_000_005 * ROSC);
    assert_eq!(m["beats"].as_array().unwrap().len(), 1, "the transfer is a beat on the trace");
    assert_eq!(amount(&m["supply"]), supply - 5 * ROSC);

    // A self-transfer would be credited without being debited. It is refused.
    let r = w.transfer(&coin, &alice, &alice, ROSC).await?;
    assert!(failure_text(&r).contains("Sender and receiver must differ"));
    // An unregistered receiver is refused, as NEP-145 requires.
    let carol = w.create("carol.sandbox").await?;
    let r = w.transfer(&coin, &alice, &carol, ROSC).await?;
    assert!(failure_text(&r).contains("The account carol.sandbox is not registered"), "{}", failure_text(&r));
    // A rescue call on a living coin is refused.
    assert!(w.refused(&alice, &coin, "compress").await?.contains("Patient has a pulse. Stop compressions."));

    // Silence. Nothing has to call anything: transfers fail from that block.
    let m = w.wait_for(&coin, "arrest", 300).await?;
    assert_eq!(m["code"]["rhythm"], "Asystole");
    let r = w.transfer(&coin, &alice, &bob, ROSC).await?;
    let text = failure_text(&r);
    assert!(text.contains(NO_PULSE), "{text}");
    assert_eq!(w.balance(&coin, &bob).await?, 1_001_000 * ROSC);

    // A rescue call records the arrest.
    let rhythm: String = w.ok(&alice, &coin, "call_code").await?;
    assert_eq!(rhythm, "asystole");
    assert!(w.refused(&bob, &coin, "call_code").await?.contains("The code is already called."));

    // Two calls to compress() in one block count once.
    let press = || {
        Action::FunctionCall(Box::new(FunctionCallAction {
            method_name: "compress".into(),
            args: b"{}".to_vec(),
            gas: NearGas::from_tgas(30),
            deposit: yocto(0),
        }))
    };
    let r = Transaction::construct(alice.clone(), coin.clone())
        .add_action(press())
        .add_action(press())
        .with_signer(w.signer.clone())
        .send_to(&w.net)
        .await?
        .into_full()
        .expect("a final result");
    assert!(r.is_success(), "{}", failure_text(&r));
    let second: String = r.json()?;
    assert_eq!(second, "already_this_block");
    let m = w.monitor(&coin).await?;
    assert_eq!(m["code"]["cycle_compressions"], 1);
    assert!(m["code"]["first_compression_block"].is_u64());

    // Gas for one counted compression, and for one that does not count.
    let r = w.call(&bob, &coin, "compress", json!({}), yocto(0)).await?;
    assert_eq!(r.clone().json::<String>()?, "counted");
    println!("compress, counted: {} gas", r.total_gas_burnt.as_gas());
    assert!(r.total_gas_burnt < NearGas::from_tgas(10), "a compression fits a small gas budget");
    Ok(())
}

// ── A code, start to finish ────────────────────────────────────────────

/// Plays cycles until the round ends: one compression from `lead`, then the
/// breaths from `airway` if there is one, else the window is left to lapse.
async fn round(w: &World, coin: &AccountId, lead: &AccountId, airway: Option<&AccountId>, cycles: u8) -> TestResult {
    for _ in 0..cycles {
        // The previous window may still be open. Press until one counts.
        loop {
            let tap: String = w.ok(lead, coin, "compress").await?;
            if tap == "counted" {
                break;
            }
            w.sandbox.fast_forward(20).await?;
        }
        if let Some(airway) = airway {
            for _ in 0..2 {
                let tap: String = w.ok(airway, coin, "breathe").await?;
                assert_eq!(tap, "counted");
            }
        }
    }
    Ok(())
}

#[tokio::test]
async fn a_pair_reaches_rosc_and_both_claim() -> TestResult {
    let w = World::start().await?;
    let coin = w.coin("coin.sandbox", &ward()).await?;
    let (alice, bob) = (w.create("alice.sandbox").await?, w.create("bob.sandbox").await?);
    w.scrub_in(&coin, &alice).await?;
    w.scrub_in(&coin, &bob).await?;
    w.wait_for(&coin, "arrest", 300).await?;

    // The lead compressor may not take the airway.
    round(&w, &coin, &alice, Some(&bob), 1).await?;
    let tap: String = w.ok(&alice, &coin, "compress").await?;
    assert_eq!(tap, "counted");
    let text = w.refused(&alice, &coin, "breathe").await?;
    assert!(text.contains("You are on the chest. Someone else takes the airway."), "{text}");
    for _ in 0..2 {
        let _: String = w.ok(&bob, &coin, "breathe").await?;
    }
    // Round 1 is over: two perfect cycles, 100 points of the 200 and some for the delay.
    let m = w.monitor(&coin).await?;
    assert_eq!(m["code"]["round"], 2);
    assert_eq!(m["code"]["perfusion"], 10_000);
    let text = w.refused(&bob, &coin, "push_epi").await?;
    assert!(text.contains("Epinephrine is every 3 to 5 minutes. Not yet."), "{text}");
    let text = w.refused(&bob, &coin, "shock").await?;
    assert!(text.contains("Not a shockable rhythm. Resume compressions."), "{text}");
    let text = w.refused(&bob, &coin, "call_time_of_death").await?;
    assert!(text.contains("Still in the code. Keep going."), "{text}");

    // They swap, so nobody tires. Round 2, then a dose if the delay raised the target.
    round(&w, &coin, &bob, Some(&alice), 2).await?;
    let m = w.monitor(&coin).await?;
    if status_of(&m) == "arrest" {
        w.run(&alice, &coin, "push_epi").await?;
    }
    let m = w.monitor(&coin).await?;
    assert_eq!(status_of(&m), "alive", "{m}");
    assert_eq!(m["scars"], 1);
    assert_eq!(m["ejection_fraction"], 55);

    // Transfers work again, and both rescuers are paid from the crash cart.
    assert!(w.transfer(&coin, &alice, &bob, ROSC).await?.is_success());
    let sheets: Value = w.view(&coin, "get_code_sheets", json!({})).await?;
    let sheet = &sheets[0];
    assert_eq!(sheet["outcome"], "Rosc");
    assert_eq!(sheet["rounds"].as_array().unwrap().len(), 2);
    let bounty = amount(&sheet["bounty"]);
    assert!(bounty > 19_000_000 * ROSC && bounty <= 20_000_000 * ROSC, "2% of the cart: {bounty}");
    let mut claimed = 0;
    for who in [&alice, &bob] {
        let due: String = w.view(&coin, "get_claimable", json!({ "account_id": who })).await?;
        let before = w.balance(&coin, who).await?;
        let paid: String = w.ok(who, &coin, "claim_bounty").await?;
        assert_eq!(paid, due);
        assert!(amount(&json!(paid)) > 0);
        assert_eq!(w.balance(&coin, who).await?, before + paid.parse::<u128>()?, "a claim carries no fee");
        claimed += paid.parse::<u128>()?;
    }
    assert_eq!(claimed, bounty);
    assert!(w.refused(&alice, &coin, "claim_bounty").await?.contains("Nothing to claim."));

    let ledger: Value = w.view(&coin, "get_ledger", json!({})).await?;
    let lines = ledger["entries"].as_array().unwrap();
    assert!(lines.iter().any(|l| l["line"]["kind"] == "Bounty"), "{ledger}");
    assert!(lines.iter().any(|l| l["line"]["kind"] == json!({ "Paid": "Faucet" })), "{ledger}");
    Ok(())
}

#[tokio::test]
async fn one_rescuer_hands_only_needs_more_rounds() -> TestResult {
    let w = World::start().await?;
    let coin = w.coin("coin.sandbox", &ward()).await?;
    let alice = w.create("alice.sandbox").await?;
    w.scrub_in(&coin, &alice).await?;
    w.wait_for(&coin, "arrest", 300).await?;

    // 70, then 56 a round once tired, and a dose after round 2: three rounds
    // at least, where the pair needed two.
    for _ in 0..10 {
        round(&w, &coin, &alice, None, 2).await?;
        // Let the last window lapse, then take the dose if one is offered.
        w.sandbox.fast_forward(70).await?;
        let m = w.monitor(&coin).await?;
        if status_of(&m) != "arrest" {
            break;
        }
        if m["code"]["epi_open"] == true {
            w.run(&alice, &coin, "push_epi").await?;
        }
        if status_of(&w.monitor(&coin).await?) != "arrest" {
            break;
        }
    }
    let m = w.monitor(&coin).await?;
    assert_eq!(status_of(&m), "alive", "{m}");

    // Nobody wrote the ROSC yet if it came from a lapsed window. A claim does.
    let paid: String = w.ok(&alice, &coin, "claim_bounty").await?;
    assert!(paid.parse::<u128>()? > 0);
    let sheets: Value = w.view(&coin, "get_code_sheets", json!({})).await?;
    let rounds = sheets[0]["rounds"].as_array().unwrap();
    assert!(rounds.len() >= 3, "hands-only took {} rounds", rounds.len());
    assert_eq!(rounds[0]["quality_bps"], 7_000, "hands-only CPR scores 70%");
    assert_eq!(rounds[1]["quality_bps"], 5_600, "the same lead two rounds running scores 80% of that");
    Ok(())
}

// ── Rhythms ────────────────────────────────────────────────────────────

#[tokio::test]
async fn shock_works_on_vf_and_fails_on_asystole_and_pea() -> TestResult {
    let w = World::start().await?;
    // A busy final bucket is four beats here, so it fits sandbox pace.
    let config = Config { sudden_beats: 4, bucket_blocks: 300, ..ward() };
    let vf = w.coin("vf.sandbox", &config).await?;
    let pea = w.coin("pea.sandbox", &config).await?;
    let flat = w.coin("flat.sandbox", &config).await?;
    let (alice, bob) = (w.create("alice.sandbox").await?, w.create("bob.sandbox").await?);
    for coin in [&vf, &pea, &flat] {
        w.scrub_in(coin, &alice).await?;
        w.scrub_in(coin, &bob).await?;
    }
    // PEA: one beat, then transfers too small to carry a pulse.
    assert!(w.transfer(&pea, &alice, &bob, ROSC).await?.is_success());
    for _ in 0..3 {
        assert!(w.transfer(&pea, &alice, &bob, 1).await?.is_success());
    }
    // VF: a burst of beats, then silence.
    for _ in 0..2 {
        assert!(w.transfer(&vf, &alice, &bob, ROSC).await?.is_success());
        assert!(w.transfer(&vf, &bob, &alice, ROSC).await?.is_success());
    }
    let m = w.wait_for(&vf, "arrest", 300).await?;
    assert_eq!(m["code"]["rhythm"], "Vf");
    assert_eq!(w.wait_for(&pea, "arrest", 20).await?["code"]["rhythm"], "Pea");
    assert_eq!(w.wait_for(&flat, "arrest", 20).await?["code"]["rhythm"], "Asystole");

    for coin in [&pea, &flat] {
        let text = w.refused(&alice, coin, "shock").await?;
        assert!(text.contains("Not a shockable rhythm. Resume compressions."), "{text}");
    }

    // VF: no shock before the rhythm check, one good round, then the shock.
    let rhythm: String = w.ok(&alice, &vf, "call_code").await?;
    assert_eq!(rhythm, "vf");
    let text = w.refused(&alice, &vf, "shock").await?;
    assert!(text.contains("Shocks follow the rhythm check. Resume compressions."), "{text}");
    round(&w, &vf, &alice, Some(&bob), 2).await?;
    let m = w.monitor(&vf).await?;
    assert_eq!(m["code"]["shock_open"], true, "{m}");
    let tap: String = w.ok(&alice, &vf, "compress").await?;
    assert_eq!(tap, "hands_off", "hands off during the rhythm check");
    let worked: bool = w.ok(&bob, &vf, "shock").await?;
    assert!(worked);
    let m = w.monitor(&vf).await?;
    assert_eq!(status_of(&m), "alive", "100 from the round and 150 from the shock: {m}");
    Ok(())
}

// ── Death ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn three_unanswered_codes_end_in_deceased() -> TestResult {
    let w = World::start().await?;
    let config = Config { no_flow_max_blocks: 200, lazarus_blocks: 200, ..demo() };
    let coin = w.coin("coin.sandbox", &config).await?;
    let (alice, bob) = (w.create("alice.sandbox").await?, w.create("bob.sandbox").await?);
    w.scrub_in(&coin, &alice).await?;
    w.scrub_in(&coin, &bob).await?;
    let supply = w.supply(&coin).await?;

    // Death cannot be called on a living coin, nor early in a code.
    let text = w.refused(&alice, &coin, "call_time_of_death").await?;
    assert!(text.contains("Patient has a pulse. There is no death to call."), "{text}");
    w.wait_for(&coin, "arrest", 300).await?;
    let text = w.refused(&alice, &coin, "call_time_of_death").await?;
    assert!(text.contains("Still in the code. Keep going."), "{text}");

    // Nobody answers. The monitor shows each step without any call.
    let m = w.wait_for(&coin, "morgue", 200).await?;
    assert_eq!((m["deaths"].as_u64(), m["scars"].as_u64()), (Some(1), Some(2)));
    let m = w.wait_for(&coin, "deceased", 2 * 401 - 100).await?;
    assert_eq!(m["deaths"], 3);
    assert_eq!(amount(&m["pools"]["cart"]), 0, "the estate is burned");

    // Anyone may write it. One call records all three deaths.
    let carol = w.create("carol.sandbox").await?;
    let r = w.call(&carol, &coin, "call_time_of_death", json!({}), yocto(0)).await?;
    assert!(r.is_success(), "{}", failure_text(&r));
    assert_eq!(r.clone().json::<u8>()?, 3);
    let logs = r.logs().join("\n");
    for event in ["arrest", "time_of_death", "deceased"] {
        assert!(logs.contains(&format!(r#""standard":"codeblue","version":"1.0.0","event":"{event}""#)), "{logs}");
    }
    assert!(logs.contains(r#""memo":"estate""#), "{logs}");
    assert_eq!(w.supply(&coin).await?, supply - 1_000_000_000 * ROSC, "the whole cart is gone");
    let block = m["status"]["Deceased"]["block"].as_u64().unwrap();

    // Every later call fails with the time of death.
    let gone = format!("Time of death was called at block {block}.");
    for method in ["compress", "breathe", "shock", "push_epi", "call_code", "call_time_of_death", "claim_bounty", "faucet_claim"] {
        let text = w.refused(&alice, &coin, method).await?;
        assert!(text.contains(&gone), "{method}: {text}");
    }
    let r = w.transfer(&coin, &alice, &bob, ROSC).await?;
    assert!(failure_text(&r).contains(&gone), "{}", failure_text(&r));
    let sheets: Value = w.view(&coin, "get_code_sheets", json!({})).await?;
    assert_eq!(sheets.as_array().unwrap().len(), 3);
    Ok(())
}

// ── Refunds ────────────────────────────────────────────────────────────

#[tokio::test]
async fn transfer_call_refunds_leave_supply_consistent() -> TestResult {
    let w = World::start().await?;
    let coin = w.coin("coin.sandbox", &demo()).await?;
    let alice = w.create("alice.sandbox").await?;
    w.scrub_in(&coin, &alice).await?;
    let pool = w.create("pool.sandbox").await?;
    Contract::deploy(pool.clone())
        .use_code(receiver_wasm().to_vec())
        .without_init_call()
        .with_signer(w.signer.clone())
        .send_to(&w.net)
        .await?
        .assert_success();
    w.register(&coin, &pool).await?;

    // (what the receiver does, what it keeps)
    for (msg, kept) in [("keep", 1_000), ("half", 500), ("all", 0), ("panic", 0)] {
        let (a0, p0, s0) = (w.balance(&coin, &alice).await?, w.balance(&coin, &pool).await?, w.supply(&coin).await?);
        let cart0 = amount(&w.monitor(&coin).await?["pools"]["cart"]);
        let r = w
            .call(&alice, &coin, "ft_transfer_call", json!({ "receiver_id": pool, "amount": (1_000 * ROSC).to_string(), "msg": msg }), yocto(1))
            .await?;
        assert!(r.is_success(), "{msg}: {}", failure_text(&r));
        let used: String = r.clone().json()?;
        println!("ft_transfer_call, receiver '{msg}': {} gas", r.total_gas_burnt.as_gas());
        assert_eq!(used.parse::<u128>()?, kept * ROSC, "{msg}");
        // The fees are charged once, on the way out, and are not refunded.
        assert_eq!(w.balance(&coin, &alice).await?, a0 - (kept + 10) * ROSC, "{msg}: sender");
        assert_eq!(w.balance(&coin, &pool).await?, p0 + kept * ROSC, "{msg}: receiver");
        assert_eq!(w.supply(&coin).await?, s0 - 5 * ROSC, "{msg}: only the oxygen burn leaves supply");
        assert_eq!(amount(&w.monitor(&coin).await?["pools"]["cart"]), cart0 + 5 * ROSC, "{msg}: premium");
    }

    // The books: every balance, the contract's included, adds up to the supply.
    let m = w.monitor(&coin).await?;
    let held = amount(&m["pools"]["cart"]) + amount(&m["pools"]["faucet"]) + amount(&m["pools"]["claimable"]);
    assert_eq!(w.balance(&coin, &coin).await?, held);
    assert_eq!(w.balance(&coin, &alice).await? + w.balance(&coin, &pool).await? + held, w.supply(&coin).await?);
    Ok(())
}

// ── No admin ───────────────────────────────────────────────────────────

/// The names of the host functions a wasm imports.
fn wasm_imports(wasm: &[u8]) -> Vec<String> {
    fn leb(data: &[u8], at: &mut usize) -> usize {
        let (mut value, mut shift) = (0usize, 0);
        loop {
            let byte = data[*at];
            *at += 1;
            value |= ((byte & 0x7f) as usize) << shift;
            shift += 7;
            if byte < 0x80 {
                return value;
            }
        }
    }
    let mut names = Vec::new();
    let mut at = 8;
    while at < wasm.len() {
        let id = wasm[at];
        at += 1;
        let size = leb(wasm, &mut at);
        let end = at + size;
        if id == 2 {
            for _ in 0..leb(wasm, &mut at) {
                let len = leb(wasm, &mut at);
                at += len;
                let len = leb(wasm, &mut at);
                names.push(String::from_utf8(wasm[at..at + len].to_vec()).unwrap());
                at += len;
                let kind = wasm[at];
                at += 1;
                assert_eq!(kind, 0, "a contract imports only functions");
                leb(wasm, &mut at);
            }
        }
        at = end;
    }
    names
}

#[test]
fn the_wasm_cannot_change_its_own_account() {
    let imports = wasm_imports(wasm());
    println!("{} imports: {}", imports.len(), imports.join(" "));
    assert!(imports.contains(&"storage_write".to_string()) && imports.contains(&"promise_create".to_string()));
    let forbidden = ["deploy", "add_key", "add_gas_key", "delete_key", "delete_account", "global_contract", "stake"];
    let found: Vec<_> = imports.iter().filter(|name| forbidden.iter().any(|word| name.contains(word))).collect();
    assert!(found.is_empty(), "the wasm can deploy code, change keys or delete the account: {found:?}");
}

#[tokio::test]
async fn the_account_has_no_keys_after_setup_and_storage_fits_the_fee() -> TestResult {
    let w = World::start().await?;
    let coin = w.coin("coin.sandbox", &demo()).await?;

    // Setup ends by deleting the deployer's key.
    let keys = Account(coin.clone()).list_keys().fetch_from(&w.net).await?.data;
    assert_eq!(keys.len(), 1);
    Account(coin.clone()).delete_key(w.signer.get_public_key().await?).with_signer(w.signer.clone()).send_to(&w.net).await?.assert_success();
    let keys = Account(coin.clone()).list_keys().fetch_from(&w.net).await?.data;
    assert!(keys.is_empty(), "the contract account has zero access keys");
    let as_contract = Contract(coin.clone())
        .call_function("new", json!({ "config": serde_json::to_value(demo())? }))
        .transaction()
        .with_signer(coin.clone(), w.signer.clone())
        .send_to(&w.net)
        .await;
    assert!(as_contract.is_err(), "nobody can sign as the contract any more");

    // The coin still works, and a second `new` from outside is refused.
    let alice = w.create("alice.sandbox").await?;
    let r = w.call(&alice, &coin, "new", json!({ "config": serde_json::to_value(demo())? }), yocto(0)).await?;
    assert!(!r.is_success());

    // Storage: the fixed fee covers the longest account id, and a deposit
    // above it, or a second one, comes back.
    let long = w.create(&format!("{}.sandbox", "a".repeat(56))).await?;
    assert_eq!(long.as_str().len(), 64);
    let bob = w.create("bob.sandbox").await?;
    let usage = async || -> TestResult<u64> { Ok(Account(coin.clone()).view().fetch_from(&w.net).await?.data.storage_usage) };
    let before = usage().await?;
    for who in [&long, &bob] {
        let near_before = Account(who.clone()).view().fetch_from(&w.net).await?.data.amount;
        let bytes_before = usage().await?;
        let r = w.call(who, &coin, "storage_deposit", json!({}), NearToken::from_near(1)).await?;
        assert!(r.is_success(), "{}", failure_text(&r));
        // The account's record is 59 bytes and its id, and 8 more once it
        // has a last beat. Its balance adds 66 and the id again.
        let record = usage().await? - bytes_before;
        let id = who.as_str().len() as u64;
        let most = record + 8 + 66 + id;
        println!("{who}: record {record} bytes; at most {most} bytes with a beat and a balance");
        assert_eq!(record, 59 + id);
        assert!(most as u128 <= 280, "the fee covers the record and the balance");
        let r = w.call(who, &coin, "storage_deposit", json!({}), NearToken::from_near(1)).await?;
        assert!(r.is_success(), "a second deposit is refunded, not refused: {}", failure_text(&r));
        let _: String = w.ok(who, &coin, "faucet_claim").await?;
        let spent = near_before.saturating_sub(Account(who.clone()).view().fetch_from(&w.net).await?.data.amount);
        assert!(spent < NearToken::from_millinear(10), "{who} spent {spent} on storage and gas");
    }
    assert!(w.transfer(&coin, &long, &bob, 1_000 * ROSC).await?.is_success());
    let used = usage().await? - before;
    let bounds: Value = w.view(&coin, "storage_balance_bounds", json!({})).await?;
    let fee: u128 = bounds["min"].as_str().unwrap().parse()?;
    println!("two accounts use {used} bytes; the fee covers {} each", fee / 10u128.pow(19));
    assert_eq!(bounds["min"], bounds["max"]);
    // The ledger lines these calls filed are the coin's own, not the accounts'.
    assert!(used as u128 <= 2 * (fee / 10u128.pow(19)) + 400, "{used} bytes");

    // The faucet pays registered accounts only, and never twice.
    let dave = w.create("dave.sandbox").await?;
    let text = w.refused(&dave, &coin, "faucet_claim").await?;
    assert!(text.contains("The account dave.sandbox is not registered"), "{text}");
    let text = w.refused(&bob, &coin, "faucet_claim").await?;
    assert!(text.contains("The faucet pays once per account."), "{text}");
    // Nor by emptying the account, closing it and opening it again.
    let all = w.balance(&coin, &long).await?;
    let spendable = all * 10_000 / 10_100;
    assert!(w.transfer(&coin, &long, &bob, spendable).await?.is_success());
    let dust = w.balance(&coin, &long).await?;
    if dust > 0 {
        assert!(w.transfer(&coin, &long, &bob, dust).await?.is_success());
    }
    assert_eq!(w.balance(&coin, &long).await?, 0);
    let r = w.call(&long, &coin, "storage_unregister", json!({}), yocto(1)).await?;
    assert!(failure_text(&r).contains("An account that took the faucet stays registered"), "{}", failure_text(&r));

    // Amounts are positive, and a transfer-call cannot target the coin itself.
    let r = w.transfer(&coin, &bob, &long, 0).await?;
    assert!(failure_text(&r).contains("The amount should be a positive number"), "{}", failure_text(&r));
    let r = w
        .call(&bob, &coin, "ft_transfer_call", json!({ "receiver_id": coin, "amount": ROSC.to_string(), "msg": "" }), yocto(1))
        .await?;
    assert!(failure_text(&r).contains("Use ft_transfer to give to the crash cart"), "{}", failure_text(&r));
    // A plain transfer to the coin is a gift to the cart, and the books still balance.
    let cart = amount(&w.monitor(&coin).await?["pools"]["cart"]);
    assert!(w.transfer(&coin, &bob, &coin, 100 * ROSC).await?.is_success());
    let m = w.monitor(&coin).await?;
    assert_eq!(amount(&m["pools"]["cart"]), cart + 100 * ROSC + ROSC / 2);
    assert_eq!(
        w.balance(&coin, &coin).await?,
        amount(&m["pools"]["cart"]) + amount(&m["pools"]["faucet"]) + amount(&m["pools"]["claimable"])
    );

    // An empty account can close and get its fee back; one with tokens cannot.
    let r = w.call(&bob, &coin, "storage_unregister", json!({}), yocto(1)).await?;
    assert!(failure_text(&r).contains("Can't unregister an account that holds tokens"), "{}", failure_text(&r));
    let carol = w.create("carol.sandbox").await?;
    w.register(&coin, &carol).await?;
    let r = w.call(&carol, &coin, "storage_unregister", json!({}), yocto(1)).await?;
    assert!(r.is_success() && r.clone().json::<bool>()?);
    let gone: Option<Value> = w.view(&coin, "storage_balance_of", json!({ "account_id": carol })).await?;
    assert!(gone.is_none());
    Ok(())
}
