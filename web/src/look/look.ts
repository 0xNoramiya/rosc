// The look of the page, as one plain object.
//
// Three traditions, each a real process whose limits are the style:
//   - plasticine stop-motion: the patient. Stepped time, no outlines, solid
//     colours, a contact shadow, faces that change by swapping parts.
//   - ink-and-paint cartoons: the timing. Everything lands on the beat and a
//     press answers within one drawing.
//   - thermal chart paper and stencil-duplicator print: everything else. One
//     burned trace on a pink millimetre grid; three inks that overprint,
//     never quite register, and make their tints out of dots.
//
// `styleOf(look)` is a pure function: the same object, seed included, always
// gives the same page. The lab at /lab.html edits one of these and saves it
// to look.json, which is the file the real page reads.

export interface Look {
  seed: number;

  // ── Modes ────────────────────────────────────────────────────────────
  inks: keyof typeof INKS;
  patient: 'clay' | 'drawn';
  wall: 'plain' | 'dots' | 'plasters' | 'pulses';
  type: 'plex' | 'pixel' | 'matrix';

  // ── Knobs ────────────────────────────────────────────────────────────
  poseRate: number;
  bounce: number;
  squash: number;
  boil: number;
  mm: number;
  gridInk: number;
  trace: number;
  warmth: number;
  register: number;
  dot: number;
  dotAngle: number;
  density: number;
  mottle: number;
  border: number;
  tilt: number;
  shadow: number;
}

/// Three inks and the paper they are printed on, in OKLCH: lightness, then
/// chroma as a share of what the screen can show at that lightness and hue,
/// then hue. The first ink is the darkest: it carries every key line and all
/// the type, so it stays at L 0.40 or below. The two light inks are siblings:
/// one lightness and one share of chroma between them, so neither outweighs
/// the other. `pink` and `gold` are the hues of those two plates; the names
/// say which plate, not which colour (Candy's second plate is aqua). `sheet`
/// is plain L C H.
export const INKS = {
  ward: { name: 'Ward', ink: [0.4, 0.88, 260], light: [0.8, 0.97], pink: 355, gold: 88, sheet: [0.965, 0.008, 258] },
  candy: { name: 'Candy', ink: [0.38, 0.85, 292], light: [0.8, 0.97], pink: 18, gold: 190, sheet: [0.968, 0.007, 292] },
  tonic: { name: 'Tonic', ink: [0.4, 0.92, 238], light: [0.82, 0.97], pink: 340, gold: 108, sheet: [0.968, 0.007, 238] },
  duplicator: { name: 'Duplicator', ink: [0.35, 0.8, 300], light: [0.8, 0.97], pink: 30, gold: 86, sheet: [0.965, 0.008, 300] },
} as const;

export const MODES = {
  inks: { label: 'Ink set', options: Object.fromEntries(Object.entries(INKS).map(([k, v]) => [k, v.name])) },
  patient: { label: 'Patient', options: { clay: 'Clay', drawn: 'Ink and paint' } },
  wall: { label: 'Wallpaper', options: { plain: 'Plain', dots: 'Dots', plasters: 'Plasters', pulses: 'Pulses' } },
  type: { label: 'Thermal type', options: { plex: 'Typewriter', pixel: 'Pixel', matrix: 'Dot matrix' } },
} as const;

export interface Knob {
  key: Exclude<keyof Look, 'seed' | keyof typeof MODES>;
  label: string;
  group: 'Motion' | 'Paper strip' | 'Print' | 'Stickers';
  min: number;
  max: number;
  step: number;
  unit: string;
}

/// Every taste decision. The bounds are the range each tradition actually
/// covers, and every value inside them gives a sound page.
export const KNOBS: Knob[] = [
  { key: 'poseRate', label: 'Poses a second', group: 'Motion', min: 8, max: 24, step: 1, unit: '' },
  { key: 'bounce', label: 'Bounce on a beat', group: 'Motion', min: 0, max: 8, step: 0.5, unit: '%' },
  { key: 'squash', label: 'Squash on a press', group: 'Motion', min: 0.82, max: 0.95, step: 0.01, unit: '' },
  { key: 'boil', label: 'Boil', group: 'Motion', min: 0, max: 1.5, step: 0.1, unit: '' },
  { key: 'mm', label: 'Grid square', group: 'Paper strip', min: 3, max: 8, step: 1, unit: 'px' },
  { key: 'gridInk', label: 'Grid ink', group: 'Paper strip', min: 0.15, max: 0.6, step: 0.05, unit: '' },
  { key: 'trace', label: 'Trace weight', group: 'Paper strip', min: 1.5, max: 3.5, step: 0.25, unit: 'px' },
  { key: 'warmth', label: 'Paper warmth', group: 'Paper strip', min: 0, max: 10, step: 1, unit: '%' },
  { key: 'register', label: 'Misregistration', group: 'Print', min: 0, max: 3, step: 0.25, unit: 'px' },
  { key: 'dot', label: 'Halftone cell', group: 'Print', min: 3, max: 8, step: 0.5, unit: 'px' },
  { key: 'dotAngle', label: 'Halftone angle', group: 'Print', min: 0, max: 75, step: 5, unit: '°' },
  { key: 'density', label: 'Ink density', group: 'Print', min: 0.7, max: 1, step: 0.02, unit: '' },
  { key: 'mottle', label: 'Mottle', group: 'Print', min: 0, max: 0.15, step: 0.01, unit: '' },
  { key: 'border', label: 'Sticker border', group: 'Stickers', min: 2, max: 6, step: 0.5, unit: 'px' },
  { key: 'tilt', label: 'Sticker tilt', group: 'Stickers', min: 0, max: 3, step: 0.25, unit: '°' },
  { key: 'shadow', label: 'Sticker shadow', group: 'Stickers', min: 0, max: 6, step: 0.5, unit: 'px' },
];

export const DEFAULT_LOOK: Look = {
  seed: 302,
  inks: 'ward',
  patient: 'clay',
  wall: 'dots',
  type: 'plex',
  poseRate: 12,
  bounce: 5,
  squash: 0.86,
  boil: 0.6,
  mm: 5,
  gridInk: 0.35,
  trace: 2.25,
  warmth: 4,
  register: 1.5,
  dot: 5,
  dotAngle: 30,
  density: 1,
  mottle: 0.06,
  border: 4,
  tilt: 1.25,
  shadow: 3.5,
};

/// Fills in anything a saved file lacks and clamps what it has, so an old or
/// hand-edited look.json can never reach a broken page.
export function normalise(saved: Partial<Look> | null | undefined): Look {
  const look: Look = { ...DEFAULT_LOOK, ...(saved ?? {}) };
  for (const knob of KNOBS) {
    const value = Number(look[knob.key]);
    look[knob.key] = Number.isFinite(value) ? Math.min(knob.max, Math.max(knob.min, value)) : DEFAULT_LOOK[knob.key];
  }
  for (const key of Object.keys(MODES) as (keyof typeof MODES)[]) {
    if (!(look[key] in MODES[key].options)) (look as unknown as Record<string, unknown>)[key] = DEFAULT_LOOK[key];
  }
  look.seed = Math.abs(Math.trunc(Number(look.seed))) || DEFAULT_LOOK.seed;
  return look;
}

/// mulberry32. All of the look's chance comes from here.
export function prng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// ── Colour arithmetic. Colours are held as OKLCH triples: L says how light,
// C how vivid, H which hue, and each can be moved without disturbing the
// others. ───────────────────────────────────────────────────────────────
type Lch = readonly [number, number, number];
type Rgb = [number, number, number];

function linear([L, C, H]: Lch): Rgb {
  const a = C * Math.cos((H * Math.PI) / 180);
  const b = C * Math.sin((H * Math.PI) / 180);
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s, -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s];
}
const gamma = (v: number) => (v <= 0.0031308 ? v * 12.92 : 1.055 * v ** (1 / 2.4) - 0.055);
const degamma = (v: number) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
const srgb = (c: Lch): Rgb => linear(c).map(v => gamma(Math.min(1, Math.max(0, v)))) as Rgb;

function lch(rgb: Rgb): Lch {
  const [r, g, b] = rgb.map(degamma);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  const a = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s;
  const bb = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, Math.hypot(a, bb), ((Math.atan2(bb, a) * 180) / Math.PI + 360) % 360];
}

/// The most chroma an sRGB screen can show at this lightness and hue.
function ceiling(L: number, H: number): number {
  let [lo, hi] = [0, 0.4];
  for (let i = 0; i < 24; i++) {
    const mid = (lo + hi) / 2;
    if (linear([L, mid, H]).every(v => v >= -1e-5 && v <= 1.00001)) lo = mid;
    else hi = mid;
  }
  return lo;
}
/// A colour at a share of its own ceiling, so two hues look equally vivid.
const vivid = (L: number, share: number, H: number): Lch => [L, ceiling(L, H) * share, H];
/// Never ask the screen for more chroma than it has.
const fit = ([L, C, H]: Lch): Lch => [L, Math.min(C, ceiling(L, H)), H];

/// APCA lightness contrast, as a magnitude.
function contrast(text: Lch, ground: Lch): number {
  const y = (c: Lch) => {
    const [r, g, b] = srgb(c);
    const v = 0.2126729 * r ** 2.4 + 0.7151522 * g ** 2.4 + 0.072175 * b ** 2.4;
    return v > 0.022 ? v : v + (0.022 - v) ** 1.414;
  };
  const [t, g] = [y(text), y(ground)];
  const s = g > t ? (g ** 0.56 - t ** 0.57) * 1.14 : (g ** 0.65 - t ** 0.62) * 1.14;
  return Math.abs(s) < 0.1 ? 0 : (Math.abs(s) - 0.027) * 100;
}
/// Makes type readable on a ground by moving its lightness only. Hue and
/// vividness are what the colour is; lightness is what contrast is made of.
function legible(text: Lch, ground: Lch, target: number): Lch {
  let [L, C, H] = text;
  const down = ground[0] > 0.6;
  for (let i = 0; i < 60 && contrast(fit([L, C, H]), ground) < target; i++) L += down ? -0.01 : 0.01;
  return fit([Math.min(1, Math.max(0, L)), C, H]);
}

/// Ink printed over ink: each channel multiplies, as translucent ink does.
const over = (a: Lch, b: Lch): Lch => lch(srgb(a).map((v, i) => v * srgb(b)[i]) as Rgb);
/// Chroma is rounded down, so a colour at its ceiling stays inside it.
const css = ([L, C, H]: Lch) => `oklch(${+L.toFixed(3)} ${Math.floor(C * 1000) / 1000} ${+H.toFixed(1)})`;
/// Hex, for the generated pattern images only.
const hex = (c: Lch) => `#${srgb(c).map(v => Math.round(v * 255).toString(16).padStart(2, '0')).join('')}`;
const svg = (body: string, w: number, h: number) =>
  `url("data:image/svg+xml,${encodeURIComponent(`<svg xmlns='http://www.w3.org/2000/svg' width='${w}' height='${h}' viewBox='0 0 ${w} ${h}'>${body}</svg>`)}")`;

const FONTS = {
  plex: "'IBM Plex Mono', ui-monospace, 'SF Mono', Menlo, monospace",
  pixel: "'VT323', ui-monospace, monospace",
  matrix: "'Doto', 'IBM Plex Mono', ui-monospace, monospace",
};

export interface Style {
  /** CSS custom properties for the page root. */
  vars: Record<string, string>;
  /** Colours the canvas trace needs as plain values. */
  strip: { paper: string; fine: string; heavy: string; burn: string; mm: number; trace: number; font: string };
  /** Milliseconds one pose is held. */
  step: number;
}

export function styleOf(input: Look): Style {
  const look = normalise(input);
  const rand = prng(look.seed);
  const set = INKS[look.inks];
  const white: Lch = [1, 0, 0];
  const sheet: Lch = set.sheet;
  // The key ink prints at full strength: it is the type. Density thins the
  // two colour plates, which only ever carry fills. Thinner ink is lighter
  // and less vivid, and the two stay siblings: one lightness, and the same
  // share of what each hue can reach there.
  const ink = vivid(set.ink[0], set.ink[1], set.ink[2]);
  const plateL = set.light[0] + (1 - set.light[0]) * (1 - look.density);
  const pink = vivid(plateL, set.light[1] * look.density, set.pink);
  const gold = vivid(plateL, set.light[1] * look.density, set.gold);
  const burn: Lch = [0.23, 0.012, 285];

  // Each colour plate is one sheet pass, so the whole plate shifts together.
  const plate = () => {
    const angle = rand() * Math.PI * 2;
    return [Math.cos(angle) * look.register, Math.sin(angle) * look.register].map(v => `${v.toFixed(2)}px`);
  };
  const [px, py] = plate();
  const [gx, gy] = plate();

  // A tint is dots of full ink, never thinner ink. One screen angle per plate.
  const dots = (colour: Lch, angle: number, cover: number) => {
    const cell = look.dot;
    const r = (cell * Math.sqrt(cover / Math.PI)).toFixed(2);
    return svg(
      `<defs><pattern id='d' width='${cell}' height='${cell}' patternUnits='userSpaceOnUse' patternTransform='rotate(${angle.toFixed(1)})'><circle cx='${cell / 2}' cy='${cell / 2}' r='${r}' fill='${hex(colour)}'/></pattern></defs><rect width='120' height='120' fill='url(%23d)'/>`.replace('%23', '#'),
      120,
      120,
    );
  };
  const jitter = () => (rand() - 0.5) * 14;
  const angles = [look.dotAngle + jitter(), look.dotAngle + 30 + jitter(), look.dotAngle + 60 + jitter()];

  // Solids mottle: uneven ink, the same pattern for a given seed.
  const noise = Math.floor(rand() * 9000);
  const grain = svg(
    `<filter id='n'><feTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' seed='${noise}'/><feColorMatrix values='0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 -1.4 1.05'/></filter><rect width='240' height='240' filter='url(%23n)'/>`.replace('%23', '#'),
    240,
    240,
  );

  // The wallpaper: one motif, scattered by the seed, in the two light inks.
  const motif = (x: number, y: number, turn: number, tone: Lch, size: number) => {
    const colour = hex(tone);
    const at = `transform='translate(${x.toFixed(0)} ${y.toFixed(0)}) rotate(${turn.toFixed(0)}) scale(${size.toFixed(2)})'`;
    if (look.wall === 'plasters') return `<g ${at}><rect x='-16' y='-6' width='32' height='12' rx='6' fill='${colour}'/><rect x='-5' y='-5' width='10' height='10' rx='2' fill='#ffffff'/></g>`;
    if (look.wall === 'pulses') return `<path ${at} d='M-22 0h10l4-11 7 22 5-11h18' fill='none' stroke='${colour}' stroke-width='4' stroke-linecap='round' stroke-linejoin='round'/>`;
    return `<circle ${at} r='9' fill='${colour}'/>`;
  };
  let scatter = '';
  if (look.wall !== 'plain') {
    for (let i = 0; i < 26; i++) {
      const colour = rand() < 0.5 ? pink : gold;
      scatter += motif(20 + rand() * 440, 20 + rand() * 260, rand() * 360, colour, 0.6 + rand() * 0.9);
    }
  }
  const wall = svg(scatter, 480, 300);

  const tilts = Array.from({ length: 8 }, () => `${((rand() - 0.5) * 2 * look.tilt).toFixed(2)}deg`);
  // The room's wall is the sheet with half its tint: the same hue, lighter.
  const paper: Lch = [sheet[0] + (1 - sheet[0]) * 0.55, sheet[1] * 0.45, sheet[2]];
  // Chart paper and its grid are one ramp on the pink ink's hue: the grid is
  // printed in that ink, so lighter and darker steps must not drift.
  const stripPaper = fit([0.988 - look.warmth * 0.0012, 0.004 + look.warmth * 0.0016, set.pink]);
  const fine = fit([0.985 - 0.16 * look.gridInk, 0.018 + 0.1 * look.gridInk, set.pink]);
  const heavy = fit([0.985 - 0.36 * look.gridInk, 0.03 + 0.22 * look.gridInk, set.pink]);
  const step = 1000 / look.poseRate;

  // Where two plates cross, the inks multiply: these are the page's only
  // other colours. Type on a fill is the key ink printed heavier: the same
  // hue, darker, until it reads at 60 (body size) on that fill.
  const vars: Record<string, string> = {
    '--sheet': css(sheet),
    '--paper': css(paper),
    '--white': css(white),
    '--ink': css(ink),
    '--pink': css(pink),
    '--gold': css(gold),
    '--burn': css(burn),
    '--ink-pink': css(over(ink, pink)),
    '--ink-gold': css(over(ink, gold)),
    '--pink-gold': css(over(pink, gold)),
    '--on-pink': css(legible(ink, pink, 60)),
    '--on-gold': css(legible(ink, gold, 60)),
    '--ink-soft': css(legible([0.66, ink[1] * 0.55, ink[2]], white, 45)),
    // Shadows and rules are the burn colour laid thin; chart furniture is
    // the same hue, lighter.
    '--muted': css([0.53, burn[1], burn[2]]),
    '--axis': css([0.76, burn[1], burn[2]]),
    '--rule': css([0.9, burn[1] * 0.6, burn[2]]),
    '--px': px,
    '--py': py,
    '--gx': gx,
    '--gy': gy,
    '--dots-ink': dots(ink, angles[0], 0.22),
    '--dots-pink': dots(pink, angles[1], 0.34),
    '--dots-gold': dots(gold, angles[2], 0.42),
    '--grain': grain,
    '--mottle': String(look.mottle),
    '--wall': wall,
    '--strip': css(stripPaper),
    '--grid-fine': css(fine),
    '--grid-heavy': css(heavy),
    '--mm': `${look.mm}px`,
    '--step': `${step.toFixed(1)}ms`,
    '--bounce': `${look.bounce}%`,
    '--squash-y': String(look.squash),
    '--squash-x': (1 / look.squash).toFixed(3),
    '--boil': String(look.boil),
    '--border': `${look.border}px`,
    '--shadow': `${look.shadow}px`,
    '--thermal': FONTS[look.type],
  };
  tilts.forEach((t, i) => (vars[`--tilt-${i}`] = t));
  return { vars, strip: { paper: css(stripPaper), fine: css(fine), heavy: css(heavy), burn: css(burn), mm: look.mm, trace: look.trace, font: FONTS[look.type] }, step };
}
