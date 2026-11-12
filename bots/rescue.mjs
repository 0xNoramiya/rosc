// bot-compressor and bot-airway: run a code when no human is watching.
//
//   node bots/rescue.mjs [deployment]        default: testnet-demo; waits for arrests until stopped
//   AIRWAY_ONLY=1 node bots/rescue.mjs       a human is on the chest: only give the breaths
//   ONCE=1 node bots/rescue.mjs              stop after one code
//
// One `compress()` goes out per block on a metronome, signed locally and not
// waited for. The breaths come from the other account, inside the ventilation
// window. After each round the bots take whatever the rhythm check offers,
// then swap the chest so nobody tires.
import { fileURLToPath } from "node:url";
import { Poller } from "./lib/coin.mjs";
import {
  Wallet, blockInfo, explorerTx, loadDeployment, measureBlockMs, panicText, phaseOf, rosc, sleep, statusOf, view,
} from "./lib/near.mjs";

/**
 * Plays the code that is running (or the next one) to its end. Returns as
 * soon as the code is over; `settled` resolves later, once every press has
 * been read back from the chain, with the landing figures.
 */
export async function runCode({ coin, team, poller, blockMs, events = null, log = () => {}, compress = true, breathe = true, timeoutMs = 900_000 }) {
  const [a, b] = team;
  let m = await poller.until((x) => statusOf(x) === "Arrest", { what: "an arrest", timeoutMs });
  const codeNo = m.code.code_no;
  const breathsPerCycle = m.config.breaths_per_cycle;
  await Promise.all([a.prime(), b.prime()]);

  const presses = [];
  const acks = [];
  const given = { shock: new Set(), epi: new Set() };

  const press = (wallet, method, code, after = () => {}) => {
    const entry = { t: Date.now(), who: wallet.accountId, method, round: code.round, cycle: code.cycle };
    presses.push(entry);
    const job = (async () => {
      const sent = await wallet.press(coin, method);
      Object.assign(entry, { t: sent.sentAt, hash: sent.hash });
      const outcome = await sent.ack;
      Object.assign(entry, {
        ok: outcome.ok,
        value: outcome.value,
        error: outcome.error ? panicText(outcome.error) : null,
        lost: Boolean(outcome.lost),
        gas: outcome.gas,
        tokens: outcome.tokens,
      });
      if (outcome.blockHash) {
        const info = await blockInfo(outcome.blockHash).catch(() => null);
        if (info) Object.assign(entry, { block: info.height, block_time: info.time });
      }
      after(entry);
      if (events) {
        const kind =
          method === "compress" ? (entry.value === "counted" ? "compression" : "missed_press")
          : method === "breathe" ? (entry.value === "counted" ? "breath" : "missed_breath")
          : method === "shock" ? (entry.ok ? "shock" : "refused_shock")
          : "epinephrine";
        events.add(kind, {
          who: entry.who, round: entry.round, cycle: entry.cycle, block: entry.block, block_time: entry.block_time,
          result: entry.ok ? entry.value : entry.error, tx: entry.hash,
        }, { t: entry.t });
      }
    })().catch((e) => log(`  a ${method} was lost: ${e.message}`));
    acks.push(job);
  };

  const started = Date.now();
  let lastRound = 0;
  for (let tick = 0; ; tick++) {
    m = poller.latest;
    if (statusOf(m) !== "Arrest" || m.code.code_no !== codeNo) break;
    if (Date.now() - started > timeoutMs) throw new Error("the code ran out of time");
    const code = m.code;
    if (code.round !== lastRound) {
      lastRound = code.round;
      log(`  round ${code.round}: perfusion ${code.perfusion / 100} of ${code.target ? code.target / 100 : "?"}, rhythm ${code.rhythm.toLowerCase()}`);
    }
    // The chest rotates every round.
    const lead = code.round % 2 === 1 ? a : b;
    const other = lead === a ? b : a;
    const phase = phaseOf(code);
    if (phase === "Compressions") {
      // The monitor is a block or two behind the presses already in the air.
      // Count those in, so the last few presses of a cycle are not wasted on a closed chest.
      const inFlight = presses.filter((p) => p.method === "compress" && p.round === code.round && p.cycle === code.cycle && Date.now() - p.t < 2.6 * blockMs).length;
      if (compress && code.cycle_compressions + inFlight < m.config.compressions_per_cycle) press(lead, "compress", code);
    } else if (phase === "Ventilation") {
      // Whoever did the most compressions may not take the airway.
      const giver = code.cycle_lead === other.accountId ? lead : other;
      // Breaths sent in the last few blocks have not reached the monitor yet.
      // One that is older and still not counted was lost: send another.
      const inFlight = presses.filter((p) => p.method === "breathe" && p.round === code.round && p.cycle === code.cycle && Date.now() - p.t < 3.5 * blockMs).length;
      if (breathe && code.cycle_lead !== giver.accountId && code.cycle_breaths + inFlight < breathsPerCycle) {
        press(giver, "breathe", code);
      }
    } else if (code.shock_open && !given.shock.has(code.round)) {
      given.shock.add(code.round);
      press(other, "shock", code, (e) => e.ok || given.shock.delete(code.round));
    } else if (code.epi_open && !given.epi.has(code.round)) {
      given.epi.add(code.round);
      press(other, "push_epi", code, (e) => e.ok || given.epi.delete(code.round));
    }
    const wait = started + (tick + 1) * blockMs - Date.now();
    if (wait > 0) await sleep(wait);
  }
  const perCycle = m.config.compressions_per_cycle;
  const settled = Promise.all(acks).then(() => {
    presses.sort((x, y) => x.t - y.t);
    return summarize(presses, perCycle);
  });
  return { codeNo, presses, settled };
}

/** Landing rate: how many presses counted, and how many blocks each cycle's compressions spanned. */
export function summarize(presses, perCycle) {
  const compress = presses.filter((p) => p.method === "compress");
  const counted = compress.filter((p) => p.value === "counted" && p.block).sort((x, y) => x.block - y.block);
  const cycles = [];
  for (let i = 0; i + perCycle <= counted.length; i += perCycle) {
    const span = counted[i + perCycle - 1].block - counted[i].block + 1;
    cycles.push({ first_block: counted[i].block, last_block: counted[i + perCycle - 1].block, span_blocks: span, landing_rate: perCycle / span });
  }
  const tally = (list, test) => list.filter(test).length;
  return {
    presses: compress.length,
    counted: counted.length,
    already_this_block: tally(compress, (p) => p.value === "already_this_block"),
    hands_off: tally(compress, (p) => p.value === "hands_off"),
    refused: tally(compress, (p) => p.ok === false && !p.lost),
    lost: tally(compress, (p) => p.lost),
    counted_share: compress.length ? counted.length / compress.length : null,
    cycles,
    mean_blocks_per_cycle: cycles.length ? cycles.reduce((s, c) => s + c.span_blocks, 0) / cycles.length : null,
    breaths: tally(presses, (p) => p.method === "breathe" && p.value === "counted"),
    doses: tally(presses, (p) => p.method === "push_epi" && p.ok),
    shocks: tally(presses, (p) => p.method === "shock" && p.ok),
  };
}

/** Collects whatever each wallet has earned. */
export async function claimAll(coin, wallets) {
  const claims = [];
  for (const wallet of wallets) {
    const due = BigInt(await view(coin, "get_claimable", { account_id: wallet.accountId }));
    if (due === 0n) continue;
    const outcome = await wallet.call(coin, "claim_bounty");
    claims.push({ who: wallet.accountId, amount: outcome.value, hash: outcome.hash, gas: outcome.gas });
  }
  return claims;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const deployment = loadDeployment(process.argv[2] || "testnet-demo");
  const coin = deployment.contract;
  const team = ["bot-compressor", "bot-airway"].map((role) => new Wallet(deployment.bots.find((b) => b.role === role).id));
  const airwayOnly = Boolean(process.env.AIRWAY_ONLY);
  const blockMs = await measureBlockMs();
  const poller = await new Poller(coin).start();
  console.log(`on call for ${coin}: one press per ${blockMs.toFixed(0)} ms${airwayOnly ? ", airway only" : ""}`);
  for (;;) {
    await poller.until((m) => statusOf(m) === "Arrest" || statusOf(m) === "Deceased", { timeoutMs: Infinity, what: "an arrest" });
    if (statusOf(poller.latest) === "Deceased") break;
    console.log(`CODE BLUE: ${poller.latest.code.rhythm.toLowerCase()} at block ${poller.latest.code.arrest_block}`);
    if (!airwayOnly && !poller.latest.code.called) {
      const called = await team[0].attempt(coin, "call_code");
      console.log(called.ok ? `  code called: ${explorerTx(called.hash)}` : `  ${panicText(called.error)}`);
    }
    const { settled } = await runCode({ coin, team, poller, blockMs, log: console.log, compress: !airwayOnly, timeoutMs: Infinity });
    await sleep(1_500);
    if (statusOf(poller.latest) === "Alive") {
      for (const c of await claimAll(coin, team)) console.log(`  ROSC. ${c.who.split(".")[0]} claimed ${rosc(c.amount)} ROSC: ${explorerTx(c.hash)}`);
    } else {
      console.log(`  the code ended without a pulse`);
    }
    const summary = await settled;
    console.log(`  ${summary.counted} of ${summary.presses} presses counted, ${summary.breaths} breaths, ${summary.doses} doses; 30 compressions took ${summary.cycles.map((c) => c.span_blocks).join(", ")} blocks`);
    if (process.env.ONCE) break;
  }
  poller.stop();
  if (process.env.ONCE) process.exit(0);
  console.log("the coin is deceased");
}
