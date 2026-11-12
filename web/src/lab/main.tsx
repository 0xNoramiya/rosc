// The look lab. Scaffolding: the page never imports this file. It edits one
// Look, draws the real ward with it, and saves it to src/look/look.json,
// which is what the page reads.
//
//   click the canvas   a new variation (a new seed)
//   the knobs          every taste decision
//   Save               writes look.json and keeps a copy beside it
import { StrictMode, useEffect, useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';
import recorded from '../../tests/recorded/monitor.json';
import type { CodeSheet, Monitor } from '../chain/types';
import '../index.css';
import { read } from '../lib/trace';
import { LookProvider } from '../look/LookProvider';
import type { Look } from '../look/look';
import { KNOBS, MODES, normalise } from '../look/look';
import saved from '../look/look.json';
import { Sheet } from '../screens/Code';
import { Ward } from '../screens/Ward';
import './lab.css';

const STATES = {
  well: 'Alive, busy',
  drowsy: 'Alive, nodding off',
  paced: 'Alive, paced by a bot',
  waiting: 'Arrest: nobody on the chest yet',
  flat: 'Arrest: asystole, CPR running',
  vf: 'Arrest: VF',
  morgue: 'Morgue',
  deceased: 'Deceased',
} as const;
type State = keyof typeof STATES;

const base = (recorded as unknown as { monitor: Monitor }).monitor;
const START = 277_000_000;

/// A monitor for each state the page can be in, moving with the clock so the
/// motion can be judged: beats land, the strip feeds, compressions press.
function fixture(state: State, block: number): Monitor {
  const config = base.config;
  const monitor: Monitor = { ...base, block, config, status: { Alive: { arrest_block: block + 60 } }, code: null, scars: 2, deaths: 1, ejection_fraction: 50, beats: [], buckets: [] };
  const every = state === 'paced' ? 22 : state === 'drowsy' ? 70 : 11;
  const tags = state === 'paced' ? [7, 8] : [1, 2, 3, 4, 5];
  const silent = state === 'waiting' ? 110 : state === 'flat' || state === 'vf' ? 150 : state === 'morgue' || state === 'deceased' ? 400 : 0;
  const lastBeat = Math.floor((block - silent - (state === 'drowsy' ? 62 : 0)) / every) * every;
  for (let b = lastBeat - every * 8; b <= lastBeat; b += every) monitor.beats.push({ block: b, tag: tags[Math.abs(Math.floor(b / every)) % tags.length] });
  monitor.last_beat_block = lastBeat;
  if (state === 'well' || state === 'drowsy' || state === 'paced') {
    monitor.status = { Alive: { arrest_block: lastBeat + config.arrest_blocks + 1 } };
    return monitor;
  }
  if (state === 'morgue') {
    monitor.status = { Morgue: { since_block: block - 20, until_block: block + 30 } };
    monitor.deaths = 2;
    monitor.scars = 4;
    return monitor;
  }
  if (state === 'deceased') {
    monitor.status = { Deceased: { block: block - 300 } };
    monitor.deaths = 3;
    monitor.pools = { ...monitor.pools, cart: '0' };
    return monitor;
  }
  const arrest = lastBeat + config.arrest_blocks + 1;
  const done = Math.max(0, block - arrest - 20);
  const inCycle = done % 34;
  monitor.status = 'Arrest';
  monitor.code = {
    code_no: 2,
    arrest_block: arrest,
    rhythm: state === 'vf' ? 'Vf' : 'Asystole',
    called: true,
    first_compression_block: arrest + 20,
    last_compression_block: inCycle < 30 ? block : block - (inCycle - 29),
    round: 1 + (Math.floor(done / 68) % 3),
    cycle: 1 + (Math.floor(done / 34) % 2),
    phase: inCycle < 30 ? 'Compressions' : { Ventilation: { opened: block - (inCycle - 29) } },
    deadline_block: block + 16,
    shock_open: false,
    epi_open: false,
    cycle_compressions: Math.min(30, inCycle + 1),
    cycle_breaths: inCycle >= 32 ? 1 : 0,
    cycle_lead: 'bot-compressor.rosc.testnet',
    perfusion: 9000,
    target: 30200,
    etco2_bps: 9300,
    units: [['bot-compressor.rosc.testnet', 60]],
  };
  // A code as the chain opens it: no compression yet, and the arrest block
  // standing in last_compression_block.
  if (state === 'waiting')
    monitor.code = { ...monitor.code, called: false, first_compression_block: null, last_compression_block: arrest, round: 1, cycle: 1, phase: 'Compressions', cycle_compressions: 0, cycle_breaths: 0, perfusion: 0, units: [] };
  return monitor;
}

const SHEET: CodeSheet = {
  code_no: 1,
  arrest_block: START,
  rhythm_at_arrest: 'Asystole',
  first_compression_block: START + 20,
  end_block: START + 194,
  outcome: 'Rosc',
  target: 20200,
  perfusion: 22603,
  rounds: [
    { round: 1, end_block: START + 90, quality_bps: 9383, fatigued: false, lead: 'bot-compressor.rosc.testnet', shock: null, epi: false, perfusion_after: 9383 },
    { round: 2, end_block: START + 194, quality_bps: 8220, fatigued: false, lead: 'bot-airway.rosc.testnet', shock: null, epi: true, perfusion_after: 22603 },
  ],
  units: [
    ['bot-compressor.rosc.testnet', 90],
    ['bot-airway.rosc.testnet', 80],
  ],
  bounty: '19979600030000000000000000',
  shares: [
    ['bot-compressor.rosc.testnet', '10577435310000000000000000'],
    ['bot-airway.rosc.testnet', '9402164720000000000000000'],
  ],
  noflow_burn: '20000000000000000000000',
  epi_burn: '1000000000000000000000000',
  death_burn: '0',
  scars_after: 1,
  deaths_after: 0,
};

interface Kept {
  name: string;
  look: Look;
}

function Lab() {
  const [look, setLook] = useState<Look>(() => normalise(saved as Partial<Look>));
  const [state, setState] = useState<State>('well');
  const [clock, setClock] = useState({ block: START, at: performance.now() });
  const [kept, setKept] = useState<Kept[]>([]);
  const [note, setNote] = useState('');
  const [party, setParty] = useState(false);

  // A chain that makes a block every 600 ms.
  useEffect(() => {
    const born = performance.now();
    const timer = setInterval(() => setClock({ block: START + Math.floor((performance.now() - born) / 600), at: performance.now() }), 600);
    return () => clearInterval(timer);
  }, []);
  const refresh = () =>
    fetch('/__lab/saved')
      .then(r => r.json())
      .then(setKept, () => setKept([]));
  useEffect(() => void refresh(), []);

  const monitor = useMemo(() => fixture(state, clock.block), [state, clock.block]);
  const compressions = useMemo(
    () => (monitor.code ? Array.from({ length: Math.min(30, monitor.code.cycle_compressions) }, (_, i) => monitor.code!.last_compression_block - i).reverse() : []),
    [monitor],
  );
  const reading = useMemo(() => read(monitor, compressions, new Set([7, 8])), [monitor, compressions]);
  const set = (patch: Partial<Look>) => setLook(current => normalise({ ...current, ...patch }));

  const save = async () => {
    const text = JSON.stringify(look, null, 2);
    navigator.clipboard?.writeText(text).catch(() => {});
    try {
      const response = await fetch('/__lab/save', { method: 'POST', body: text });
      const { path, kept: copy } = await response.json();
      setNote(`Saved to ${path} (copy at ${copy}). Also on the clipboard. The page is now using it.`);
      refresh();
    } catch {
      setNote('Copied to the clipboard. The file can only be written while the dev server is running.');
    }
  };

  return (
    <LookProvider look={look}>
      <div className="lab">
        <div className="lab-canvas" onClick={() => set({ seed: Math.floor(Math.random() * 1_000_000) })} title="Click for a new variation">
          <div className="ward">
            <header className="plate">
              <h1>
                Code Blue <span className="ticker">$ROSC</span>
              </h1>
              <p className="tagline">A memecoin with a pulse. Every transfer is a heartbeat. Stop trading it and it flatlines, and somebody has to do CPR.</p>
            </header>
            <Ward
              monitor={monitor}
              reading={reading}
              blockMs={600}
              at={clock.at}
              deployment={{ name: 'lab', contract: 'lab.testnet', deployed_at_block: START - 5000 }}
              rpc=""
              party={party}
            />
            <nav className="softkeys" aria-label="Sticker specimens">
              {['Monitor', 'Code', 'Chart', 'Simulation', 'Parameters'].map((label, i) => (
                <span key={label} className="softkey" aria-current={i === 0 ? 'page' : undefined}>
                  {label}
                </span>
              ))}
            </nav>
            <div className="panel">
              <div className="code">
                <p className="aed">Continue compressions.</p>
                <div className="pads">
                  <div className="metronome is-tick">
                    <span className="metronome-dot" />
                    <span className="metronome-rate">100 a minute</span>
                  </div>
                  <button
                    type="button"
                    className="pad pad-compress"
                    onClick={event => {
                      event.stopPropagation();
                      window.dispatchEvent(new Event('codeblue:press'));
                    }}
                  >
                    <span className="pad-big">Push</span>
                    <span>
                      one compression <kbd>Space</kbd>
                    </span>
                  </button>
                  <button type="button" className="pad" onClick={e => e.stopPropagation()}>
                    Give breath <kbd>B</kbd>
                  </button>
                  <button type="button" className="pad primary" onClick={e => e.stopPropagation()}>
                    Claim bounty
                  </button>
                  <button type="button" className="pad" disabled>
                    Shock <kbd>S</kbd>
                  </button>
                </div>
                <Sheet sheet={SHEET} blockMs={600} />
              </div>
            </div>
          </div>
        </div>

        <aside className="lab-panel">
          <h1>Look lab</h1>
          <p className="lab-seed">
            Seed <strong>{look.seed}</strong>. Click the picture for a new variation.
          </p>

          <label>
            Show the patient
            <select id="lab-state" value={state} onChange={e => setState(e.target.value as State)}>
              {Object.entries(STATES).map(([key, label]) => (
                <option key={key} value={key}>
                  {label}
                </option>
              ))}
            </select>
          </label>
          <button
            type="button"
            onClick={() => {
              setParty(true);
              setTimeout(() => setParty(false), 3000);
            }}
          >
            Play a revival
          </button>

          <h2>Modes</h2>
          {(Object.keys(MODES) as (keyof typeof MODES)[]).map(key => (
            <label key={key}>
              {MODES[key].label}
              <select id={`mode-${key}`} value={look[key]} onChange={e => set({ [key]: e.target.value } as Partial<Look>)}>
                {Object.entries(MODES[key].options).map(([value, label]) => (
                  <option key={value} value={value}>
                    {label}
                  </option>
                ))}
              </select>
            </label>
          ))}

          {(['Motion', 'Paper strip', 'Print', 'Stickers'] as const).map(group => (
            <section key={group}>
              <h2>{group}</h2>
              {KNOBS.filter(k => k.group === group).map(knob => (
                <label key={knob.key} className="lab-knob">
                  <span>
                    {knob.label}
                    <output>
                      {look[knob.key]}
                      {knob.unit}
                    </output>
                  </span>
                  <input
                    id={`knob-${knob.key}`}
                    type="range"
                    min={knob.min}
                    max={knob.max}
                    step={knob.step}
                    value={look[knob.key]}
                    onChange={e => set({ [knob.key]: Number(e.target.value) } as Partial<Look>)}
                  />
                </label>
              ))}
            </section>
          ))}

          <button type="button" className="primary lab-save" onClick={save}>
            Save this look
          </button>
          {note && <p className="lab-note">{note}</p>}

          <h2>Saved</h2>
          {kept.length === 0 && <p className="lab-note">Nothing saved yet.</p>}
          <ol className="lab-rail">
            {[...kept].reverse().map(item => (
              <li key={item.name}>
                <button type="button" onClick={() => setLook(normalise(item.look))}>
                  {item.name.replace('.json', '').replace('T', ' ')} · seed {item.look.seed}
                </button>
              </li>
            ))}
          </ol>
        </aside>
      </div>
    </LookProvider>
  );
}

// Saving rewrites look.json. The lab already holds that look, so it takes
// the update without starting over.
if (import.meta.hot) import.meta.hot.accept('../look/look.json', () => {});

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Lab />
  </StrictMode>,
);
