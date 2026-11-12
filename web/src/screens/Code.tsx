// Code mode: the metronome, the buttons, the AED's voice and the numbers a
// team watches while they work.
import { useCallback, useEffect, useRef, useState } from 'react';
import type { Rescuer } from '../chain/rescuer';
import { refusal } from '../chain/sender';
import type { CodeSheet, Monitor } from '../chain/types';
import { phaseName, statusName } from '../chain/types';
import { band } from '../lib/band';
import { points, rosc, short, span } from '../lib/format';

type Method = 'compress' | 'breathe' | 'shock' | 'push_epi' | 'call_code';

export interface Press {
  id: number;
  method: Method;
  block: number;
  /** pending until the chain answers; counted or missed after. */
  state: 'pending' | 'counted' | 'missed';
  note?: string;
  sentAt: number;
  settledAt?: number;
}

interface Props {
  monitor: Monitor;
  blockMs: number;
  rescuer: Rescuer;
  sheet: CodeSheet | null;
  /** The last code has ended but the chain has not written its sheet yet. */
  sheetDue?: boolean;
  claimable: string;
  explorer: (hash: string) => string;
}

/// What the AED would say right now.
export function prompt(monitor: Monitor, me: string | null, blockMs: number): string {
  const status = statusName(monitor.status);
  const code = monitor.code;
  if (status === 'alive') return 'Patient has a pulse. No compressions needed.';
  if (status === 'deceased') return 'Time of death has been called.';
  if (status === 'morgue' && typeof monitor.status === 'object' && 'Morgue' in monitor.status) {
    return `Time of death was called. The next code opens in ${span(monitor.status.Morgue.until_block - monitor.block, blockMs)}.`;
  }
  if (!code) return '';
  if (code.first_compression_block === null) return 'No pulse. Start compressions.';
  const phase = phaseName(code.phase);
  if (phase === 'ventilation') {
    return code.cycle_lead === me ? 'Hands off. Someone else gives two breaths.' : `Give two breaths. ${code.cycle_breaths} of ${monitor.config.breaths_per_cycle} given.`;
  }
  if (phase === 'check') {
    if (code.shock_open) return 'Analyzing rhythm. Shock advised.';
    if (code.epi_open) return 'Analyzing rhythm. No shock advised. Epinephrine is due.';
    return 'No shock advised, resume compressions.';
  }
  return code.cycle_compressions >= 5 && code.etco2_bps < 6000 ? 'Push faster.' : 'Continue compressions.';
}

function useMetronome(blockMs: number, onBeat: () => void, sound: boolean) {
  const [beat, setBeat] = useState(0);
  const handler = useRef(onBeat);
  handler.current = onBeat;
  const audio = useRef<AudioContext | null>(null);

  useEffect(() => {
    let next = performance.now() + blockMs;
    let timer: ReturnType<typeof setTimeout>;
    const tick = () => {
      setBeat(b => b + 1);
      handler.current();
      if (sound) {
        const ctx = (audio.current ??= new AudioContext());
        const osc = ctx.createOscillator();
        const gain = ctx.createGain();
        osc.frequency.value = 880;
        gain.gain.setValueAtTime(0.12, ctx.currentTime);
        gain.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + 0.05);
        osc.connect(gain).connect(ctx.destination);
        osc.start();
        osc.stop(ctx.currentTime + 0.06);
      }
      next += blockMs;
      timer = setTimeout(tick, Math.max(0, next - performance.now()));
    };
    timer = setTimeout(tick, blockMs);
    return () => clearTimeout(timer);
  }, [blockMs, sound]);
  return beat;
}

export function CodeScreen({ monitor, blockMs, rescuer, sheet, sheetDue = false, claimable, explorer }: Props) {
  const [presses, setPresses] = useState<Press[]>([]);
  const [device, setDevice] = useState(false);
  const [sound, setSound] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  const [lastHash, setLastHash] = useState<string | null>(null);
  const usedDevice = useRef(false);
  const nextId = useRef(1);
  const blockRef = useRef(monitor.block);
  blockRef.current = monitor.block;

  const status = statusName(monitor.status);
  const code = monitor.code;
  const me = rescuer.accountId;
  const phase = code ? phaseName(code.phase) : null;
  const sender = rescuer.sender;

  const press = useCallback(
    async (method: Method) => {
      if (!sender) {
        setSaid('Scrub in first: the page needs its key before it can press.');
        return;
      }
      // The patient answers the press at once; the chain answers a second later.
      if (method === 'compress') window.dispatchEvent(new Event('codeblue:press'));
      const id = nextId.current++;
      setPresses(list => [...list.slice(-59), { id, method, block: blockRef.current, state: 'pending', sentAt: performance.now() }]);
      const settle = (state: Press['state'], note?: string) =>
        setPresses(list => list.map(p => (p.id === id && p.state === 'pending' ? { ...p, state, note, settledAt: performance.now() } : p)));
      try {
        const sent = await sender.send(method);
        setLastHash(sent.hash);
        const value = await sent.result;
        if (method === 'compress' || method === 'breathe') settle(value === 'counted' ? 'counted' : 'missed', String(value));
        else if (method === 'shock') {
          settle(value ? 'counted' : 'missed');
          setSaid(value ? 'Shock delivered.' : 'Shock delivered. The round was too weak for it to help.');
        } else settle('counted');
      } catch (error) {
        const message = refusal(error);
        settle('missed', message);
        setSaid(message);
      }
    },
    [sender],
  );

  // The chain's own tally arrives with every poll, sooner than a call's answer:
  // when this account's work units rise, its oldest pending presses counted.
  const myUnits = code?.units.find(([who]) => who === me)?.[1] ?? 0;
  const seenUnits = useRef({ code: 0, units: 0 });
  useEffect(() => {
    const codeNo = code?.code_no ?? 0;
    if (seenUnits.current.code !== codeNo) seenUnits.current = { code: codeNo, units: myUnits };
    let gained = myUnits - seenUnits.current.units;
    seenUnits.current.units = myUnits;
    if (gained <= 0) return;
    const worth: Record<Method, number> = {
      compress: Number(monitor.config.units_compression),
      breathe: Number(monitor.config.units_breath),
      shock: Number(monitor.config.units_shock),
      call_code: Number(monitor.config.units_call_code),
      push_epi: 0,
    };
    setPresses(list =>
      list.map(p => {
        if (p.state !== 'pending' || worth[p.method] === 0 || gained < worth[p.method]) return p;
        gained -= worth[p.method];
        return { ...p, state: 'counted' as const, settledAt: performance.now() };
      }),
    );
  }, [myUnits, code?.code_no, monitor.config]);

  // The device presses a little faster than the chain makes blocks. Measured
  // on testnet, 0.8 of the block interval lands a press in every block; the
  // contract ignores the extra ones.
  const beat = useMetronome(device ? blockMs * 0.8 : blockMs, () => {
    if (device && sender && status === 'arrest' && phase === 'compressions') {
      usedDevice.current = true;
      press('compress');
    }
  }, sound);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement;
      if (event.metaKey || event.ctrlKey || event.altKey || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName)) return;
      const map: Record<string, () => void> = {
        ' ': () => press('compress'),
        b: () => press('breathe'),
        s: () => press('shock'),
        e: () => press('push_epi'),
        c: () => press('call_code'),
        d: () => setDevice(on => !on),
        m: () => setSound(on => !on),
      };
      const action = map[event.key.toLowerCase()];
      if (!action || event.repeat) return;
      event.preventDefault();
      action();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [press]);

  // How long each press took to be marked, for the acceptance tests.
  // And the band: a tuba note for a compression that counted, a sour one for
  // a miss, a slide whistle for a breath.
  const played = useRef(new Set<number>());
  useEffect(() => {
    (window as unknown as { codebluePresses?: Press[] }).codebluePresses = presses;
    for (const p of presses) {
      if (p.state === 'pending' || played.current.has(p.id)) continue;
      played.current.add(p.id);
      if (p.state === 'missed') band.sour();
      else if (p.method === 'breathe') band.whistle();
      else if (p.method === 'compress') band.tuba();
    }
  }, [presses]);

  const counted = presses.filter(p => p.method === 'compress' && p.state === 'counted').length;
  const missed = presses.filter(p => p.method === 'compress' && p.state === 'missed').length;
  const line = said ?? prompt(monitor, me, blockMs);
  useEffect(() => setSaid(null), [phase, status, code?.round]);

  return (
    <div className="code">
      <p className="aed" role="status" aria-live="polite">
        {line}
      </p>

      {code && (
        <div className="code-numbers">
          <div className="vital v-round">
            <span className="v-name">Round</span>
            <span className="v-num">
              {code.round} of {monitor.config.max_rounds}
            </span>
            <span className="v-unit">
              cycle {code.cycle} of {monitor.config.cycles_per_round}
            </span>
          </div>
          <div className="vital v-count">
            <span className="v-name">Compressions</span>
            <span className="v-num">
              {code.cycle_compressions} of {monitor.config.compressions_per_cycle}
            </span>
            <span className="v-unit">
              breaths {code.cycle_breaths} of {monitor.config.breaths_per_cycle}
            </span>
          </div>
          <div className="vital v-etco2">
            <span className="v-name">ETCO2</span>
            <span className="v-num">{Math.round(code.etco2_bps / 100)}</span>
            <span className="v-unit">quality of this cycle, out of 100</span>
          </div>
          <div className="vital v-epi">
            <span className="v-name">Epinephrine</span>
            <span className="v-num">{code.epi_open ? 'Due now' : `After round ${nextDose(code.round, monitor)}`}</span>
            <span className="v-unit">{phase !== 'compressions' ? `window closes in ${Math.max(0, code.deadline_block - monitor.block)} blocks` : `rhythm: ${code.rhythm === 'Vf' ? 'VF, shockable' : code.rhythm === 'Pea' ? 'PEA, not shockable' : 'asystole, not shockable'}`}</span>
          </div>
          <div className="perfusion" role="meter" aria-valuemin={0} aria-valuemax={code.target} aria-valuenow={code.perfusion} aria-label="Perfusion against the target">
            <div className="perfusion-fill" style={{ width: `${Math.min(100, (code.perfusion / Math.max(1, code.target)) * 100)}%` }} />
            <span className="perfusion-text">
              Perfusion {points(code.perfusion)} of {points(code.target)} needed for ROSC
            </span>
          </div>
        </div>
      )}

      <div className="pads">
        <div className={`metronome ${beat % 2 ? 'is-tick' : 'is-tock'}`} aria-hidden="true">
          <span className="metronome-dot" key={beat} />
          <span className="metronome-rate">{Math.round(60000 / blockMs)} a minute</span>
        </div>
        <button type="button" className="pad pad-compress" onClick={() => press('compress')} disabled={status !== 'arrest'}>
          <span className="pad-big">Push</span>
          <span>
            one compression <kbd>Space</kbd>
          </span>
        </button>
        <button type="button" className="pad" onClick={() => press('breathe')} disabled={phase !== 'ventilation'}>
          Give breath <kbd>B</kbd>
        </button>
        <button type="button" className="pad" onClick={() => press('shock')} disabled={status !== 'arrest'}>
          Shock <kbd>S</kbd>
        </button>
        <button type="button" className="pad" onClick={() => press('push_epi')} disabled={status !== 'arrest'}>
          Push epinephrine <kbd>E</kbd>
        </button>
        <button type="button" className="pad" onClick={() => press('call_code')} disabled={status !== 'arrest' || !!code?.called}>
          Call the code <kbd>C</kbd>
        </button>
      </div>

      <div className="code-options">
        <label>
          <input id="device-mode" type="checkbox" checked={device} onChange={e => setDevice(e.target.checked)} /> Device presses on every beat <kbd>D</kbd>
        </label>
        <label>
          <input id="metronome-sound" type="checkbox" checked={sound} onChange={e => setSound(e.target.checked)} /> Metronome click <kbd>M</kbd>
        </label>
      </div>

      <div className="tally" aria-label="Your presses">
        <ol className="presses">
          {presses.slice(-40).map(p => (
            <li key={p.id} className={`press is-${p.state} m-${p.method}`} title={`${p.method} at block ${p.block}: ${p.note ?? p.state}`}>
              <span className="visually-hidden">
                {p.method} {p.state}
              </span>
            </li>
          ))}
        </ol>
        <p className="tally-text">
          <span className="key is-counted" /> counted {counted} <span className="key is-missed" /> missed {missed} <span className="key is-pending" /> in flight{' '}
          {presses.filter(p => p.state === 'pending').length}
          {lastHash && (
            <>
              {' '}
              <a href={explorer(lastHash)} target="_blank" rel="noreferrer">
                last transaction
              </a>
            </>
          )}
        </p>
      </div>

      {status !== 'arrest' && sheetDue && (
        <p className="note sheet-due">
          This code is over. The chain writes its sheet with the next transaction on the coin{BigInt(claimable) > 0n ? ', and claiming your bounty is one' : ''}.
        </p>
      )}
      {status !== 'arrest' && !sheetDue && sheet && <Sheet sheet={sheet} blockMs={blockMs} me={me} device={usedDevice.current} />}
      {BigInt(claimable) > 0n && (
        <button type="button" className="pad pad-claim" onClick={() => sender?.send('claim_bounty').then(s => s.result).then(() => setSaid('Bounty claimed.'), e => setSaid(refusal(e)))}>
          Claim {rosc(claimable)} ROSC bounty
        </button>
      )}
    </div>
  );
}

function nextDose(round: number, monitor: Monitor): number {
  const first = Number(monitor.config.epi_first_round);
  const every = Number(monitor.config.epi_every_rounds);
  let next = first;
  while (next < round) next += every;
  return next;
}

export function Sheet({ sheet, blockMs, me, device }: { sheet: CodeSheet; blockMs: number; me?: string | null; device?: boolean }) {
  const saved = sheet.outcome === 'Rosc';
  const reason = saved ? '' : { NoFlow: 'no compressions in time', Pause: 'compressions paused too long', RoundsExhausted: 'the last round ended below target' }[(sheet.outcome as { Death: string }).Death];
  return (
    <section className="sheet" aria-label={`Code sheet ${sheet.code_no}`}>
      <h3>
        Code {sheet.code_no}: {saved ? 'return of spontaneous circulation' : `time of death, ${reason}`}
      </h3>
      <dl className="sheet-facts">
        <div>
          <dt>Downtime</dt>
          <dd>
            {span(sheet.end_block - sheet.arrest_block, blockMs)} ({sheet.end_block - sheet.arrest_block} blocks)
          </dd>
        </div>
        <div>
          <dt>No flow</dt>
          <dd>{sheet.first_compression_block === null ? 'nobody came' : `${sheet.first_compression_block - sheet.arrest_block} blocks`}</dd>
        </div>
        <div>
          <dt>Rhythm at arrest</dt>
          <dd>{sheet.rhythm_at_arrest === 'Vf' ? 'VF' : sheet.rhythm_at_arrest === 'Pea' ? 'PEA' : 'Asystole'}</dd>
        </div>
        <div>
          <dt>Perfusion</dt>
          <dd>
            {points(sheet.perfusion)} of {points(sheet.target)}
          </dd>
        </div>
        <div>
          <dt>Bounty</dt>
          <dd>{rosc(sheet.bounty)} ROSC</dd>
        </div>
      </dl>
      {sheet.rounds.length > 0 && (
        <div className="scroll">
          <table>
            <thead>
              <tr>
                <th>Round</th>
                <th>Quality</th>
                <th>Lead compressor</th>
                <th>Shock</th>
                <th>Epinephrine</th>
                <th>Perfusion after</th>
              </tr>
            </thead>
            <tbody>
              {sheet.rounds.map(r => (
                <tr key={r.round}>
                  <td>{r.round}</td>
                  <td>
                    {(r.quality_bps / 100).toFixed(0)}%{r.fatigued ? ' (tired: same lead twice)' : ''}
                  </td>
                  <td>{short(r.lead)}</td>
                  <td>{r.shock === null ? 'none' : r.shock ? 'worked' : 'no effect'}</td>
                  <td>{r.epi ? 'given' : 'none'}</td>
                  <td>{points(r.perfusion_after)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <div className="scroll">
        <table>
          <thead>
            <tr>
              <th>Who</th>
              <th>Work units</th>
              <th>Share of the bounty</th>
            </tr>
          </thead>
          <tbody>
            {sheet.units.map(([who, units]) => (
              <tr key={who}>
                <td>
                  {short(who)}
                  {who === me ? (device ? ' (you, with the device)' : ' (you)') : ''}
                </td>
                <td>{units}</td>
                <td>{rosc(sheet.shares.find(([w]) => w === who)?.[1] ?? '0')} ROSC</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
