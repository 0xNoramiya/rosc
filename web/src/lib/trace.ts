// What the ECG draws. Every mark comes from the chain: this file turns one
// `get_monitor()` answer into a list of marks, and `Ecg.tsx` only paints them.
import type { Monitor, Rhythm } from '../chain/types';
import { statusName } from '../chain/types';

export type Mark =
  | { kind: 'beat'; block: number; tag: number; paced: boolean; bot: boolean }
  /** Transfers that carried no pulse. The contract counts them per bucket. */
  | { kind: 'electrical'; block: number }
  | { kind: 'compression'; block: number }
  /** A stretch with no pulse: the arrest rhythm, the morgue, or death. */
  | { kind: 'flatline'; from: number; to: number; rhythm: Rhythm | 'none' };

export type RhythmLabel = 'Sinus' | 'Bradycardia' | 'Tachycardia' | 'Paced' | 'VF' | 'PEA' | 'Asystole' | 'No rhythm';

export interface Reading {
  marks: Mark[];
  rhythm: RhythmLabel;
  /** Beats in the last arrest window. */
  beatsInWindow: number;
  senders: number;
}

/// The page's own marks: fewer beats than this in one arrest window is slow,
/// more than ten times that is fast.
export const LOW_MARK = 4;
export const HIGH_MARK = 40;

export function read(monitor: Monitor, compressions: number[], bots: Set<number>): Reading {
  const { config, beats, buckets, code, block } = monitor;
  const status = statusName(monitor.status);
  const marks: Mark[] = [];

  // Paced: two or fewer senders made the last 32 beats.
  const recent = beats.slice(-32);
  const senders = new Set(recent.map(b => b.tag)).size;
  const paced = recent.length >= 4 && senders <= 2;
  for (const beat of beats) marks.push({ kind: 'beat', block: beat.block, tag: beat.tag, paced, bot: bots.has(beat.tag) });

  // Electrical activity is known per bucket, so it is spread across the bucket.
  for (const bucket of buckets) {
    for (let i = 0; i < bucket.electrical; i++) {
      const at = (bucket.index + (i + 1) / (bucket.electrical + 1)) * config.bucket_blocks;
      if (at <= block) marks.push({ kind: 'electrical', block: at });
    }
  }

  let rhythm: RhythmLabel;
  if (status === 'arrest' && code) {
    marks.push({ kind: 'flatline', from: code.arrest_block, to: block, rhythm: code.rhythm });
    for (const b of compressions) marks.push({ kind: 'compression', block: b });
    rhythm = code.rhythm === 'Vf' ? 'VF' : code.rhythm === 'Pea' ? 'PEA' : 'Asystole';
  } else if (status === 'morgue' || status === 'deceased') {
    const from = typeof monitor.status === 'object' && 'Morgue' in monitor.status ? monitor.status.Morgue.since_block : (monitor.status as { Deceased: { block: number } }).Deceased.block;
    marks.push({ kind: 'flatline', from, to: block, rhythm: 'none' });
    rhythm = 'No rhythm';
  } else {
    const beatsInWindow = beats.filter(b => b.block > block - config.arrest_blocks).length;
    rhythm = paced ? 'Paced' : beatsInWindow < LOW_MARK ? 'Bradycardia' : beatsInWindow > HIGH_MARK ? 'Tachycardia' : 'Sinus';
  }
  const beatsInWindow = beats.filter(b => b.block > block - config.arrest_blocks).length;
  return { marks, rhythm, beatsInWindow, senders };
}

/// A stream of numbers that depends only on its seed, so every viewer of the
/// same arrest sees the same fibrillation.
export function seeded(seed: number): () => number {
  let s = (seed ^ 0x9e3779b9) >>> 0;
  return () => {
    s ^= s << 13;
    s >>>= 0;
    s ^= s >>> 17;
    s ^= s << 5;
    s >>>= 0;
    return s / 0xffffffff;
  };
}
