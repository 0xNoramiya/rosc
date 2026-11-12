// Every Config value, read from the chain.
import type { Config } from '../chain/types';
import { rosc, span } from '../lib/format';

const AMOUNTS = new Set(['min_stroke', 'epi_dose', 'total_supply', 'faucet_amount', 'min_rescuer_balance']);
const MEANING: Record<string, string> = {
  arrest_blocks: 'Silence that stops the heart',
  min_stroke: 'Smallest transfer that is a beat',
  refractory_blocks: 'Gap between two beats from one sender',
  bucket_blocks: 'Width of one monitor bucket',
  sudden_beats: 'Beats before the silence that make the arrest VF',
  pea_min: 'Transfers without a pulse that make the arrest PEA',
  vf_degrade_round: 'Round at which unshocked VF becomes asystole',
  compressions_per_cycle: 'Compressions in a cycle',
  breaths_per_cycle: 'Breaths in a cycle',
  cycles_per_round: 'Cycles between rhythm checks',
  vent_window_blocks: 'Window for breaths, shock and epinephrine',
  max_pause_blocks: 'Pause in compressions after which death may be called',
  max_rounds: 'Rounds in a code',
  no_flow_max_blocks: 'Arrest with no compressions before death may be called',
  lazarus_blocks: 'Wait after a death before a new code',
  rehab_blocks: 'Unbroken life that heals one scar',
  base_target: 'Perfusion points for ROSC on a first arrest',
  scar_target: 'Extra points per scar',
  noflow_target_cap: 'Most that delay can add to the target',
  noflow_target_blocks: 'Blocks of delay per extra point',
  hands_only_bps: 'Score of a cycle with no breaths',
  breath_bps: 'Extra score per breath',
  fatigue_bps: 'Round score when the same account leads twice',
  epi_bonus: 'Points per dose of epinephrine',
  epi_first_round: 'First round after which a dose is offered',
  epi_every_rounds: 'Rounds between doses',
  shock_bonus: 'Points for a shock that works',
  shock_min_quality_bps: 'Round quality a shock needs',
  o2_bps: 'Oxygen burn on each transfer',
  premium_bps: 'Premium into the crash cart on each transfer',
  noflow_burn_bps: 'Cart burned per stretch with no compressions',
  noflow_burn_blocks: 'Length of that stretch',
  epi_dose: 'Burned from the cart per dose',
  death_burn_bps: 'Cart burned at each time of death',
  max_deaths: 'Deaths after which the coin is gone for good',
  rosc_scars: 'Scars added by a ROSC',
  death_scars: 'Scars added by a death',
  bounty_bps: 'Share of the cart paid at ROSC',
  ef_start: 'Ejection fraction of an unscarred coin',
  ef_step: 'Ejection fraction lost per scar',
  ef_floor: 'Lowest ejection fraction',
  units_compression: 'Work units per compression',
  units_breath: 'Work units per breath',
  units_shock: 'Work units per successful shock',
  units_call_code: 'Work units for calling the code',
  total_supply: 'Minted once, at launch',
  cart_bps: 'Share of the supply minted into the crash cart',
  faucet_amount: 'Paid once per account by the faucet',
  min_rescuer_balance: 'Balance an account needs to act in a code',
  post_mortem_transfers: 'Plain transfers reopen after final death',
};

function show(key: string, value: number | string | boolean, blockMs: number): string {
  if (typeof value === 'boolean') return value ? 'yes' : 'no';
  if (AMOUNTS.has(key)) return `${rosc(String(value))} ROSC`;
  if (key.endsWith('_blocks')) return `${Number(value).toLocaleString('en-US')} blocks, about ${span(Number(value), blockMs)}`;
  if (key.endsWith('_bps')) return `${Number(value) / 100}%`;
  return Number(value).toLocaleString('en-US');
}

export function ParametersScreen({ config, blockMs, contract }: { config: Config; blockMs: number; contract: string }) {
  return (
    <section className="params">
      <h2>Parameters</h2>
      <p className="note">
        Read from <code>{contract}</code> just now. They were set once, at launch. No method changes them, and the account has no keys.
      </p>
      <div className="scroll">
        <table>
          <thead>
            <tr>
              <th>Parameter</th>
              <th>Value</th>
              <th>Meaning</th>
            </tr>
          </thead>
          <tbody>
            {Object.entries(config).map(([key, value]) => (
              <tr key={key}>
                <td>
                  <code>{key.toUpperCase()}</code>
                </td>
                <td>{show(key, value, blockMs)}</td>
                <td>{MEANING[key] ?? ''}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
