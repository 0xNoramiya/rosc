// The practice coin's guest desk: lends a visitor's browser a key that can only do CPR.
//
//   node bots/guest.mjs <deployment> <public key>     add that key to the deployment's guest account
//
// A visitor with no wallet presses as the coin's guest account. The page makes a key pair, keeps the secret half
// in the browser and hands over the public half. It goes on the guest account as a function-call key: it can make
// the rescue calls on that one coin and nothing else, it cannot move a token or a NEAR, and it has a small gas
// allowance. Keys older than an hour come off in the same transaction.
import { fileURLToPath } from "node:url";
import { PublicKey, actions } from "near-api-js";
import { NEAR, Wallet, accessKeys, block, explorerTx, loadDeployment } from "./lib/near.mjs";

/** The rescue calls, the same list as the page's own key (web/src/chain/rescuer.ts). */
export const RESCUE_METHODS = ["call_code", "compress", "breathe", "shock", "push_epi", "call_time_of_death", "claim_bounty"];
// 0.2 NEAR of gas. A practice code burns about 0.011, but every press in flight holds some 0.016 of the allowance
// until its refund is back, and four can be in flight at once.
const ALLOWANCE = NEAR / 5n;
const KEEP_BLOCKS = 6_000; // a guest key lasts about an hour
const MOST = 30; // guest keys at any one time

/** Adds `publicKey` to the guest account for the rescue calls on `coin`. `{ full: true }` when every place is taken. */
export async function admitGuest({ coin, guest, publicKey }) {
  const key = PublicKey.fromString(publicKey);
  const [keys, head] = await Promise.all([accessKeys(guest.accountId), block()]);
  const lent = keys.filter((k) => k.access_key.permission !== "FullAccess");
  if (lent.some((k) => k.public_key === key.toString())) return { accountId: guest.accountId, tx: null };
  // A key's nonce starts at its block times a million, so the nonce says how old the key is.
  const born = (k) => Number(BigInt(k.access_key.nonce) / 1_000_000n);
  const stale = lent.filter((k) => head.header.height - born(k) > KEEP_BLOCKS).slice(0, 30);
  if (lent.length - stale.length >= MOST) return { full: true };
  const outcome = await guest.send(guest.accountId, [
    ...stale.map((k) => actions.deleteKey(PublicKey.fromString(k.public_key))),
    actions.addFunctionCallAccessKey(key, coin, RESCUE_METHODS, ALLOWANCE),
  ]);
  // Asked twice in a row, the list above may not show the first key yet. It is there, which is all that was wanted.
  if (!outcome.ok && String(outcome.error).includes("AddKeyAlreadyExists")) return { accountId: guest.accountId, tx: null };
  if (!outcome.ok) throw new Error(outcome.error);
  return { accountId: guest.accountId, tx: outcome.hash, removed: stale.length };
}

export const guestOf = (deployment) => deployment.bots.find((b) => b.role === "guest")?.id ?? null;

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [name, publicKey] = process.argv.slice(2);
  if (!name || !publicKey) {
    console.error("usage: node bots/guest.mjs <deployment> <public key>");
    process.exit(2);
  }
  const deployment = loadDeployment(name);
  const result = await admitGuest({ coin: deployment.contract, guest: new Wallet(guestOf(deployment)), publicKey });
  console.log(result.full ? "every guest key is in use" : result.tx ? `added to ${result.accountId}, ${result.removed} old key(s) removed: ${explorerTx(result.tx)}` : "that key is already there");
}
