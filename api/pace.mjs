// The pacemaker's second trigger.
//
// GitHub's scheduler is meant to run .github/workflows/pacemaker.yml, and on
// some days it barely does: over the job's first day it ran one slot, and
// then none for seven hours. Vercel's cron calls this function every three
// hours as well. It does what the workflow does: one small transfer between
// the two pacer wallets, and only when the coin has gone a sixth of its
// arrest window without a beat, so the two triggers never double up.
//
// It needs two settings in the Vercel project. PACER_KEYS is the same JSON
// list of the two bot keys that the workflow has. CRON_SECRET is what Vercel
// sends with every scheduled call, so nobody else can make the bots spend gas.
import { readFileSync } from 'node:fs';
import { Wallet } from '../bots/lib/near.mjs';
import { beatDue, paceOnce } from '../bots/pacer.mjs';

const deployment = JSON.parse(readFileSync(new URL('../deployments/testnet-live.json', import.meta.url), 'utf8'));

export async function GET(request) {
  if (!process.env.CRON_SECRET || request.headers.get('authorization') !== `Bearer ${process.env.CRON_SECRET}`)
    return new Response('For the scheduler only.\n', { status: 401 });
  const coin = deployment.contract;
  // The wallets are made on every call, due or not, so a missing or damaged key shows up at once and not on the
  // day it is needed.
  const keys = new Map(JSON.parse(process.env.PACER_KEYS).map(key => [key.account_id, key.private_key]));
  const wallets = ['bot-pacer', 'bot-pacer-2'].map(role => {
    const id = deployment.bots.find(bot => bot.role === role).id;
    return new Wallet(id, keys.get(id));
  });
  const { due, quiet, every } = await beatDue(coin);
  if (!due) return Response.json({ coin, sent: false, why: `last beat ${quiet} blocks ago, the next is due at ${every}`, wallets: wallets.map(w => w.accountId) });
  const log = [];
  const sent = await paceOnce({ coin, wallets, log: line => log.push(line) });
  // A refusal means the heart has stopped, and a pacemaker cannot restart it.
  // The 500 puts that in the project's logs.
  return Response.json({ coin, sent, log }, { status: sent ? 200 : 500 });
}
