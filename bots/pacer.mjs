// bot-pacer: one beat kept going between two wallets, to show a paced rhythm.
// The coin stays alive on-chain; the monitor labels it for what it is.
//
//   node bots/pacer.mjs [deployment] [hours]      default: testnet-demo, until stopped
//   node bots/pacer.mjs <deployment> --once       one beat, then exit: for a scheduled job
import { fileURLToPath } from "node:url";
import { Wallet, explorerTx, loadDeployment, measureBlockMs, sleep, statusOf, view } from "./lib/near.mjs";

/** Sends the smallest transfer that is a beat, back and forth, twice per arrest window. */
export async function runPacer({ coin, wallets: [a, b], running = () => true, onBeat = () => {}, log = () => {} }) {
  const config = await view(coin, "get_config");
  const blockMs = await measureBlockMs();
  const everyMs = Math.max(4_000, (config.arrest_blocks / 2) * blockMs);
  let beats = 0;
  for (let i = 0; running(); i++) {
    const [from, to] = i % 2 ? [b, a] : [a, b];
    const outcome = await from.attempt(coin, "ft_transfer", { receiver_id: to.accountId, amount: config.min_stroke }, { deposit: 1n });
    if (outcome.ok) {
      beats++;
      onBeat(outcome, from);
    } else {
      log(`refused: ${outcome.error}`);
      // A pacemaker cannot restart a stopped heart. Wait for someone to do CPR.
      const m = await view(coin, "get_monitor");
      if (statusOf(m) === "Deceased") break;
    }
    const until = Date.now() + everyMs;
    while (running() && Date.now() < until) await sleep(500);
  }
  return beats;
}

/** One beat. The direction turns with the hour, so neither wallet runs dry. Returns whether it counted. */
export async function paceOnce({ coin, wallets, log = () => {} }) {
  const config = await view(coin, "get_config");
  const [from, to] = Math.floor(Date.now() / 3_600_000) % 2 ? wallets : [...wallets].reverse();
  const outcome = await from.attempt(coin, "ft_transfer", { receiver_id: to.accountId, amount: config.min_stroke }, { deposit: 1n });
  if (outcome.ok) log(`${new Date().toISOString()} paced beat from ${from.accountId.split(".")[0]}  ${explorerTx(outcome.hash)}`);
  else log(`refused: ${outcome.error}`);
  const monitor = await view(coin, "get_monitor");
  log(`${coin} is ${statusOf(monitor).toLowerCase()} at block ${monitor.block}, ${monitor.deaths} of ${monitor.max_deaths} deaths`);
  return outcome.ok;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const deployment = loadDeployment(process.argv[2] || "testnet-demo");
  const hours = Number(process.argv[3] || 0);
  const wallets = ["bot-pacer", "bot-pacer-2"].map((role) => new Wallet(deployment.bots.find((b) => b.role === role).id));
  if (process.argv.includes("--once")) {
    // A refusal means the heart has stopped. A pacemaker cannot restart it, so the job fails loudly.
    process.exit((await paceOnce({ coin: deployment.contract, wallets, log: console.log })) ? 0 : 1);
  }
  const until = hours ? Date.now() + hours * 3_600_000 : Infinity;
  console.log(`pacing ${deployment.contract}${hours ? ` for ${hours} h` : ", until stopped"}`);
  await runPacer({
    coin: deployment.contract,
    wallets,
    running: () => Date.now() < until,
    onBeat: (o, from) => console.log(`${new Date().toISOString()} paced beat from ${from.accountId.split(".")[0]}  ${explorerTx(o.hash)}`),
    log: console.log,
  });
}
