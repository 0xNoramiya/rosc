// Talking to NEAR testnet: JSON-RPC with failover, wallets that keep their own
// nonce, and the helpers every bot shares.
//
// Testnet only. Requests carry JSON-RPC bodies and nothing else.
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { KeyPair, KeyPairSigner, actions, baseDecode, baseEncode, createTransaction } from "near-api-js";

export const HERE = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
export const ROOT = path.dirname(HERE);
export const KEYS = path.join(HERE, ".keys");
export const OUT = path.join(HERE, "out");
export const DEPLOYMENTS = path.join(ROOT, "deployments");

export const ONE = 10n ** 18n; // one ROSC
export const TGAS = 10n ** 12n;
export const NEAR = 10n ** 24n;

// Public endpoints are shared and rate-limited. The first one that answers is
// used; a refusal moves on to the next. NEAR_RPC overrides the list.
export const ENDPOINTS = (process.env.NEAR_RPC || "https://test.rpc.fastnear.com,https://near-testnet.drpc.org")
  .split(",")
  .map((s) => s.trim())
  .filter(Boolean);

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
export const explorerTx = (hash) => `https://testnet.nearblocks.io/txns/${hash}`;
export const explorerAccount = (id) => `https://testnet.nearblocks.io/address/${id}`;

/** 1234500000000000000000 -> "1,234.5" */
export function rosc(amount, places = 2) {
  const v = BigInt(amount);
  const whole = v / ONE;
  const frac = ((v % ONE) * 10n ** BigInt(places)) / ONE;
  const text = whole.toLocaleString("en-US");
  return frac ? `${text}.${frac.toString().padStart(places, "0").replace(/0+$/, "")}` : text;
}

export class RpcError extends Error {
  constructor(message, data) {
    super(message);
    this.data = data;
  }
}

// ── JSON-RPC ───────────────────────────────────────────────────────────

const cooling = new Map(); // endpoint -> time it may be used again
const refusals = new Map(); // endpoint -> refusals in a row
let rpcId = 0;
export const rpcStats = { requests: 0, refused: 0, byEndpoint: {} };

function order() {
  const now = Date.now();
  const ready = ENDPOINTS.filter((e) => (cooling.get(e) || 0) <= now);
  return ready.length ? ready : [...ENDPOINTS].sort((a, b) => cooling.get(a) - cooling.get(b));
}

async function post(endpoint, method, params, timeoutMs) {
  const ctl = new AbortController();
  const timer = setTimeout(() => ctl.abort(), timeoutMs);
  try {
    const res = await fetch(endpoint, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: ++rpcId, method, params }),
      signal: ctl.signal,
    });
    const text = await res.text();
    let json = null;
    try {
      json = JSON.parse(text);
    } catch {
      /* a rate-limit page, not JSON */
    }
    return { status: res.status, json };
  } finally {
    clearTimeout(timer);
  }
}

/**
 * One JSON-RPC call. Moves to the next endpoint when one refuses or times
 * out. An error the chain itself returns (unknown account, invalid nonce) is
 * thrown at once: asking another node would not change it.
 */
export async function rpc(method, params, { timeoutMs = 20_000, attempts = 6, endpoint = null } = {}) {
  let last = null;
  for (let attempt = 0; attempt < attempts; attempt++) {
    const target = endpoint || order()[0];
    rpcStats.requests++;
    rpcStats.byEndpoint[target] = (rpcStats.byEndpoint[target] || 0) + 1;
    let res;
    try {
      res = await post(target, method, params, timeoutMs);
    } catch (e) {
      last = new RpcError(`${target}: ${e.name === "AbortError" ? "timed out" : e.message}`);
      cooling.set(target, Date.now() + 5_000);
      continue;
    }
    const refused = res.status === 429 || res.json?.error?.code === -429 || res.status >= 500 || !res.json;
    if (refused) {
      rpcStats.refused++;
      last = new RpcError(`${target}: HTTP ${res.status} ${res.json?.error?.message || ""}`.trim());
      // Back off longer each time: a public endpoint that is out of quota stays out for a while.
      const strikes = (refusals.get(target) || 0) + 1;
      refusals.set(target, strikes);
      cooling.set(target, Date.now() + Math.min(300_000, 15_000 * 2 ** (strikes - 1)));
      if (endpoint || ENDPOINTS.length === 1) await sleep(1_500 * (attempt + 1));
      continue;
    }
    refusals.set(target, 0);
    if (res.json.error) {
      const err = res.json.error;
      const text = err.data ? (typeof err.data === "string" ? err.data : JSON.stringify(err.data)) : err.message;
      // A node that is a block behind does not know a brand-new transaction or block yet.
      const unknown = /UNKNOWN_TRANSACTION|UNKNOWN_BLOCK|TIMEOUT_ERROR|[Tt]imeout|doesn't exist|does not exist while viewing/.test(
        `${err.cause?.name} ${text}`,
      );
      throw new RpcError(text, { ...err, retryable: unknown });
    }
    return res.json.result;
  }
  throw last || new RpcError("no endpoint answered");
}

/** Calls a view method and parses its JSON result. */
export async function view(contractId, methodName, args = {}) {
  const result = await rpc("query", {
    request_type: "call_function",
    finality: "optimistic",
    account_id: contractId,
    method_name: methodName,
    args_base64: Buffer.from(JSON.stringify(args)).toString("base64"),
  });
  const text = Buffer.from(result.result).toString("utf8");
  return text ? JSON.parse(text) : null;
}

export const viewAccount = (accountId) =>
  rpc("query", { request_type: "view_account", finality: "optimistic", account_id: accountId });

export const accessKeys = (accountId) =>
  rpc("query", { request_type: "view_access_key_list", finality: "final", account_id: accountId }).then((r) => r.keys);

export const block = (ref = { finality: "final" }) => rpc("block", ref);

const heights = new Map();
/** The height and time of a block, by hash. Cached. */
export async function blockInfo(hash) {
  if (!heights.has(hash)) {
    let found = null;
    for (let i = 0; i < 6 && !found; i++) {
      try {
        await lookupSlot();
        const b = await block({ block_id: hash });
        found = { height: b.header.height, time: new Date(Number(BigInt(b.header.timestamp_nanosec) / 1_000_000n)).toISOString() };
      } catch (e) {
        if (!e.data?.retryable) throw e;
        await sleep(700);
      }
    }
    if (!found) throw new RpcError(`block ${hash} not found`);
    heights.set(hash, found);
  }
  return heights.get(hash);
}

/** Milliseconds per block height, measured over the last `span` heights. */
export async function measureBlockMs(span = 300) {
  const tip = await block({ finality: "final" });
  let past = null;
  for (let back = span; back < span + 20 && !past; back++) {
    past = await block({ block_id: tip.header.height - back }).catch(() => null);
  }
  if (!past) return 600;
  const ms = Number(BigInt(tip.header.timestamp_nanosec) - BigInt(past.header.timestamp_nanosec)) / 1e6;
  return ms / (tip.header.height - past.header.height);
}

/** Four bytes that stand for an account on the monitor trace. Matches the contract. */
export function tagOf(accountId) {
  return crypto.createHash("sha256").update(accountId).digest().readUInt32LE(0);
}

// ── Keys ───────────────────────────────────────────────────────────────

export function saveKey(accountId, keyPair) {
  fs.mkdirSync(KEYS, { recursive: true, mode: 0o700 });
  const file = path.join(KEYS, `${accountId}.json`);
  fs.writeFileSync(
    file,
    JSON.stringify({ account_id: accountId, public_key: keyPair.getPublicKey().toString(), private_key: keyPair.toString() }),
    { mode: 0o600 },
  );
  return file;
}

export function hasKey(accountId) {
  return [path.join(KEYS, `${accountId}.json`), path.join(os.homedir(), ".near-credentials", "testnet", `${accountId}.json`)].some(
    (p) => fs.existsSync(p),
  );
}

function loadKey(accountId) {
  for (const p of [path.join(KEYS, `${accountId}.json`), path.join(os.homedir(), ".near-credentials", "testnet", `${accountId}.json`)]) {
    if (fs.existsSync(p)) {
      const j = JSON.parse(fs.readFileSync(p, "utf8"));
      return j.private_key || j.secret_key;
    }
  }
  throw new Error(`no key on this machine for ${accountId}`);
}

export function removeKey(accountId) {
  const file = path.join(KEYS, `${accountId}.json`);
  if (fs.existsSync(file)) fs.rmSync(file);
}

// ── Outcomes ───────────────────────────────────────────────────────────

/** What a finished transaction did, in the terms the bots care about. */
export function digest(result) {
  const receipts = result.receipts_outcome || [];
  const all = [result.transaction_outcome, ...receipts].filter(Boolean);
  const gas = all.reduce((sum, o) => sum + BigInt(o.outcome.gas_burnt), 0n);
  const tokens = all.reduce((sum, o) => sum + BigInt(o.outcome.tokens_burnt), 0n);
  const out = {
    hash: result.transaction?.hash || result.transaction_outcome?.id,
    ok: false,
    value: undefined,
    error: null,
    gas,
    tokens,
    logs: receipts.flatMap((r) => r.outcome.logs),
    // The block the call itself ran in: its first receipt.
    blockHash: receipts[0]?.block_hash || result.transaction_outcome?.block_hash,
  };
  const status = result.status || {};
  if ("SuccessValue" in status) {
    out.ok = true;
    const text = Buffer.from(status.SuccessValue, "base64").toString("utf8");
    try {
      out.value = text ? JSON.parse(text) : null;
    } catch {
      out.value = text;
    }
  } else {
    const failure = status.Failure ?? status;
    const exec = failure?.ActionError?.kind?.FunctionCallError?.ExecutionError;
    out.error = exec || JSON.stringify(failure);
  }
  return out;
}

/** "Smart contract panicked: Foo." -> "Foo." */
export const panicText = (error) => String(error || "").replace(/^Smart contract panicked: /, "");

// ── Wallet ─────────────────────────────────────────────────────────────

const DONE = new Set(["EXECUTED_OPTIMISTIC", "EXECUTED", "FINAL"]);

// Outcome lookups share one slow lane, so a burst of presses cannot turn
// into a burst of status requests.
let nextLookup = 0;
async function lookupSlot(gapMs = Number(process.env.LOOKUP_GAP_MS || 300)) {
  const now = Date.now();
  const at = Math.max(now, nextLookup);
  nextLookup = at + gapMs;
  if (at > now) await sleep(at - now);
}

/**
 * One account, one key, one local nonce. Every transaction the account sends
 * goes through here, so presses fired a block apart never collide.
 */
export class Wallet {
  constructor(accountId, secretKey = loadKey(accountId)) {
    this.accountId = accountId;
    this.keyPair = KeyPair.fromString(secretKey);
    this.signer = new KeyPairSigner(this.keyPair);
    this.publicKey = this.keyPair.getPublicKey();
    this.nonce = null;
    this.blockHash = null;
    this.primedAt = 0;
  }

  static fresh(accountId) {
    const keyPair = KeyPair.fromRandom("ed25519");
    saveKey(accountId, keyPair);
    return new Wallet(accountId, keyPair.toString());
  }

  /** Reads the key's nonce and a recent block hash. Two calls, then none for ten minutes. */
  async prime(force = false) {
    if (!force && this.nonce !== null && Date.now() - this.primedAt < 600_000) return;
    let key = null;
    for (let i = 0; i < 12 && !key; i++) {
      try {
        key = await rpc("query", {
          request_type: "view_access_key",
          finality: "optimistic",
          account_id: this.accountId,
          public_key: this.publicKey.toString(),
        });
      } catch (e) {
        // A just-created account takes a block or two to show up everywhere.
        if (i === 11 || !/does not exist|doesn't exist|UNKNOWN/i.test(e.message)) throw e;
        await sleep(800);
      }
    }
    const tip = await block({ finality: "final" });
    const chain = BigInt(key.nonce);
    this.nonce = this.nonce !== null && this.nonce > chain ? this.nonce : chain;
    this.blockHash = baseDecode(tip.header.hash);
    this.primedAt = Date.now();
  }

  /**
   * Signs and broadcasts. Returns at once with the hash and a promise for the
   * outcome; a press does not wait for it. The outcome is read back with
   * `tx` rather than by holding the send open: free endpoints cut long
   * requests short.
   */
  async fire(receiverId, actionList, { tries = 40 } = {}) {
    await this.prime();
    const nonce = ++this.nonce; // reserved before any await below
    const tx = createTransaction(this.accountId, this.publicKey, receiverId, nonce, actionList, this.blockHash);
    const { txHash, signedTransaction } = await this.signer.signTransaction(tx);
    const hash = baseEncode(txHash);
    const sentAt = Date.now();
    const signed = Buffer.from(signedTransaction.encode()).toString("base64");
    const ack = rpc("send_tx", { signed_tx_base64: signed, wait_until: "NONE" }, { timeoutMs: 15_000, attempts: 3 })
      .catch(async (e) => {
        if (/InvalidNonce|Expired/.test(e.message)) {
          await this.prime(true).catch(() => {});
          throw e;
        }
        // The answer was lost; the transaction may still have gone out.
      })
      .then(() => this.status(hash, { tries }))
      .catch((e) => ({ hash, ok: false, error: e.message, lost: true, gas: 0n, tokens: 0n, logs: [] }))
      .then((outcome) => outcome || { hash, ok: false, error: "the transaction never showed up", lost: true, gas: 0n, tokens: 0n, logs: [] });
    return { hash, nonce, sentAt, ack };
  }

  /** The outcome of a transaction, once it has run. Null if it never shows up. */
  async status(hash, { tries = 40 } = {}) {
    await sleep(1_100);
    for (let i = 0; i < tries; i++) {
      await lookupSlot();
      try {
        const result = await rpc("tx", { tx_hash: hash, sender_account_id: this.accountId, wait_until: "NONE" }, { timeoutMs: 15_000 });
        if (DONE.has(result.final_execution_status)) return { ...digest(result), hash };
      } catch (e) {
        if (!e.data?.retryable) throw e;
      }
      await sleep(600);
    }
    return null;
  }

  /** Sends and waits for the outcome. Retries once on a stale nonce. */
  async send(receiverId, actionList) {
    for (let attempt = 0; ; attempt++) {
      const { ack } = await this.fire(receiverId, actionList);
      const outcome = await ack;
      if (outcome.lost && attempt < 3) {
        await sleep(1_500);
        await this.prime(true);
        continue;
      }
      return outcome;
    }
  }

  /** Calls a contract method and waits. Throws the contract's own message on a refusal. */
  async call(contractId, methodName, args = {}, { gas = 30n * TGAS, deposit = 0n } = {}) {
    const outcome = await this.send(contractId, [actions.functionCall(methodName, args, gas, deposit)]);
    if (!outcome.ok) {
      const err = new Error(panicText(outcome.error));
      err.outcome = outcome;
      throw err;
    }
    return outcome;
  }

  /** The same call, but a refusal comes back as a value instead of a throw. */
  async attempt(contractId, methodName, args = {}, opts = {}) {
    try {
      return await this.call(contractId, methodName, args, opts);
    } catch (e) {
      if (!e.outcome) throw e;
      return e.outcome;
    }
  }

  /**
   * Fires a contract call without waiting. A press that has not shown up
   * after a dozen blocks is not coming.
   */
  press(contractId, methodName, gas = 10n * TGAS) {
    return this.fire(contractId, [actions.functionCall(methodName, {}, gas, 0n)], { tries: 10 });
  }

  transferRosc(contractId, receiverId, amount) {
    return this.call(contractId, "ft_transfer", { receiver_id: receiverId, amount: amount.toString() }, { deposit: 1n });
  }
}

// ── Deployments ────────────────────────────────────────────────────────

export function loadDeployment(name) {
  const file = path.join(DEPLOYMENTS, `${name}.json`);
  if (!fs.existsSync(file)) throw new Error(`no deployment named ${name}: run \`node bots/deploy.mjs ${name}\` first`);
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

export function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(value, (k, v) => (typeof v === "bigint" ? v.toString() : v), 2) + "\n");
}

export const statusOf = (monitor) => (typeof monitor.status === "string" ? monitor.status : Object.keys(monitor.status)[0]);
export const phaseOf = (code) => (typeof code.phase === "string" ? code.phase : Object.keys(code.phase)[0]);
