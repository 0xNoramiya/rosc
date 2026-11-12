import { useEffect, useMemo, useRef, useState } from 'react';
import { useRescuer } from './chain/rescuer';
import { READ_RPCS, SEND_RPC, loadDeployments, useMonitor, view } from './chain/rpc';
import type { CodeSheet, Deployment, Monitor } from './chain/types';
import { statusName } from './chain/types';
import { read } from './lib/trace';
import type { Chart } from './screens/Bedside';
import { Bedside, CprCard } from './screens/Bedside';
import { band } from './lib/band';
import { rosc, span, tagOf } from './lib/format';
import { ChartScreen } from './screens/Chart';
import { CodeScreen } from './screens/Code';
import { ParametersScreen } from './screens/Parameters';
import { SimulationScreen } from './screens/Simulation';
import { Ward } from './screens/Ward';

const TABS = [
  ['monitor', 'Monitor'],
  ['code', 'Code'],
  ['chart', 'Chart'],
  ['simulation', 'Simulation'],
  ['parameters', 'Parameters'],
] as const;
type Tab = (typeof TABS)[number][0];

const STATUS_TEXT = { alive: 'Pulse present', arrest: 'Code blue: no pulse', morgue: 'In the morgue', deceased: 'Deceased' };

function useTab(): [Tab, (tab: Tab) => void] {
  const parse = () => (TABS.find(([id]) => `#${id}` === location.hash)?.[0] ?? 'monitor') as Tab;
  const [tab, setTab] = useState<Tab>(parse);
  useEffect(() => {
    const onHash = () => setTab(parse());
    window.addEventListener('hashchange', onHash);
    return () => window.removeEventListener('hashchange', onHash);
  }, []);
  return [tab, next => (location.hash = next)];
}

/// The blocks of the compressions the page has watched land, for the trace.
function useCompressions(monitor: Monitor | null): number[] {
  const seen = useRef<{ code: number; last: number; blocks: number[] }>({ code: 0, last: -1, blocks: [] });
  const code = monitor?.code;
  if (!code) {
    if (seen.current.blocks.length) seen.current = { code: 0, last: -1, blocks: [] };
    return seen.current.blocks;
  }
  if (seen.current.code !== code.code_no) {
    // First sight of this code: the current cycle's presses, one block apart
    // back from the last one. Exact when no block was missed.
    const blocks = code.first_compression_block === null ? [] : Array.from({ length: code.cycle_compressions }, (_, i) => code.last_compression_block - i).reverse();
    seen.current = { code: code.code_no, last: code.first_compression_block === null ? -1 : code.last_compression_block, blocks };
  } else if (code.first_compression_block !== null && code.last_compression_block !== seen.current.last) {
    seen.current = { code: code.code_no, last: code.last_compression_block, blocks: [...seen.current.blocks.slice(-200), code.last_compression_block] };
  }
  return seen.current.blocks;
}

export default function App() {
  const [deployments, setDeployments] = useState<Deployment[] | null>(null);
  useEffect(() => void loadDeployments().then(setDeployments), []);

  const query = useMemo(() => new URLSearchParams(location.search), []);
  // `?coin=` takes a deployment's name or a contract account; the pager links with `?contract=`.
  const wanted = query.get('coin') ?? query.get('contract');
  const deployment = useMemo<Deployment | null>(() => {
    if (!deployments) return null;
    const known = deployments.find(d => d.name === wanted || d.contract === wanted);
    if (known) return known;
    if (wanted?.includes('.')) return { name: wanted, contract: wanted };
    // With nothing asked for, open on a coin left to run rather than a demo
    // coin, which lives for minutes.
    return deployments.find(d => d.preset === 'live') ?? deployments[0] ?? null;
  }, [deployments, wanted]);
  const contract = deployment?.contract ?? null;
  const rpc = query.get('rpc') ?? READ_RPCS[0];
  const sendRpc = query.get('send') ?? SEND_RPC;

  const { monitor, blockMs, at, error } = useMonitor(rpc, contract);
  const rescuer = useRescuer(sendRpc, contract);
  const [tab, setTab] = useTab();

  // Bots announce themselves by name. The trace knows senders by four bytes.
  const [bots, setBots] = useState<Set<number>>(new Set());
  useEffect(() => {
    const ids = (deployment?.bots ?? []).map(bot => (typeof bot === 'string' ? bot : bot.id));
    Promise.all(ids.map(tagOf)).then(tags => setBots(new Set(tags)));
  }, [deployment]);

  const compressions = useCompressions(monitor);
  const reading = useMemo(() => (monitor ? read(monitor, compressions, bots) : null), [monitor, compressions, bots]);
  const status = monitor ? statusName(monitor.status) : null;

  // What the trace is drawing, for the acceptance tests to compare with the chain.
  useEffect(() => {
    (window as unknown as { codeblue?: unknown }).codeblue = { monitor, reading };
  }, [monitor, reading]);

  // The code screen opens by itself when the heart stops, and the band follows the chain.
  const [bandOn, setBandOn] = useState(false);
  const [party, setParty] = useState(0);
  const [iris, setIris] = useState<'in' | 'out' | null>(null);
  const previous = useRef(status);
  useEffect(() => {
    const was = previous.current;
    previous.current = status;
    if (!was || !status || was === status) return;
    // One reaction per change of state, never per press.
    if (status === 'arrest' && was === 'alive') {
      setTab('code');
      band.arrest();
    } else if (status === 'alive' && was === 'arrest') {
      band.fanfare();
      setParty(Date.now());
      setIris('in');
    } else if (was === 'arrest') {
      band.deflate();
      setIris('out');
    }
  }, [status]);
  useEffect(() => {
    if (!party && !iris) return;
    const timer = setTimeout(() => {
      setParty(0);
      setIris(null);
    }, 3200);
    return () => clearTimeout(timer);
  }, [party, iris]);

  // One woodblock tick for each beat that lands while you watch.
  const heard = useRef<number | null>(null);
  const lastBeat = monitor?.last_beat_block ?? null;
  const newestBeat = monitor?.beats.length ? monitor.beats[monitor.beats.length - 1].block : null;
  useEffect(() => {
    if (lastBeat === null) return;
    if (heard.current !== null && lastBeat !== heard.current && lastBeat === newestBeat) band.woodblock();
    heard.current = lastBeat;
  }, [lastBeat, newestBeat]);

  // Code sheets change only when a code ends. The monitor shows an ending
  // the moment it is due, but the chain writes the sheet with the next
  // transaction: until the sheet for the last ended code is there, the page
  // asks again every six blocks.
  const [sheets, setSheets] = useState<CodeSheet[]>([]);
  const ended = monitor ? Number(monitor.totals.saves) + monitor.deaths : 0;
  const latestSheet = sheets.length ? sheets[sheets.length - 1] : null;
  const sheetDue = ended > (latestSheet?.code_no ?? 0);
  const asking = monitor && sheetDue ? Math.floor(monitor.block / 6) : 0;
  const written = monitor ? `${ended}:${Math.floor(monitor.block / 30)}` : '';
  useEffect(() => {
    if (contract) view<CodeSheet[]>(rpc, contract, 'get_code_sheets').then(setSheets, () => {});
  }, [rpc, contract, ended, asking, tab === 'chart' ? written : '']);

  // The visitor's own standing with the coin.
  const [chart, setChart] = useState<Chart>({ registered: false, balance: '0', claimable: '0', storageFee: '0' });
  const account = rescuer.accountId;
  const slow = monitor ? Math.floor(monitor.block / 8) : 0;
  useEffect(() => {
    if (!contract) return;
    let dropped = false;
    (async () => {
      try {
        const bounds = await view<{ min: string }>(rpc, contract, 'storage_balance_bounds');
        if (!account) return !dropped && setChart(c => ({ ...c, storageFee: bounds.min }));
        const [storage, balance, claimable] = await Promise.all([
          view<unknown>(rpc, contract, 'storage_balance_of', { account_id: account }),
          view<string>(rpc, contract, 'ft_balance_of', { account_id: account }),
          view<string>(rpc, contract, 'get_claimable', { account_id: account }),
        ]);
        if (!dropped) setChart({ registered: storage !== null, balance, claimable, storageFee: bounds.min });
      } catch {
        /* the next poll will try again */
      }
    })();
    return () => {
      dropped = true;
    };
  }, [rpc, contract, account, slow, rescuer.busy]);

  const explorer = (hash: string) => `https://testnet.nearblocks.io/txns/${hash}`;

  const burned = monitor ? BigInt(monitor.config.total_supply) - BigInt(monitor.supply) : 0n;

  return (
    <div className={`ward is-${status ?? 'off'}`}>
      <header className="plate">
        <h1>
          Code Blue <span className="ticker">$ROSC</span>
        </h1>
        <p className="tagline">A memecoin with a pulse. Every transfer is a heartbeat. Stop trading it and it flatlines, and somebody has to do CPR.</p>
        <div className="plate-tools">
          {deployments && deployments.length > 1 && (
            <label className="coin-pick">
              Coin
              <select id="coin" value={deployment?.name ?? ''} onChange={e => (location.search = `?coin=${e.target.value}`)}>
                {deployments.map(d => (
                  <option key={d.name} value={d.name}>
                    {d.name}
                  </option>
                ))}
              </select>
            </label>
          )}
          <button
            type="button"
            className="band-toggle"
            aria-pressed={bandOn}
            onClick={() => {
              band.enable(!bandOn);
              setBandOn(!bandOn);
              if (!bandOn) band.woodblock();
            }}
          >
            {bandOn ? 'Band is playing' : 'Start the band'}
          </button>
        </div>
      </header>

      <main>
        <div className="status-strip">
          <span className={`lamp is-${status ?? 'off'}`} aria-hidden="true" />
          <strong role="status">{status ? STATUS_TEXT[status] : contract ? 'Connecting to the chain' : 'No coin selected'}</strong>
          <span className="strip-meta">{contract ? `${contract} on testnet` : ''}</span>
          {monitor && <span className="strip-meta">block {monitor.block.toLocaleString('en-US')}</span>}
        </div>

        {error && !monitor && <p className="room-note is-alarm">The chain is not answering: {error}</p>}
        {deployments && !contract && (
          <p className="room-note">
            No coin is deployed yet. Run <code>make deploy-demo</code>, or open this page with <code>?coin=your-contract.testnet</code>.
          </p>
        )}
        {monitor && reading && deployment && (
          <Ward monitor={monitor} reading={reading} blockMs={blockMs} at={at} deployment={deployment} rpc={rpc} party={party > 0} iris={iris} />
        )}

        <nav className="softkeys" aria-label="Screens">
          {TABS.map(([id, label]) => (
            <a key={id} href={`#${id}`} className="softkey" aria-current={tab === id ? 'page' : undefined}>
              {label}
            </a>
          ))}
        </nav>

        <div className="panel">
          {monitor && reading && deployment && (
            <>
              {tab === 'monitor' && (
                <div className="notes">
                  <div className="note-card sticker is-gold-tint">
                    <h2>Every transfer is a heartbeat</h2>
                    <p>
                      Send at least {rosc(monitor.config.min_stroke)} ROSC to someone else and the coin's heart beats once. {Number(monitor.totals.beats).toLocaleString('en-US')} beats so
                      far.
                    </p>
                  </div>
                  <div className="note-card sticker is-pink-tint">
                    <h2>{span(monitor.config.arrest_blocks, blockMs)} of silence stops it</h2>
                    <p>No pulse, no circulation: transfers freeze until rescuers bring it back. Its eyelids droop as the clock runs down.</p>
                  </div>
                  <div className="note-card sticker is-gold-tint">
                    <h2>CPR is one press per block</h2>
                    <p>
                      {monitor.config.compressions_per_cycle} compressions, {monitor.config.breaths_per_cycle} breaths, repeat. Rescuers split a bounty from the crash cart. Every
                      save leaves a plaster.
                    </p>
                  </div>
                  <p className="note">
                    Supply {rosc(monitor.supply)} ROSC, {rosc(burned)} burned so far. Die {monitor.max_deaths} times and it is gone for good.
                  </p>
                </div>
              )}
              {tab === 'code' && (
                <CodeScreen monitor={monitor} blockMs={blockMs} rescuer={rescuer} sheet={latestSheet} sheetDue={sheetDue} claimable={chart.claimable} explorer={explorer} />
              )}
              {tab === 'chart' && <ChartScreen rpc={rpc} contract={deployment.contract} sheets={sheets} sheetDue={sheetDue} blockMs={blockMs} block={monitor.block} />}
              {tab === 'parameters' && <ParametersScreen config={monitor.config} blockMs={blockMs} contract={deployment.contract} />}
            </>
          )}
          {tab === 'simulation' && <SimulationScreen />}
        </div>
      </main>

      <div className="below">
        {contract && monitor && <Bedside rescuer={rescuer} chart={chart} minBalance={monitor.config.min_rescuer_balance} status={status ?? 'alive'} />}
        <CprCard />
      </div>

      <footer className="small-print">
        <p>
          Testnet toy. Not an investment, not a medical device, not CPR training. "Time of death" and "deceased" on this page describe a token. The contract has no owner and no
          upgrade path; if it dies three times its transfers stay frozen.
        </p>
        <p>The patient was made with Higgsfield and the band with ElevenLabs.</p>
      </footer>
    </div>
  );
}
