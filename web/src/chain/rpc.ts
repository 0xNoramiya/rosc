// Reads. The page has no backend: everything it shows comes from view calls.
import { JsonRpcProvider } from 'near-api-js';
import { useEffect, useRef, useState } from 'react';
import type { Deployment, Monitor } from './types';

// Measured on 12 Nov 2026: the free FastNEAR endpoint takes
// sends in order and without loss, but refuses sustained polling after about
// two minutes. dRPC takes three reads a second for as long as we tried. So
// reads go to dRPC and sends to FastNEAR, each with the other as a fallback.
// `?rpc=` and `?send=` in the address override them.
export const READ_RPCS = ['https://near-testnet.drpc.org', 'https://test.rpc.fastnear.com'];
export const SEND_RPC = 'https://test.rpc.fastnear.com';

const providers = new Map<string, JsonRpcProvider>();
/// One retry only: a press that fails is a missed press, not one to resend late.
export function provider(url: string): JsonRpcProvider {
  let p = providers.get(url);
  if (!p) providers.set(url, (p = new JsonRpcProvider({ url }, { retries: 1 })));
  return p;
}

export async function view<T>(rpc: string, contractId: string, method: string, args: Record<string, unknown> = {}): Promise<T> {
  let last: unknown;
  for (const url of [rpc, ...READ_RPCS.filter(u => u !== rpc)]) {
    try {
      return (await provider(url).callFunction({ contractId, method, args })) as T;
    } catch (error) {
      last = error;
    }
  }
  throw last;
}

export async function loadDeployments(): Promise<Deployment[]> {
  try {
    const response = await fetch(`${import.meta.env.BASE_URL}deployments.json`);
    return response.ok ? await response.json() : [];
  } catch {
    return [];
  }
}

export interface Pulse {
  monitor: Monitor | null;
  /** Milliseconds per block, measured from the polls. 600 until there is a measurement. */
  blockMs: number;
  /** When the newest monitor arrived, for drawing between polls. */
  at: number;
  error: string | null;
}

/// Polls `get_monitor()` every 400 ms, about three times in two blocks. Slows down when the endpoint
/// pushes back or the tab is hidden, and never overlaps two requests.
export function useMonitor(rpc: string, contractId: string | null, everyMs = 400): Pulse {
  const [pulse, setPulse] = useState<Pulse>({ monitor: null, blockMs: 600, at: 0, error: null });
  const first = useRef<{ block: number; at: number } | null>(null);

  useEffect(() => {
    if (!contractId) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    let wait = everyMs;
    first.current = null;
    setPulse({ monitor: null, blockMs: 600, at: 0, error: null });

    const tick = async () => {
      if (stopped) return;
      if (document.hidden) {
        timer = setTimeout(tick, 1000);
        return;
      }
      try {
        const monitor = await view<Monitor>(rpc, contractId, 'get_monitor');
        if (stopped) return;
        const at = performance.now();
        if (!first.current) first.current = { block: monitor.block, at };
        const blocks = monitor.block - first.current.block;
        const measured = blocks >= 20 ? (at - first.current.at) / blocks : null;
        setPulse(prev => ({ monitor, at, error: null, blockMs: measured ?? prev.blockMs }));
        wait = everyMs;
      } catch (error) {
        wait = Math.min(wait * 2, 8000);
        setPulse(prev => ({ ...prev, error: error instanceof Error ? error.message : String(error) }));
      }
      timer = setTimeout(tick, wait);
    };
    tick();
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  }, [rpc, contractId, everyMs]);

  return pulse;
}
