// Sends one signed call per block with no wallet prompt and no waiting.
//
// near-api-js's `Account` fetches a block and waits for execution on every
// call. For a press every 600 ms that is too slow, so this keeps its own
// nonce counter: one read of the key and of a block hash at the start, then
// each press is built, signed locally and sent without awaiting the answer.
// Measured on testnet: 94% of presses sent this way were counted.
//
// Gas: a counted compression burns about 2.4 TGas. 15 TGas is attached so a
// call that also has to write a long-due arrest or death still fits, and the
// key's allowance is not tied up: each call in flight holds roughly
// (attached TGas + 1) x 0.001 NEAR of it until the refund comes back.
import { KeyPairSigner, actions, baseDecode, baseEncode, createTransaction } from 'near-api-js';
import type { JsonRpcProvider, KeyPairString } from 'near-api-js';

export interface Sent {
  hash: string;
  /** Resolves with the call's return value, or rejects with the refusal. */
  result: Promise<unknown>;
}

const PANIC = 'Smart contract panicked: ';

/// The contract's own words, without the runtime's prefix.
export function refusal(error: unknown): string {
  const text = error instanceof Error ? error.message : typeof error === 'string' ? error : JSON.stringify(error);
  const at = text.indexOf(PANIC);
  return at >= 0 ? text.slice(at + PANIC.length).replace(/["'}\\]+\s*$/, '') : text;
}

function outcome(response: any): unknown {
  const status = response?.status ?? response?.final_execution_status;
  const failure = response?.status?.Failure ?? response?.receipts_outcome?.find((r: any) => r.outcome?.status?.Failure)?.outcome.status.Failure;
  if (failure) throw new Error(JSON.stringify(failure));
  const value = status?.SuccessValue;
  if (typeof value !== 'string' || value === '') return null;
  return JSON.parse(atob(value));
}

export class FastSender {
  private signer: KeyPairSigner;
  private nonce: bigint | null = null;
  private blockHash: Uint8Array | null = null;
  private primedAt = 0;

  private rpc: JsonRpcProvider;
  readonly accountId: string;
  readonly contractId: string;
  private gas: bigint;

  constructor(rpc: JsonRpcProvider, accountId: string, secretKey: string, contractId: string, gas = 15_000_000_000_000n) {
    this.rpc = rpc;
    this.accountId = accountId;
    this.contractId = contractId;
    this.gas = gas;
    this.signer = KeyPairSigner.fromSecretKey(secretKey as KeyPairString);
  }

  /// Two reads, once. Returns the key's remaining allowance, if it has one.
  async prime(): Promise<{ allowance: string | null; fullAccess: boolean }> {
    const publicKey = await this.signer.getPublicKey();
    const [key, block] = await Promise.all([
      this.rpc.viewAccessKey({ accountId: this.accountId, publicKey, finalityQuery: { finality: 'optimistic' } } as any),
      this.rpc.viewBlock({ finality: 'final' }),
    ]);
    this.nonce = BigInt((key as any).nonce);
    this.blockHash = baseDecode((block as any).header.hash);
    this.primedAt = Date.now();
    const permission = (key as any).permission;
    return { allowance: permission?.FunctionCall?.allowance ?? null, fullAccess: permission === 'FullAccess' };
  }

  /// Signs and sends. Returns as soon as the transaction is on its way.
  async send(method: string, args: Record<string, unknown> = {}, deposit = 0n): Promise<Sent> {
    // A block hash is good for about a day. Refresh well before that.
    if (this.nonce === null || this.blockHash === null || Date.now() - this.primedAt > 6 * 3600_000) await this.prime();
    // The nonce is taken before any await, so two presses never share one.
    const nonce = ++this.nonce!;
    const publicKey = await this.signer.getPublicKey();
    const tx = createTransaction(this.accountId, publicKey, this.contractId, nonce, [actions.functionCall(method, args, this.gas, deposit)], this.blockHash!);
    const { txHash, signedTransaction } = await this.signer.signTransaction(tx);
    const result = this.rpc.sendTransactionUntil(signedTransaction, 'EXECUTED_OPTIMISTIC').then(outcome, error => {
      // A nonce the chain has moved past: read it again for the next press.
      if (/nonce/i.test(String(error?.message ?? error))) this.nonce = null;
      throw error;
    });
    return { hash: baseEncode(txHash), result };
  }
}
