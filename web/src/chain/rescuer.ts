// Scrubbing in: who is at the bedside, and the key the page presses with.
//
// The wallet never hands the page a key. The page makes its own key pair,
// keeps the secret in this browser, and asks the wallet once to add the
// public half as a function-call key limited to the coin's rescue methods.
// From then on rescue calls are signed here, with no prompt per press.
import { KeyPair, KeyPairSigner } from 'near-api-js';
import type { KeyPairString } from 'near-api-js';
import { Actions, useNearWallet } from 'near-connect-hooks';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { provider } from './rpc';
import { FastSender, refusal } from './sender';

export const RESCUE_METHODS = ['call_code', 'compress', 'breathe', 'shock', 'push_epi', 'call_time_of_death', 'claim_bounty'];
/// 1 NEAR. A ten-round code is about 320 calls at roughly 0.00025 NEAR each,
/// so this covers a dozen. An allowance of "0" is not unlimited: it is zero.
const ALLOWANCE = '1000000000000000000000000';
const GAS = '30000000000000';

interface Stored {
  accountId: string;
  secretKey: string;
  local?: boolean;
  /** A key the guest desk lent: it can press on this coin and do nothing else. */
  guest?: boolean;
}

const slot = (contractId: string) => `codeblue:rescuer:${contractId}`;

/// Turns what someone pasted into a secret key the page can sign with:
/// `ed25519:…` as it is, or the same key without its prefix. Throws with a
/// message a person can act on.
export function readSecretKey(pasted: string): string {
  const text = pasted.trim().replace(/^["']|["',]+$/g, '');
  const key = !text.includes(':') && /^[1-9A-HJ-NP-Za-km-z]{60,}$/.test(text) ? `ed25519:${text}` : text;
  if (/\s/.test(text)) {
    throw new Error('That looks like a seed phrase. This page needs the private key itself, the one that starts with "ed25519:".');
  }
  try {
    KeyPairSigner.fromSecretKey(key as KeyPairString);
  } catch {
    throw new Error('That is not a secret key. Paste the private key that starts with "ed25519:".');
  }
  return key;
}

function read(contractId: string): Stored | null {
  try {
    return JSON.parse(localStorage.getItem(slot(contractId)) ?? 'null');
  } catch {
    return null;
  }
}

function write(contractId: string, value: Stored | null) {
  try {
    if (value) localStorage.setItem(slot(contractId), JSON.stringify(value));
    else localStorage.removeItem(slot(contractId));
  } catch {
    /* private window: the key lasts for this visit only */
  }
}

export interface Rescuer {
  /** The account at the bedside, from the wallet, from a pasted key, or the coin's guest. */
  accountId: string | null;
  walletAccountId: string | null;
  /** At the bedside as the coin's guest account, with a key that can only press. */
  isGuest: boolean;
  /** Ready to press: a key is on chain and primed. */
  sender: FastSender | null;
  keyStatus: 'none' | 'checking' | 'ready' | 'missing';
  busy: string | null;
  error: string | null;
  connect(): Promise<void>;
  disconnect(): Promise<void>;
  /** One wallet approval: adds the page's key for the rescue methods. */
  scrubIn(): Promise<void>;
  /** Registers with the coin and claims the faucet, signed by the wallet. */
  admit(storageFee: string): Promise<void>;
  transfer(receiverId: string, amount: string): Promise<void>;
  /** Uses an account and secret key typed in by hand instead of a wallet. */
  useLocalKey(accountId: string, secretKey: string): void;
  /** No wallet: makes a key here and asks the coin's guest desk to add it to the guest account. */
  scrubInAsGuest(guestId: string): Promise<void>;
  forgetKey(): void;
}

export function useRescuer(rpc: string, contractId: string | null): Rescuer {
  const wallet = useNearWallet();
  const [stored, setStored] = useState<Stored | null>(null);
  const [sender, setSender] = useState<FastSender | null>(null);
  const [keyStatus, setKeyStatus] = useState<Rescuer['keyStatus']>('none');
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const walletAccountId = wallet.signedAccountId || null;
  useEffect(() => setStored(contractId ? read(contractId) : null), [contractId]);

  // A stored key counts only for the account it was made for.
  const active = stored && (stored.local || stored.accountId === walletAccountId) ? stored : null;
  const accountId = active?.accountId ?? walletAccountId;

  useEffect(() => {
    setSender(null);
    if (!contractId || !active) {
      setKeyStatus('none');
      return;
    }
    let dropped = false;
    setKeyStatus('checking');
    let next: FastSender;
    try {
      next = new FastSender(provider(rpc), active.accountId, active.secretKey, contractId);
    } catch {
      // A key this page cannot read must never stop the page from drawing.
      write(contractId, null);
      setStored(null);
      setKeyStatus('none');
      setError('The key saved in this browser could not be read, so it was removed. Add it again.');
      return;
    }
    next.prime().then(
      () => {
        if (dropped) return;
        setSender(next);
        setKeyStatus('ready');
      },
      () => !dropped && setKeyStatus('missing'),
    );
    return () => {
      dropped = true;
    };
  }, [rpc, contractId, active?.accountId, active?.secretKey]);

  const attempt = useCallback(async (label: string, work: () => Promise<void>) => {
    setBusy(label);
    setError(null);
    try {
      await work();
    } catch (e) {
      setError(refusal(e));
    } finally {
      setBusy(null);
    }
  }, []);

  return useMemo<Rescuer>(
    () => ({
      accountId,
      walletAccountId,
      isGuest: Boolean(active?.guest),
      sender,
      keyStatus,
      busy,
      error,
      connect: () => attempt('Opening the wallet', () => wallet.signIn()),
      disconnect: () => attempt('Signing out', () => wallet.signOut()),
      scrubIn: () =>
        attempt('Waiting for the wallet to add the key', async () => {
          if (!contractId || !walletAccountId) throw new Error('Connect a wallet first.');
          const pair = KeyPair.fromRandom('ed25519');
          await wallet.addFunctionCallKey({
            publicKey: pair.getPublicKey().toString(),
            contractId,
            methodNames: RESCUE_METHODS,
            allowance: ALLOWANCE,
          });
          const next = { accountId: walletAccountId, secretKey: pair.toString() };
          write(contractId, next);
          setStored(next);
        }),
      admit: storageFee =>
        attempt('Waiting for the wallet', async () => {
          if (!contractId) return;
          await wallet.signAndSendTransaction({
            receiverId: contractId,
            actions: [
              Actions.functionCall('storage_deposit', {}, GAS, storageFee),
              Actions.functionCall('faucet_claim', {}, GAS, '0'),
            ],
          });
        }),
      transfer: (receiverId, amount) =>
        attempt('Waiting for the wallet', async () => {
          if (!contractId) return;
          await wallet.signAndSendTransaction({
            receiverId: contractId,
            actions: [Actions.functionCall('ft_transfer', { receiver_id: receiverId, amount }, GAS, '1')],
          });
        }),
      useLocalKey: (id, secretKey) => {
        if (!contractId) return;
        setError(null);
        try {
          const next = { accountId: id.trim().toLowerCase(), secretKey: readSecretKey(secretKey), local: true };
          write(contractId, next);
          setStored(next);
        } catch (e) {
          setError(e instanceof Error ? e.message : String(e));
        }
      },
      scrubInAsGuest: guestId =>
        attempt('Asking the guest desk for a key', async () => {
          if (!contractId) return;
          const pair = KeyPair.fromRandom('ed25519');
          const response = await fetch(`${import.meta.env.BASE_URL}api/guest`, {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify({ public_key: pair.getPublicKey().toString() }),
          });
          const answer: { account_id?: string; error?: string } | null = await response.json().catch(() => null);
          if (!response.ok || answer?.account_id !== guestId) throw new Error(answer?.error ?? 'The guest desk did not answer. Connect a wallet instead.');
          // The key is on chain a block or two before every node shows it.
          const probe = new FastSender(provider(rpc), guestId, pair.toString(), contractId);
          for (let tries = 0; ; tries++) {
            try {
              await probe.prime();
              break;
            } catch {
              if (tries >= 8) throw new Error('The guest key was added but has not shown up yet. Try again in a moment.');
              await new Promise(resolve => setTimeout(resolve, 700));
            }
          }
          const next = { accountId: guestId, secretKey: pair.toString(), local: true, guest: true };
          write(contractId, next);
          setStored(next);
        }),
      forgetKey: () => {
        if (contractId) write(contractId, null);
        setStored(null);
      },
    }),
    [accountId, walletAccountId, active?.guest, sender, keyStatus, busy, error, contractId, rpc, wallet, attempt],
  );
}
