// bot-holder-*: transfers on a schedule, from several senders, so the monitor
// shows a sinus rhythm. Every transfer burns a little oxygen.
//
//   node bots/holders.mjs [deployment] [seconds]      default: testnet-demo, until stopped
import { fileURLToPath } from "node:url";
import { ONE, Wallet, explorerTx, loadDeployment, rosc, sleep, tagOf } from "./lib/near.mjs";

/**
 * Sends one transfer after another around the ring of `wallets` until
 * `running()` is false. `onBeat(outcome, from, to, amount)` hears each one.
 */
export async function runHolders({ coin, wallets, gapMs = 1_200, running = () => true, onBeat = () => {}, onRefusal = () => {} }) {
  let sent = 0;
  for (let i = 0; running(); i++) {
    const from = wallets[i % wallets.length];
    const to = wallets[(i + 1 + (i % (wallets.length - 1))) % wallets.length];
    if (from === to) continue;
    // Between 5 and 60 ROSC: well over the stroke volume, small against a faucet claim.
    const amount = BigInt(5 + ((i * 37) % 56)) * ONE;
    const outcome = await from.attempt(coin, "ft_transfer", { receiver_id: to.accountId, amount: amount.toString() }, { deposit: 1n });
    if (outcome.ok) {
      sent++;
      onBeat(outcome, from, to, amount);
    } else {
      onRefusal(outcome, from, to, amount);
      if (/No pulse|Time of death/.test(outcome.error)) break;
    }
    await sleep(gapMs);
  }
  return sent;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const deployment = loadDeployment(process.argv[2] || "testnet-demo");
  const seconds = Number(process.argv[3] || 0);
  const wallets = deployment.bots.filter((b) => b.role.startsWith("bot-holder")).map((b) => new Wallet(b.id));
  const until = seconds ? Date.now() + seconds * 1000 : Infinity;
  console.log(`${wallets.length} holder bots on ${deployment.contract}${seconds ? ` for ${seconds} s` : ", until stopped"}`);
  const sent = await runHolders({
    coin: deployment.contract,
    wallets,
    running: () => Date.now() < until,
    onBeat: (o, from, to, amount) =>
      console.log(`beat  ${from.accountId.split(".")[0]} -> ${to.accountId.split(".")[0]}  ${rosc(amount)} ROSC  tag ${tagOf(from.accountId)}  ${explorerTx(o.hash)}`),
    onRefusal: (o) => console.log(`refused: ${o.error}`),
  });
  console.log(`${sent} transfers`);
}
