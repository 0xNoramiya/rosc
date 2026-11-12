// `make demo`: the spec's demo script, played on NEAR testnet.
//
//   node bots/demo.mjs
//
//   HUMAN=1        a person does the CPR on the page; the bots only watch
//   AIRWAY=1       with HUMAN=1: the airway bot gives the breaths
//   FRESH=1        deploy new coins even if usable ones are on file
//   HEADSTONE=reuse  accept a headstone coin whose deaths are already written
//   HOLD_SECONDS   how long the holder bots run first (default 30)
//
// Exits 0 only if every step did what the script says. Writes
// bots/out/events.json (a wall-clock log for the video edit) and
// bots/out/gas.json (what each call cost).
import path from "node:path";
import { EventLog, Poller, describe, loadJson, points, sheetLines } from "./lib/coin.mjs";
import { deploy } from "./deploy.mjs";
import { runHolders } from "./holders.mjs";
import { runPacer } from "./pacer.mjs";
import { Pager, poster } from "./pager.mjs";
import { claimAll, runCode } from "./rescue.mjs";
import {
  DEPLOYMENTS, ONE, OUT, Wallet, blockInfo, explorerAccount, explorerTx, hasKey, measureBlockMs, panicText, rosc,
  rpcStats, sleep, statusOf, tagOf, view, viewAccount, writeJson,
} from "./lib/near.mjs";

const NO_PULSE = "No pulse. Tokens cannot circulate without circulation. Start compressions.";
const NOT_SHOCKABLE = "Not a shockable rhythm. Resume compressions.";
const HUMAN = Boolean(process.env.HUMAN);
const FRESH = Boolean(process.env.FRESH);
const HEADSTONE_LIFE = { no_flow_max_blocks: 60, lazarus_blocks: 20 };

const say = (text = "") => console.log(text);
const step = (n, title) => say(`\n${n}. ${title}`);
let checks = 0;
function check(condition, what) {
  if (!condition) throw new Error(`step failed: ${what}`);
  checks++;
  say(`   ok  ${what}`);
}
const short = (id) => id.split(".")[0];
const stats = (list) => {
  const v = list.map(Number).sort((a, b) => a - b);
  if (!v.length) return null;
  return { n: v.length, median: v[Math.floor(v.length / 2)], min: v[0], max: v[v.length - 1], median_tgas: Number((v[Math.floor(v.length / 2)] / 1e12).toFixed(3)) };
};

// ── Which coins ────────────────────────────────────────────────────────

async function usable(name, test) {
  const record = loadJson(path.join(DEPLOYMENTS, `${name}.json`));
  if (!record || FRESH) return null;
  if (!record.bots.every((b) => hasKey(b.id))) return null; // deployed from another machine
  const monitor = await view(record.contract, "get_monitor").catch(() => null);
  return monitor && (await test(record, monitor)) ? record : null;
}

async function demoCoin() {
  const found = await usable("testnet-demo", (r, m) => statusOf(m) === "Alive" && m.deaths === 0 && m.scars <= 6);
  if (found) say(`using the demo coin on file: ${found.contract}`);
  return found || deploy("testnet-demo");
}

async function headstoneCoin() {
  const found = await usable("testnet-headstone", async (r) => {
    const sheets = await view(r.contract, "get_code_sheets");
    return process.env.HEADSTONE === "reuse" || sheets.length === 0; // nobody has written its deaths yet
  });
  if (found) say(`using the headstone coin on file: ${found.contract}`);
  return (
    found ||
    deploy("testnet-headstone", {
      overrides: HEADSTONE_LIFE,
      team: "minimal",
      note: "Demo preset with a shortened life, so three unanswered codes take about three minutes: NO_FLOW_MAX_BLOCKS 60 instead of 1,000 and LAZARUS_BLOCKS 20 instead of 50.",
    })
  );
}

// ── The life ───────────────────────────────────────────────────────────

async function life(demo, events, gas) {
  const coin = demo.contract;
  const wallet = (role) => new Wallet(demo.bots.find((b) => b.role === role).id);
  const holders = demo.bots.filter((b) => b.role.startsWith("bot-holder")).map((b) => new Wallet(b.id));
  const [pacer, compressor, airway] = [wallet("bot-pacer"), wallet("bot-compressor"), wallet("bot-airway")];
  const blockMs = await measureBlockMs();
  const poller = await new Poller(coin, 600).start();
  const forward = poster();
  const pager = new Pager({ contract: coin, blockMs, post: (text) => forward(text) });
  poller.on((m) => pager.observe(m));
  const config = poller.latest.config;
  say(`\ncoin ${coin}  ${explorerAccount(coin)}`);
  say(`one block every ${blockMs.toFixed(0)} ms; the coin is ${describe(poller.latest)}`);

  // 1 ────────────────────────────────────────────────────────────────
  step(1, "Holder bots run: sinus rhythm, oxygen burn ticking");
  const before = poller.latest;
  check(statusOf(before) === "Alive", "the coin has a pulse");
  let running = true;
  const senders = new Set();
  const holding = runHolders({
    coin,
    wallets: holders,
    running: () => running,
    onBeat: (o, from, to, amount) => {
      senders.add(from.accountId);
      gas.ft_transfer.push(o.gas);
      events.add("beat", { who: from.accountId, to: to.accountId, amount: amount.toString(), tag: tagOf(from.accountId), tx: o.hash }, { blockHash: o.blockHash });
      say(`   beat  ${short(from.accountId)} -> ${short(to.accountId)}  ${rosc(amount)} ROSC  ${explorerTx(o.hash)}`);
    },
  });
  await sleep(Number(process.env.HOLD_SECONDS || 30) * 1000);
  running = false;
  const sent = await holding;
  await poller.next();
  const after = await poller.next();
  const tags = new Set(holders.map((h) => tagOf(h.accountId)));
  const traced = after.beats.filter((b) => b.block > before.block && tags.has(b.tag));
  check(sent >= 6 && traced.length >= sent - 1, `${sent} transfers, ${traced.length} of them beats on the monitor's trace`);
  check(new Set(traced.map((b) => b.tag)).size >= 3, `${new Set(traced.map((b) => b.tag)).size} different senders: a sinus rhythm, not a paced one`);
  const burned = BigInt(after.totals.burned_oxygen) - BigInt(before.totals.burned_oxygen);
  check(burned > 0n && BigInt(before.supply) - BigInt(after.supply) === burned, `oxygen burn: supply fell by ${rosc(burned, 4)} ROSC, to ${rosc(after.supply, 4)}`);

  // 2 ────────────────────────────────────────────────────────────────
  step(2, "A transfer from one more wallet lands as a beat on the trace");
  const extra = await pacer.transferRosc(coin, holders[0].accountId, 42n * ONE);
  gas.ft_transfer.push(extra.gas);
  const landed = await blockInfo(extra.blockHash);
  events.add("beat", { who: pacer.accountId, to: holders[0].accountId, amount: (42n * ONE).toString(), tag: tagOf(pacer.accountId), tx: extra.hash, block: landed.height, block_time: landed.time });
  say(`   ${short(pacer.accountId)} sent 42 ROSC in block ${landed.height}: ${explorerTx(extra.hash)}`);
  const withBeat = await poller.until((m) => m.block > landed.height, { what: "the next block" });
  check(withBeat.beats.some((b) => b.block === landed.height && b.tag === tagOf(pacer.accountId)), `the trace has a beat at block ${landed.height} tagged ${tagOf(pacer.accountId)}`);
  check(withBeat.last_beat_block === landed.height, "it reset the arrest clock");

  // 3 ────────────────────────────────────────────────────────────────
  step(3, "The bots stop. The countdown runs out");
  say(`   ${config.arrest_blocks} blocks of silence stop the heart: about ${Math.round((config.arrest_blocks * blockMs) / 1000)} s`);
  const arrested = await poller.until((m) => statusOf(m) === "Arrest", { timeoutMs: config.arrest_blocks * blockMs + 90_000, what: "the arrest" });
  events.add("arrest", { block: arrested.code.arrest_block, rhythm: arrested.code.rhythm.toLowerCase(), code: arrested.code.code_no });
  check(arrested.code.arrest_block === landed.height + config.arrest_blocks + 1, `arrest at block ${arrested.code.arrest_block}, ${config.arrest_blocks} blocks after the last beat, with no call from anyone`);
  const frozen = await holders[1].attempt(coin, "ft_transfer", { receiver_id: holders[2].accountId, amount: ONE.toString() }, { deposit: 1n });
  say(`   a transfer now fails: ${explorerTx(frozen.hash)}`);
  say(`   the chain says: ${frozen.error}`);
  check(!frozen.ok && panicText(frozen.error) === NO_PULSE, "refused with the no-pulse message");

  // 4 ────────────────────────────────────────────────────────────────
  step(4, "The pager posts CODE BLUE");
  await pager.busy;
  for (const p of pager.posts) say(`   ${p.kind}: ${p.text}`);
  check(pager.posts.some((p) => p.kind === "bradycardia"), "it warned at half the window");
  check(pager.posts.some((p) => p.kind === "code_blue" && p.text.startsWith("CODE BLUE") && p.text.includes("#code")), "it posted CODE BLUE with the rhythm and a link to the code screen");

  // 5 and 6 ──────────────────────────────────────────────────────────
  step(5, HUMAN ? "Scrub in on the page and run the code" : "The rescue bots run the code");
  const nearBefore = { [compressor.accountId]: BigInt((await viewAccount(compressor.accountId)).amount), [airway.accountId]: BigInt((await viewAccount(airway.accountId)).amount) };
  if (!HUMAN) {
    const called = await compressor.call(coin, "call_code");
    gas.call_code.push(called.gas);
    say(`   ${short(compressor.accountId)} called the code: ${called.value}  ${explorerTx(called.hash)}`);
    check(called.value === arrested.code.rhythm.toLowerCase(), "the recorded rhythm matches the monitor");
  } else {
    say(`   open ${pager.pageUrl}?contract=${coin}#code and start compressions`);
  }
  step(6, "A shock on a flatline is refused");
  if (arrested.code.rhythm === "Vf") {
    say("   the arrest is VF this time, which is shockable: nothing to refuse");
  } else {
    const shock = await airway.attempt(coin, "shock");
    events.add("refused_shock", { who: airway.accountId, result: panicText(shock.error), tx: shock.hash }, { blockHash: shock.blockHash });
    say(`   ${explorerTx(shock.hash)}`);
    say(`   the chain says: ${shock.error}`);
    check(!shock.ok && panicText(shock.error) === NOT_SHOCKABLE, "refused with the not-shockable message");
  }
  say(HUMAN ? "   waiting for ROSC" : "   30 compressions on the metronome, two breaths from the second account, a rhythm check");
  const code = await runCode({
    coin, team: [compressor, airway], poller, blockMs, events, log: say,
    compress: !HUMAN, breathe: !HUMAN || Boolean(process.env.AIRWAY), timeoutMs: HUMAN ? 1_800_000 : 600_000,
  });

  // 7 ────────────────────────────────────────────────────────────────
  step(7, "ROSC");
  const alive = await poller.until((m) => statusOf(m) !== "Arrest", { timeoutMs: 60_000, what: "the code to end" });
  check(statusOf(alive) === "Alive", `return of spontaneous circulation at block ${alive.alive_since_block}`);
  events.add("rosc", { block: alive.alive_since_block, code: code.codeNo, scars: alive.scars });
  const again = await holders[1].attempt(coin, "ft_transfer", { receiver_id: holders[2].accountId, amount: (7n * ONE).toString() }, { deposit: 1n });
  say(`   ${again.ok ? "a transfer went through" : again.error}: ${explorerTx(again.hash)}`);
  check(again.ok, "transfers work again");
  // From here to the end of the run a pacemaker keeps the pulse going:
  // reading every press back takes longer than the coin can stay silent.
  let pacing = true;
  const paced = runPacer({ coin, wallets: [pacer, wallet("bot-pacer-2")], running: () => pacing });
  const claims = await claimAll(coin, [compressor, airway]);
  for (const c of claims) {
    gas.claim_bounty.push(c.gas);
    say(`   ${short(c.who)} claimed ${rosc(c.amount)} ROSC  ${explorerTx(c.hash)}`);
  }
  const sheet = (await view(coin, "get_code_sheets")).find((x) => x.code_no === code.codeNo);
  check(sheet && sheet.outcome === "Rosc", "the code sheet is on file");
  for (const line of sheetLines(sheet, short)) say(`   ${line}`);
  const paid = sheet.shares.reduce((sum, [, share]) => sum + BigInt(share), 0n);
  check(paid === BigInt(sheet.bounty) && paid > 0n, `the bounty of ${rosc(sheet.bounty)} ROSC is split by work units`);
  if (!HUMAN) {
    check(claims.length === 2 && claims.every((c) => sheet.shares.some(([who, share]) => who === c.who && share === c.amount)), "both rescuers claimed exactly their share");
  }
  const s = await code.settled;
  if (!HUMAN) {
    say(`   ${s.counted} of ${s.presses} presses counted (${s.already_this_block} landed in a block that already had one, ${s.hands_off} during a window, ${s.refused + s.lost} lost or refused)`);
    for (const c of s.cycles) say(`   30 compressions in ${c.span_blocks} blocks (${c.first_block} to ${c.last_block})`);
    check(s.cycles.length >= 2 && s.breaths >= 2, `${s.cycles.length} cycles of 30 compressions and ${s.breaths} breaths landed`);
  }
  await pager.busy;
  const roscPost = pager.posts.find((p) => p.kind === "rosc");
  if (roscPost) say(`   pager: ${roscPost.text}`);
  check(Boolean(roscPost), "the pager posted the ROSC");
  poller.stop();

  // What it cost.
  for (const p of code.presses) {
    if (!p.ok || p.gas === undefined) continue;
    if (p.method === "compress") gas[p.value === "counted" ? "compress_counted" : `compress_${p.value}`]?.push(p.gas);
    if (p.method === "breathe" && p.value === "counted") gas.breathe.push(p.gas);
    if (p.method === "push_epi") gas.push_epi.push(p.gas);
  }
  const spent = {};
  for (const w of [compressor, airway]) spent[w.accountId] = nearBefore[w.accountId] - BigInt((await viewAccount(w.accountId)).amount);
  return { coin, blockMs, code: s, sheet, near_spent: spent, pager: pager.posts, stopPacing: async () => { pacing = false; return paced; } };
}

// ── The death ──────────────────────────────────────────────────────────

async function death(headstone, events, gas) {
  const coin = headstone.contract;
  const [a, b] = headstone.bots.map((bot) => new Wallet(bot.id));
  const blockMs = await measureBlockMs();
  step(8, "The second deployment: nobody answers three codes");
  say(`   coin ${coin}  ${explorerAccount(coin)}`);
  say(`   ${headstone.note || `overrides ${JSON.stringify(headstone.overrides)}`}`);
  const poller = await new Poller(coin, 2_000).start();
  const pager = new Pager({ contract: coin, blockMs, post: poster() });
  poller.on((m) => pager.observe(m));
  say(`   the coin is ${describe(poller.latest)}`);
  const c = poller.latest.config;
  const life = c.arrest_blocks + c.max_deaths * (c.no_flow_max_blocks + 1) + (c.max_deaths - 1) * c.lazarus_blocks;
  const dead = await poller.until((m) => statusOf(m) === "Deceased", { timeoutMs: life * blockMs + 180_000, what: "the third death" });
  const block = dead.status.Deceased.block;
  check(dead.deaths === c.max_deaths && dead.pools.cart === "0", `the monitor shows ${dead.deaths} deaths and an empty cart, with no call from anyone`);

  const written = (await view(coin, "get_code_sheets")).length;
  if (written === 0) {
    const tod = await a.call(coin, "call_time_of_death");
    gas.call_time_of_death.push(tod.gas);
    say(`   ${short(a.accountId)} called it: ${explorerTx(tod.hash)}`);
    check(tod.value === c.max_deaths, `one call wrote all ${tod.value} deaths`);
    const logs = tod.logs.join("\n");
    check(/"event":"deceased"/.test(logs) && /"memo":"estate"/.test(logs), "the log has the deceased event and the estate burn");
  } else {
    say("   its deaths were written on an earlier run (HEADSTONE=reuse)");
  }
  const sheets = await view(coin, "get_code_sheets");
  for (const sheet of sheets) {
    events.add("time_of_death", { block: sheet.end_block, code: sheet.code_no, deaths: sheet.deaths_after });
    for (const line of sheetLines(sheet, short)) say(`   ${line}`);
  }
  events.add("deceased", { block });
  check(sheets.length === c.max_deaths && sheets.every((x) => x.first_compression_block === null), "three code sheets, and no compression on any of them");
  const final = await view(coin, "get_monitor");
  say(`   supply ${rosc(headstone.supply_at_launch)} at launch, ${rosc(final.supply)} now; ${rosc(final.totals.burned_estate)} burned as the estate`);

  step(9, "Every later call fails with the time of death");
  const gone = `Time of death was called at block ${block}.`;
  const tries = [
    ["compress", a, {}], ["breathe", a, {}], ["shock", a, {}], ["push_epi", a, {}], ["call_code", a, {}],
    ["call_time_of_death", a, {}], ["claim_bounty", a, {}], ["faucet_claim", b, {}],
    ["ft_transfer", a, { receiver_id: b.accountId, amount: ONE.toString() }],
  ];
  let first = null;
  for (const [method, wallet, args] of tries) {
    const r = await wallet.attempt(coin, method, args, method === "ft_transfer" ? { deposit: 1n } : {});
    first ??= r;
    say(`   ${method.padEnd(18)} ${r.ok ? "WENT THROUGH" : panicText(r.error)}`);
    check(!r.ok && panicText(r.error) === gone, `${method} is refused`);
  }
  say(`   the chain says: ${first.error}  ${explorerTx(first.hash)}`);
  await pager.busy;
  for (const p of pager.posts) say(`   pager: ${p.text}`);
  poller.stop();
  return { coin, block, pager: pager.posts };
}

// ── Run ────────────────────────────────────────────────────────────────

const started = Date.now();
const events = new EventLog();
const gas = { compress_counted: [], compress_already_this_block: [], compress_hands_off: [], breathe: [], push_epi: [], ft_transfer: [], call_code: [], claim_bounty: [], call_time_of_death: [] };
try {
  say("Code Blue ($ROSC): demo on NEAR testnet");
  // The headstone coin goes first: it dies on its own while the other one lives.
  const headstone = await headstoneCoin();
  const demo = await demoCoin();
  const lived = await life(demo, events, gas);
  const died = await death(headstone, events, gas);
  const pacedBeats = await lived.stopPacing();

  const faucet = demo.bots.map((b) => b.faucet_gas).filter(Boolean);
  const storage = demo.bots.map((b) => b.storage_gas).filter(Boolean);
  const near = (yocto) => Number(yocto / 10n ** 18n) / 1e6;
  const cycles = lived.code.cycles;
  const report = {
    measured_at: new Date().toISOString(),
    network: "testnet",
    contract: lived.coin,
    wasm_sha256: demo.wasm_sha256,
    block_ms: Number(lived.blockMs.toFixed(1)),
    gas: {
      compress_counted: stats(gas.compress_counted),
      compress_already_this_block: stats(gas.compress_already_this_block),
      compress_hands_off: stats(gas.compress_hands_off),
      breathe: stats(gas.breathe),
      push_epi: stats(gas.push_epi),
      call_code: stats(gas.call_code),
      ft_transfer: stats(gas.ft_transfer),
      faucet_claim: stats(faucet),
      storage_deposit: stats(storage),
      claim_bounty: stats(gas.claim_bounty),
      call_time_of_death_writing_three_deaths: stats(gas.call_time_of_death),
    },
    full_code: {
      rounds: lived.sheet.rounds.length,
      near_spent: Object.fromEntries(Object.entries(lived.near_spent).map(([who, yocto]) => [short(who), near(yocto)])),
      presses: lived.code.presses,
      counted: lived.code.counted,
      counted_share_of_presses: lived.code.counted_share,
      blocks_from_first_to_thirtieth: cycles.map((x) => x.span_blocks),
      landing_rate: cycles.length ? cycles.reduce((sum, x) => sum + x.landing_rate, 0) / cycles.length : null,
    },
    rpc: rpcStats,
  };
  writeJson(path.join(OUT, "gas.json"), report);
  const saved = await events.save({ demo: lived.coin, headstone: died.coin, wasm_sha256: demo.wasm_sha256, block_ms: report.block_ms });

  say(`\nDone in ${Math.round((Date.now() - started) / 1000)} s: ${checks} checks passed.`);
  say(`demo coin       ${lived.coin}`);
  say(`headstone coin  ${died.coin}`);
  if (!HUMAN) {
    say(`gas: compress ${report.gas.compress_counted?.median_tgas} Tgas counted, breathe ${report.gas.breathe?.median_tgas}, transfer ${report.gas.ft_transfer?.median_tgas}, claim ${report.gas.claim_bounty?.median_tgas}`);
    say(`landing: ${report.full_code.counted} of ${report.full_code.presses} presses counted; 30 compressions took ${report.full_code.blocks_from_first_to_thirtieth.join(", ")} blocks`);
    say(`a full code cost the compressor ${report.full_code.near_spent["bot-compressor"]} NEAR`);
  }
  say(`${saved.length} events in bots/out/events.json; gas in bots/out/gas.json`);
  say(`the pacemaker sent ${pacedBeats} beats and has now stopped: the demo coin arrests ${demo.config.arrest_blocks} blocks after its last beat. \`make pacer\` keeps it alive.`);
  process.exit(0);
} catch (e) {
  await events.save({ failed: e.message }).catch(() => {});
  console.error(`\nDEMO FAILED after ${checks} checks: ${e.message}`);
  if (process.env.DEBUG) console.error(e.stack);
  process.exit(1);
}
