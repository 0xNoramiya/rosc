// The practice coin's guest desk, for a visitor with no wallet.
//
// The page posts the public half of a key it has just made. This puts it on
// the practice coin's guest account as a key for the rescue calls only
// (bots/guest.mjs has the rules), and the page then presses with it.
//
// It needs GUEST_KEY in the Vercel project: the guest account's own key, as
// {"account_id": ..., "private_key": ...}. That account holds testnet NEAR for
// gas and practice-coin ROSC, and nothing else.
import { readFileSync } from 'node:fs';
import { Wallet } from '../bots/lib/near.mjs';
import { admitGuest, guestOf } from '../bots/guest.mjs';

const deployment = JSON.parse(readFileSync(new URL('../deployments/testnet-practice.json', import.meta.url), 'utf8'));

export async function POST(request) {
  const body = await request.json().catch(() => null);
  const publicKey = body?.public_key;
  if (typeof publicKey !== 'string' || !/^ed25519:[1-9A-HJ-NP-Za-km-z]{43,44}$/.test(publicKey))
    return Response.json({ error: 'Send {"public_key": "ed25519:..."}.' }, { status: 400 });
  try {
    const key = JSON.parse(process.env.GUEST_KEY);
    if (key.account_id !== guestOf(deployment)) throw new Error('GUEST_KEY is not the key of this coin\'s guest account');
    const result = await admitGuest({ coin: deployment.contract, guest: new Wallet(key.account_id, key.private_key), publicKey });
    if (result.full)
      return Response.json({ error: 'Every guest key is in use. Try again in a few minutes, or scrub in with a wallet.' }, { status: 429 });
    return Response.json({ account_id: result.accountId, contract: deployment.contract, tx: result.tx });
  } catch (e) {
    return Response.json({ error: `The guest key could not be added: ${e instanceof Error ? e.message : e}` }, { status: 502 });
  }
}
