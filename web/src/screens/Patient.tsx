// The patient: one memecoin, in bed. Everything it does comes from the chain.
// It bobs on each beat, gets sleepier as the arrest clock runs, wears one
// plaster per scar, and is squashed by every compression that counts.
//
// There are two of it. The clay one is photographs of a plasticine puppet,
// one per pose, swapped and never morphed. The drawn one is ink and flat
// colour, and is also what shows if a photograph fails to load.
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { RefObject } from 'react';
import art from '../../public/art/manifest.json';
import type { Monitor } from '../chain/types';
import { statusName } from '../chain/types';
import { rosc } from '../lib/format';
import type { Reading } from '../lib/trace';
import { useLook } from '../look/LookProvider';

const PLASTERS: [number, number, number][] = [
  [152, 112, -24],
  [258, 110, 22],
  [140, 152, 8],
  [270, 150, -14],
  [205, 92, 0],
  [236, 186, 30],
  [174, 188, -30],
  [205, 134, 90],
];

type Mood = 'well' | 'drowsy' | 'paced' | 'flat' | 'vf' | 'morgue';

export function moodOf(monitor: Monitor, reading: Reading): Mood {
  const status = statusName(monitor.status);
  if (status === 'arrest') return monitor.code?.rhythm === 'Vf' ? 'vf' : 'flat';
  if (status !== 'alive') return 'morgue';
  if (reading.rhythm === 'Paced') return 'paced';
  return reading.rhythm === 'Bradycardia' ? 'drowsy' : 'well';
}

/// A press on the page squashes the coin at once, before the chain has
/// answered. Compressions the chain reports from other rescuers do the same.
function usePresses(chain: string): number {
  const [count, setCount] = useState(0);
  const mine = useRef(0);
  useEffect(() => {
    const onPress = () => {
      mine.current = performance.now();
      setCount(c => c + 1);
    };
    window.addEventListener('codeblue:press', onPress);
    return () => window.removeEventListener('codeblue:press', onPress);
  }, []);
  // What the chain already showed when the page opened is not a new press.
  const seen = useRef(chain);
  useEffect(() => {
    if (seen.current === chain) return;
    seen.current = chain;
    if (performance.now() - mine.current > 2500) setCount(c => c + 1);
  }, [chain]);
  return count;
}

interface Props {
  monitor: Monitor;
  reading: Reading;
  /** A save just happened: the patient does its take. */
  taking?: boolean;
}

const pressKey = (monitor: Monitor) => `${monitor.code?.code_no ?? 0}-${monitor.code?.last_compression_block ?? 0}-${monitor.code?.cycle_compressions ?? 0}`;
/// How far the arrest clock has run: 0 just after a beat, 1 at the arrest.
const silenceOf = (monitor: Monitor) => Math.min(1, Math.max(0, (monitor.block - monitor.last_beat_block) / monitor.config.arrest_blocks));

function describe(monitor: Monitor, mood: Mood): string {
  if (mood === 'morgue') return 'The coin is under the blanket. Time of death was called.';
  if (mood === 'flat' || mood === 'vf') return `The coin is flat on its back. ${monitor.code?.first_compression_block ? 'Rescuers are doing compressions.' : 'Nobody is on the chest yet.'}`;
  return `The coin is sitting up in bed${mood === 'drowsy' ? ', nodding off' : ''}${mood === 'paced' ? ', kept going by a pacemaker' : ''}, with ${monitor.scars} ${monitor.scars === 1 ? 'plaster' : 'plasters'}.`;
}

export function Patient(props: Props) {
  const { look } = useLook();
  const [broken, setBroken] = useState(false);
  if (look.patient === 'clay' && !broken) return <ClayPatient {...props} onBroken={() => setBroken(true)} />;
  return <DrawnPatient {...props} />;
}

// ── The clay patient. Every size and position below is read from the
// manifest the sprites were measured into, in units of the bed's width. ──
const BED = art.sprites.bed.size;
const BED_H = BED[1] / BED[0];
const COIN = art.scene.coin_box_in_bed_widths;
const HANDS = art.scene.hands.box_in_coin_widths;
const pct = (share: number) => `${(share * 100).toFixed(2)}%`;
/// A box given in bed widths, as CSS percentages of the scene.
const box = (left: number, top: number, width: number, height: number) => ({ left: pct(left), top: pct(top / BED_H), width: pct(width), height: pct(height / BED_H) });
const COIN_BOX = box(COIN.left, COIN.top, COIN.size, COIN.size);
const handsBox = (pose: 'up' | 'down') => box(COIN.left + HANDS[pose].left * COIN.size, COIN.top + HANDS[pose].top * COIN.size, HANDS[pose].width * COIN.size, HANDS[pose].height * COIN.size);
const [CONTACT_X, CONTACT_Y] = art.scene.contact_on_bed.sitting;
const RIM = (art.coin.rim_diameter_px.sitting / art.coin.canvas[0]) * COIN.size;
const src = (file: string) => `${import.meta.env.BASE_URL}art/${file}`;

const POSES = ['well', 'drowsy', 'asleep', 'take', 'flat', 'flat-pressed', 'covered'] as const;
type Pose = (typeof POSES)[number];

/// Plays an element's CSS animation again from the start. The pictures stay
/// mounted, so nothing flickers while one is swapped for another.
function useReplay(ref: RefObject<HTMLElement | null>, trigger: unknown) {
  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;
    element.style.animation = 'none';
    void element.offsetWidth;
    element.style.animation = '';
  }, [ref, trigger]);
}

function ClayPatient({ monitor, reading, taking = false, onBroken }: Props & { onBroken: () => void }) {
  const { style } = useLook();
  const mood = moodOf(monitor, reading);
  const code = monitor.code;
  const presses = usePresses(pressKey(monitor));
  const working = mood === 'flat' || mood === 'vf';
  const sitting = !working && mood !== 'morgue';
  const silence = silenceOf(monitor);

  // A press is two poses of the squashed picture, then back.
  const [down, setDown] = useState(false);
  useEffect(() => {
    if (presses === 0) return;
    setDown(true);
    const timer = setTimeout(() => setDown(false), style.step * 2);
    return () => clearTimeout(timer);
  }, [presses, style.step]);
  // Hands arrive with the first press of a code and stay for the rest of it.
  const handsFor = useRef<number | null>(null);
  if (down && code) handsFor.current = code.code_no;
  const onChest = working && code != null && (code.first_compression_block != null || handsFor.current === code.code_no);

  const pose: Pose =
    mood === 'morgue' ? 'covered' : working ? (down ? 'flat-pressed' : 'flat') : taking ? 'take' : silence > 0.8 ? 'asleep' : mood === 'drowsy' || silence > 0.45 ? 'drowsy' : 'well';

  const coin = useRef<HTMLDivElement>(null);
  useReplay(coin, sitting ? `${monitor.last_beat_block}-${taking}` : mood);
  const motion = taking && sitting ? 'is-take' : sitting ? 'is-bob' : mood === 'vf' ? 'is-shiver' : '';
  const scars = Math.min(monitor.scars, 12);

  return (
    <div className={`clay is-${mood}`} role="img" aria-label={describe(monitor, mood)} style={{ aspectRatio: `${BED[0]} / ${BED[1]}` }}>
      <img className="clay-bed" src={src(art.sprites.bed.file)} alt="" onError={onBroken} />
      {/* Everything on a table has a shadow where it touches. */}
      <span className="clay-shadow" style={{ left: pct(CONTACT_X), top: pct(CONTACT_Y), width: pct(RIM * (sitting ? 0.82 : 1.2)) }} />
      <div className="clay-figure">
        <div ref={coin} className={`clay-coin ${motion}`} style={COIN_BOX}>
          {POSES.map(name => (
            <img key={name} className={name === pose ? 'is-on' : undefined} src={src(art.sprites[`coin-${name}`].file)} alt="" onError={onBroken} />
          ))}
        </div>
        <img className={`clay-hands${onChest ? ' is-on' : ''}`} src={src(art.sprites.hands.file)} alt="" style={handsBox(down ? 'down' : 'up')} onError={onBroken} />
      </div>
      {/* A breath: a puff for each one that counted */}
      {working && (code?.cycle_breaths ?? 0) > 0 && (
        <span className="clay-puff" key={`breath-${code?.code_no}-${code?.cycle}-${code?.cycle_breaths}`}>
          <i />
          <i />
          <i />
        </span>
      )}
      {mood === 'paced' && <span className="clay-pacer">pacemaker</span>}
      {scars > 0 && (
        <span className="clay-plasters">
          {Array.from({ length: scars }, (_, i) => (
            <i key={i} />
          ))}
        </span>
      )}
    </div>
  );
}

function DrawnPatient({ monitor, reading, taking = false }: Props) {
  const mood = moodOf(monitor, reading);
  const code = monitor.code;
  const presses = usePresses(pressKey(monitor));
  // Eyelids: open after a beat, closing as the silence grows.
  const lid = mood === 'well' || mood === 'paced' || mood === 'drowsy' ? 6 + silenceOf(monitor) * 22 : 0;
  const scars = Math.min(monitor.scars, PLASTERS.length);
  const says = describe(monitor, mood);

  return (
    <svg className={`patient is-${mood}${taking ? ' is-taking' : ''}`} viewBox="0 0 400 320" role="img" aria-label={says}>
      <clipPath id="eye-l">
        <ellipse cx="178" cy="138" rx="13" ry="15" />
      </clipPath>
      <clipPath id="eye-r">
        <ellipse cx="232" cy="138" rx="13" ry="15" />
      </clipPath>

      {/* The bed */}
      <rect className="p-wood" x="26" y="92" width="46" height="186" rx="16" />
      <rect className="p-wood" x="338" y="176" width="34" height="102" rx="13" />
      <rect className="p-wood" x="48" y="236" width="312" height="28" rx="10" />
      <rect className="p-wood" x="60" y="262" width="16" height="34" rx="6" />
      <rect className="p-wood" x="328" y="262" width="16" height="34" rx="6" />
      <rect className="p-sheet" x="62" y="204" width="288" height="40" rx="16" />
      <rect className="p-pillow" x="112" y="84" width="186" height="136" rx="44" />

      {/* A pacemaker, when two wallets are doing all the beating */}
      <g className="p-pacer">
        <path className="p-wire" d="M330 150 C310 120 290 150 268 132" />
        <rect className="p-box" x="318" y="140" width="46" height="34" rx="8" />
        <circle className="p-blink" cx="332" cy="157" r="5" />
        <path className="p-line" d="M342 157h14" />
      </g>

      {/* The coin. Each beat restarts the bob; each compression restarts the squash. */}
      <g className="p-squash" key={`press-${presses}`}>
        <g className="p-bob" key={`beat-${monitor.last_beat_block}`}>
          <g className="p-coin">
            <circle className="p-rim" cx="205" cy="150" r="80" />
            <circle className="p-face" cx="205" cy="150" r="67" />
            <path className="p-shine" d="M158 112 a62 62 0 0 1 50 -26" />
            <path className="p-emblem" d="M184 108h9l4-9 7 16 5-7h11" />
            <circle className="p-cheek" cx="158" cy="162" r="10" />
            <circle className="p-cheek" cx="252" cy="162" r="10" />

            <g className="p-eyes-open">
              <ellipse className="p-white" cx="178" cy="138" rx="13" ry="15" />
              <ellipse className="p-white" cx="232" cy="138" rx="13" ry="15" />
              <circle className="p-pupil" cx="179" cy="142" r="6" />
              <circle className="p-pupil" cx="233" cy="142" r="6" />
              <rect className="p-lid" clipPath="url(#eye-l)" x="163" y="122" width="30" height={lid} />
              <rect className="p-lid" clipPath="url(#eye-r)" x="217" y="122" width="30" height={lid} />
              <path className="p-line" d="M165 136q13-9 26 0M219 136q13-9 26 0" style={{ transform: `translateY(${lid - 12}px)` }} />
            </g>
            <path className="p-eyes-shut p-line" d="M166 140q12 8 24 0M220 140q12 8 24 0" />

            <path className="p-mouth m-well p-line" d="M192 172q13 12 26 0" />
            <path className="p-mouth m-drowsy p-line" d="M197 177h16" />
            <path className="p-mouth m-flat p-line" d="M193 180q12-8 24 0" />
            <path className="p-mouth m-vf p-line" d="M188 178l7-7 7 7 7-7 7 7 7-7" />

            {PLASTERS.slice(0, scars).map(([x, y, turn], i) => (
              <g key={i} transform={`rotate(${turn} ${x} ${y})`}>
                <rect className="p-plaster" x={x - 13} y={y - 5} width="26" height="10" rx="5" />
                <rect className="p-pad" x={x - 4} y={y - 4} width="8" height="8" rx="2" />
              </g>
            ))}
          </g>
        </g>
        {/* Two hands on the chest during a code */}
        <g className="p-hands">
          <ellipse className="p-glove" cx="188" cy="62" rx="24" ry="16" transform="rotate(-14 188 62)" />
          <ellipse className="p-glove" cx="222" cy="60" rx="24" ry="16" transform="rotate(12 222 60)" />
          <path className="p-line" d="M178 44v-22M232 42v-22" />
        </g>
      </g>

      {/* A breath: a puff for each one that counted */}
      <g className="p-puff" key={`breath-${code?.code_no ?? 0}-${code?.cycle}-${code?.cycle_breaths ?? 0}`}>
        {(code?.cycle_breaths ?? 0) > 0 && (
          <>
            <circle cx="262" cy="186" r="9" />
            <circle cx="278" cy="176" r="12" />
            <circle cx="298" cy="170" r="8" />
          </>
        )}
      </g>

      {/* The blanket, and two stubby arms resting on it */}
      <path className="p-blanket" d="M58 204q24-22 48-2t48 0 48 0 48 0 48 0 50-2v44a16 16 0 0 1-16 16H74a16 16 0 0 1-16-16z" />
      <path className="p-blanket p-shroud" d="M58 204C80 110 150 56 205 56s125 54 147 148v40a16 16 0 0 1-16 16H74a16 16 0 0 1-16-16z" />
      <g className="p-arms">
        <ellipse className="p-rim" cx="150" cy="206" rx="22" ry="13" transform="rotate(-12 150 206)" />
        <ellipse className="p-rim" cx="262" cy="206" rx="22" ry="13" transform="rotate(12 262 206)" />
        <rect className="p-band" x="250" y="197" width="10" height="20" rx="3" transform="rotate(12 255 207)" />
      </g>
      <g className="p-tag">
        <rect x="236" y="212" width="74" height="30" rx="6" transform="rotate(-6 273 227)" />
        <text x="273" y="232" textAnchor="middle" transform="rotate(-6 273 227)">
          {monitor.deaths} of {monitor.max_deaths}
        </text>
      </g>

      <text className="p-zzz" x="292" y="92">
        z
      </text>
      <text className="p-zzz p-zzz-2" x="312" y="68">
        z
      </text>
    </svg>
  );
}

/// The crash cart, hung as a drip. Its level is the cart against what it held at launch.
export function CrashCart({ monitor }: { monitor: Monitor }) {
  const launch = (BigInt(monitor.config.total_supply) * BigInt(Number(monitor.config.cart_bps))) / 10_000n;
  const level = launch > 0n ? Math.min(1, Number((BigInt(monitor.pools.cart) * 1000n) / launch) / 1000) : 0;
  const top = 44 + (1 - level) * 92;
  return (
    <figure className="cart">
      <svg viewBox="0 0 110 230" role="img" aria-label={`Crash cart: ${Math.round(level * 100)}% of what it held at launch`}>
        <clipPath id="bag">
          <rect x="22" y="40" width="66" height="100" rx="18" />
        </clipPath>
        <path className="p-line" d="M55 8v16M40 8h30" />
        <rect className="c-bag" x="22" y="40" width="66" height="100" rx="18" />
        <rect className="c-fluid" clipPath="url(#bag)" x="22" y={top} width="66" height={140 - top} />
        <rect className="c-outline" x="22" y="40" width="66" height="100" rx="18" />
        <path className="p-line" d="M55 24v16M55 140v14" />
        <rect className="c-chamber" x="47" y="154" width="16" height="26" rx="6" />
        <circle className="c-drop" cx="55" cy="162" r="3.5" />
        <path className="c-tube" d="M55 180c0 30 30 20 46 44" />
      </svg>
      <figcaption>
        <strong>{rosc(monitor.pools.cart)}</strong>
        <span>ROSC in the crash cart</span>
      </figcaption>
    </figure>
  );
}

/// Three lives. A death breaks one.
export function Lives({ monitor }: { monitor: Monitor }) {
  const hearts = Array.from({ length: monitor.max_deaths }, (_, i) => i < monitor.max_deaths - monitor.deaths);
  return (
    <figure className="lives">
      <svg viewBox={`0 0 ${hearts.length * 44} 40`} role="img" aria-label={`${monitor.deaths} of ${monitor.max_deaths} deaths used`}>
        {hearts.map((whole, i) => (
          <g key={i} transform={`translate(${i * 44 + 4} 2)`} className={whole ? 'l-whole' : 'l-broken'}>
            <path d="M18 34C4 24 0 16 0 10a9.500 9.500 0 0 1 18-4 9.500 9.500 0 0 1 18 4c0 6-4 14-18 24z" />
            {!whole && <path className="l-crack" d="M18 8l-5 8 8 4-5 10" />}
          </g>
        ))}
      </svg>
      <figcaption>
        <strong>
          {monitor.deaths} of {monitor.max_deaths}
        </strong>
        <span>deaths</span>
      </figcaption>
    </figure>
  );
}
