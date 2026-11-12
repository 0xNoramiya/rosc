// The JSON the contract returns. Amounts are decimal strings in base units.

export type Rhythm = 'Vf' | 'Pea' | 'Asystole';

export type Status =
  | { Alive: { arrest_block: number } }
  | 'Arrest'
  | { Morgue: { since_block: number; until_block: number } }
  | { Deceased: { block: number } };

export type Phase =
  | 'Compressions'
  | { Ventilation: { opened: number } }
  | { AfterRound: { opened: number; shock_open: boolean; epi_open: boolean } };

export interface CodeView {
  code_no: number;
  arrest_block: number;
  rhythm: Rhythm;
  called: boolean;
  first_compression_block: number | null;
  last_compression_block: number;
  round: number;
  cycle: number;
  phase: Phase;
  deadline_block: number;
  shock_open: boolean;
  epi_open: boolean;
  cycle_compressions: number;
  cycle_breaths: number;
  cycle_lead: string | null;
  perfusion: number;
  target: number;
  etco2_bps: number;
  units: [string, number][];
}

export interface Config {
  [key: string]: number | string | boolean;
  arrest_blocks: number;
  bucket_blocks: number;
  compressions_per_cycle: number;
  breaths_per_cycle: number;
  cycles_per_round: number;
  vent_window_blocks: number;
  max_rounds: number;
  max_deaths: number;
  sudden_beats: number;
  min_stroke: string;
  total_supply: string;
  min_rescuer_balance: string;
  faucet_amount: string;
}

export interface Monitor {
  block: number;
  status: Status;
  code: CodeView | null;
  last_beat_block: number;
  alive_since_block: number;
  scars: number;
  deaths: number;
  max_deaths: number;
  ejection_fraction: number;
  beats: { block: number; tag: number }[];
  buckets: { index: number; beats: number; electrical: number }[];
  pools: { cart: string; faucet: string; claimable: string };
  supply: string;
  totals: Record<string, string | number>;
  config: Config;
}

export interface RoundLog {
  round: number;
  end_block: number;
  quality_bps: number;
  fatigued: boolean;
  lead: string | null;
  shock: boolean | null;
  epi: boolean;
  perfusion_after: number;
}

export interface CodeSheet {
  code_no: number;
  arrest_block: number;
  rhythm_at_arrest: Rhythm;
  first_compression_block: number | null;
  end_block: number;
  outcome: 'Rosc' | { Death: string };
  target: number;
  perfusion: number;
  rounds: RoundLog[];
  units: [string, number][];
  bounty: string;
  shares: [string, string][];
  noflow_burn: string;
  epi_burn: string;
  death_burn: string;
  scars_after: number;
  deaths_after: number;
}

export interface LedgerLine {
  seq: number;
  line: { block: number; kind: 'Bounty' | { Burn: string } | { Paid: string }; account: string | null; amount: string; count: number };
}

export interface Deployment {
  name: string;
  network?: string;
  rpc?: string | string[];
  contract: string;
  preset?: string;
  deployed_at_block?: number;
  /** Bot accounts, as ids or as records with an `id`. The trace marks their beats. */
  bots?: (string | { id: string; role?: string })[];
  [key: string]: unknown;
}

export type StatusName = 'alive' | 'arrest' | 'morgue' | 'deceased';

export function statusName(status: Status): StatusName {
  if (status === 'Arrest') return 'arrest';
  if ('Alive' in status) return 'alive';
  if ('Morgue' in status) return 'morgue';
  return 'deceased';
}

export function phaseName(phase: Phase): 'compressions' | 'ventilation' | 'check' {
  if (phase === 'Compressions') return 'compressions';
  return 'Ventilation' in phase ? 'ventilation' : 'check';
}
