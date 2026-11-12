// Charts for the simulator's output files. Pure functions: CSV rows in, SVG
// strings out. `render-charts.mjs` writes them to sim/out/charts/ and the
// page's Simulation tab draws the same files with the same code.
//
// Colours come from CSS custom properties with light fallbacks, so a file on
// its own is a light chart and a page can restyle it for dark mode.

export function parseCsv(text) {
  const [head, ...lines] = text.trim().split('\n');
  const keys = head.split(',');
  return lines.map(line => {
    const cells = line.split(',');
    return Object.fromEntries(keys.map((k, i) => [k, cells[i] === '' || isNaN(cells[i]) ? cells[i] : Number(cells[i])]));
  });
}

export const CSS = `
.cb text{font-family:var(--cb-font,system-ui,-apple-system,"Segoe UI",sans-serif);font-size:11px;fill:var(--cb-muted,#898781)}
.cb .cb-title{font-size:13px;font-weight:600;fill:var(--cb-ink,#0b0b0b)}
.cb .cb-sub{fill:var(--cb-ink2,#52514e)}
.cb .cb-lab{fill:var(--cb-ink2,#52514e)}
.cb .cb-surface{fill:var(--cb-surface,#fcfcfb)}
.cb .cb-grid{stroke:var(--cb-grid,#e1e0d9);stroke-width:1;fill:none}
.cb .cb-axis{stroke:var(--cb-axis,#c3c2b7);stroke-width:1;fill:none}
.cb .cb-goal{fill:var(--cb-ink,#0b0b0b);fill-opacity:.06}
.cb .cb-line{fill:none;stroke-width:2;stroke-linejoin:round;stroke-linecap:round}
.cb .cb-band{stroke:none;fill-opacity:.14}
.cb .cb-dot{stroke:var(--cb-surface,#fcfcfb);stroke-width:2}
.cb .cb-ring{fill:var(--cb-surface,#fcfcfb);stroke-width:2}
.cb .cb-s1{stroke:var(--cb-s1,#2a78d6)}.cb .cb-f1{fill:var(--cb-s1,#2a78d6)}
.cb .cb-s2{stroke:var(--cb-s2,#eb6834)}.cb .cb-f2{fill:var(--cb-s2,#eb6834)}
.cb .cb-s3{stroke:var(--cb-s3,#1baf7a)}.cb .cb-f3{fill:var(--cb-s3,#1baf7a)}
.cb .cb-o1{stroke:var(--cb-o1,#86b6ef)}.cb .cb-g1{fill:var(--cb-o1,#86b6ef)}
.cb .cb-o2{stroke:var(--cb-o2,#5598e7)}.cb .cb-g2{fill:var(--cb-o2,#5598e7)}
.cb .cb-o3{stroke:var(--cb-o3,#2a78d6)}.cb .cb-g3{fill:var(--cb-o3,#2a78d6)}
.cb .cb-o4{stroke:var(--cb-o4,#1c5cab)}.cb .cb-g4{fill:var(--cb-o4,#1c5cab)}
.cb .cb-o5{stroke:var(--cb-o5,#104281)}.cb .cb-g5{fill:var(--cb-o5,#104281)}
.cb .cb-o6{stroke:var(--cb-o6,#0d366b)}.cb .cb-g6{fill:var(--cb-o6,#0d366b)}
`;

const esc = s => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
const n = v => Number(v.toFixed(1));
const lin = (d0, d1, r0, r1) => v => r0 + ((v - d0) / (d1 - d0)) * (r1 - r0);

export function rosc(v) {
  if (v >= 1e9) return `${(v / 1e9).toFixed(2)}B`;
  if (v >= 1e6) return `${(v / 1e6).toFixed(1)}M`;
  if (v >= 1e3) return `${(v / 1e3).toFixed(1)}K`;
  return String(Math.round(v));
}
const pct = v => `${Math.round(v * 100)}%`;

function open(w, h, label) {
  return `<svg class="cb" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}" role="img" aria-label="${esc(label)}">`;
}

/** Title, hairline grid with y labels, baseline and x labels. */
function frame(g, { title, sub, x, y, xTicks, yTicks, xFmt = String, yFmt = String, xLabel, edge = true }) {
  let s = `<text class="cb-title" x="${g.l}" y="16">${esc(title)}</text>`;
  if (sub) s += `<text class="cb-sub" x="${g.l}" y="30">${esc(sub)}</text>`;
  for (const t of yTicks) {
    s += `<path class="cb-grid" d="M${g.l} ${n(y(t))}H${g.w - g.r}"/>`;
    s += `<text x="${g.l - 6}" y="${n(y(t) + 4)}" text-anchor="end">${esc(yFmt(t))}</text>`;
  }
  s += `<path class="cb-axis" d="M${g.l} ${g.h - g.b}H${g.w - g.r}"/>`;
  xTicks.forEach((t, i) => {
    const anchor = !edge ? 'middle' : i === 0 ? 'start' : i === xTicks.length - 1 ? 'end' : 'middle';
    s += `<text x="${n(x(t))}" y="${g.h - g.b + 15}" text-anchor="${anchor}">${esc(xFmt(t))}</text>`;
  });
  if (xLabel) s += `<text x="${n((g.l + g.w - g.r) / 2)}" y="${g.h - g.b + 31}" text-anchor="middle">${esc(xLabel)}</text>`;
  return s;
}

const path = (pts, step) =>
  pts.map(([px, py], i) => (i === 0 ? `M${n(px)} ${n(py)}` : step ? `H${n(px)}V${n(py)}` : `L${n(px)} ${n(py)}`)).join('');

function band(rows, x, y, key, lo, hi, cls) {
  const up = rows.map(r => [x(r[key]), y(r[hi])]);
  const down = rows.map(r => [x(r[key]), y(r[lo])]).reverse();
  return `<path class="cb-band ${cls}" d="${path(up)}L${down.map(p => `${n(p[0])} ${n(p[1])}`).join('L')}Z"/>`;
}

const P = { w: 340, h: 214, l: 44, r: 14, t: 40, b: 26 };
const DAYS = [0, 20, 40, 60];

/** Supply and crash cart over time for one scenario: median and 10% to 90% band. */
export function lifePanel(title, sub, rows) {
  const g = P;
  const x = lin(0, 60, g.l, g.w - g.r);
  const y = lin(0, 2.6e9, g.h - g.b, g.t);
  let s = open(g.w, g.h, `${title}: supply and crash cart over 60 days`);
  s += frame(g, { title, sub, x, y, xTicks: DAYS, yTicks: [0, 1e9, 2e9], xFmt: d => `day ${d}`, yFmt: v => (v ? `${v / 1e9}B` : '0') });
  const series = [
    { label: 'Supply', key: 'supply', stroke: 'cb-s1', fill: 'cb-f1' },
    { label: 'Crash cart', key: 'cart', stroke: 'cb-s2', fill: 'cb-f2' },
  ];
  for (const sr of series) {
    s += band(rows, x, y, 'day', `${sr.key}_p10`, `${sr.key}_p90`, sr.fill);
    s += `<path class="cb-line ${sr.stroke}" d="${path(rows.map(r => [x(r.day), y(r[`${sr.key}_p50`])]))}"/>`;
    const last = rows[rows.length - 1];
    s += `<circle class="cb-dot ${sr.fill}" cx="${n(x(last.day))}" cy="${n(y(last[`${sr.key}_p50`]))}" r="4"/>`;
    const end = last[`${sr.key}_p50`];
    const dy = sr.key === 'cart' && end > 0.3e9 ? 17 : -8;
    s += `<text class="cb-lab" x="${g.w - g.r - 8}" y="${n(y(end) + dy)}" text-anchor="end">${sr.label} ${rosc(end)}</text>`;
  }
  return {
    svg: s + '</svg>',
    hover: {
      ...g, xs: rows.map(r => r.day), px: rows.map(r => x(r.day)), xName: d => `Day ${d}`,
      series: series.map(sr => ({
        label: sr.label, cls: sr.stroke,
        text: rows.map(r => `${rosc(r[`${sr.key}_p50`])} (${rosc(r[`${sr.key}_p10`])} to ${rosc(r[`${sr.key}_p90`])})`),
      })),
    },
  };
}

/** Share of runs still alive by day. `curves` is [{label, rows}] with up to three entries. */
export function survivalPanel(title, sub, curves, key = 'alive_share') {
  const g = P;
  const x = lin(0, 60, g.l, g.w - g.r);
  const y = lin(0, 1, g.h - g.b, g.t);
  let s = open(g.w, g.h, `${title}: share of runs still alive by day`);
  s += frame(g, { title, sub, x, y, xTicks: DAYS, yTicks: [0, 0.5, 1], xFmt: d => `day ${d}`, yFmt: pct });
  const same = curves.every(c => c.rows.every((r, i) => r[key] === curves[0].rows[i][key]));
  curves.forEach((c, i) => {
    const cls = i + 1;
    s += `<path class="cb-line cb-s${cls}" d="${path(c.rows.map(r => [x(r.day), y(r[key])]), true)}"/>`;
    // Label each curve beside its drop through 50%, or under its end if it never drops that far.
    const cross = c.rows.find(r => r[key] <= 0.5);
    const at = cross ?? c.rows[c.rows.length - 1];
    const text = same ? (curves.length > 1 ? 'all the same' : '') : c.label;
    if (!text || (same && i > 0)) return;
    let lx, ly, anchor;
    if (cross) {
      [lx, ly, anchor] = [x(at.day) + 6, y(0.5) + 4 + (i - 1) * 15, 'start'];
      if (lx > g.w - g.r - 30) [lx, anchor] = [x(at.day) - 6, 'end'];
    } else {
      [lx, ly, anchor] = [g.w - g.r - 4, y(at[key]) + (at[key] > 0.9 && !same ? 16 : -7) + (same ? 0 : 0), 'end'];
      if (at[key] <= 0.9) ly = y(at[key]) + 16;
    }
    s += `<text class="cb-lab" x="${n(lx)}" y="${n(ly)}" text-anchor="${anchor}">${esc(text)}</text>`;
  });
  return {
    svg: s + '</svg>',
    hover: {
      ...g, xs: curves[0].rows.map(r => r.day), px: curves[0].rows.map(r => x(r.day)), xName: d => `Day ${d}`,
      series: curves.map((c, i) => ({ label: c.label, cls: `cb-s${i + 1}`, text: c.rows.map(r => `${(r[key] * 100).toFixed(1)}% alive`) })),
    },
  };
}

const WIDE = { w: 640, h: 400, l: 46, r: 64, t: 44, b: 62 };

/**
 * The trade-off behind ARREST_BLOCKS: the day a fading coin dies, by how busy
 * it was at launch, one line per arrest window. A hollow marker means a
 * healthy coin of that size would also arrest.
 */
export function sweepChart(rows, { reference = 60 } = {}) {
  const g = WIDE;
  const beats = [...new Set(rows.map(r => r.launch_beats_per_day))].sort((a, b) => a - b);
  const hours = [...new Set(rows.map(r => r.arrest_hours))].sort((a, b) => a - b);
  const lx = lin(Math.log(beats[0]), Math.log(beats[beats.length - 1]), g.l, g.w - g.r);
  const x = v => lx(Math.log(v));
  const y = lin(0, 60, g.h - g.b, g.t);
  let s = open(g.w, g.h, 'Day a fading coin dies, by launch activity and arrest window');
  s += `<rect class="cb-goal" x="${g.l}" y="${n(y(50))}" width="${g.w - g.r - g.l}" height="${n(y(20) - y(50))}"/>`;
  s += frame(g, {
    title: 'When a fading coin dies, by how busy it was at launch',
    sub: 'Median day of final death, one line per arrest window. Shaded: the goal, day 20 to day 50.',
    x, y, xTicks: beats, yTicks: [0, 20, 40, 60], yFmt: d => `day ${d}`, xLabel: 'beats per day at launch (log scale)', edge: false,
  });
  s += `<path class="cb-axis" d="M${n(x(reference))} ${g.t}V${g.h - g.b}"/>`;
  s += `<text class="cb-lab" x="${n(x(reference) - 6)}" y="${n(y(3.4))}" text-anchor="end">reference: ${reference} a day</text>`;
  const points = [];
  hours.forEach((h, i) => {
    const cls = i + 1;
    const line = rows.filter(r => r.arrest_hours === h && r.fading_death_day_median !== '');
    if (!line.length) return;
    s += `<path class="cb-line cb-o${cls}" d="${path(line.map(r => [x(r.launch_beats_per_day), y(r.fading_death_day_median)]))}"/>`;
    for (const r of line) {
      const safe = r.healthy_runs_with_arrest <= 0.01;
      const [cx, cy] = [x(r.launch_beats_per_day), y(r.fading_death_day_median)];
      s += safe
        ? `<circle class="cb-dot cb-g${cls}" cx="${n(cx)}" cy="${n(cy)}" r="5"/>`
        : `<circle class="cb-ring cb-o${cls}" cx="${n(cx)}" cy="${n(cy)}" r="4"/>`;
      points.push({
        cx, cy,
        text: `${h} h window, ${r.launch_beats_per_day} beats a day: dies day ${r.fading_death_day_median}; a healthy coin arrests in ${(r.healthy_runs_with_arrest * 100).toFixed(1)}% of runs`,
      });
    }
    const end = line[line.length - 1];
    s += `<text class="cb-lab" x="${n(x(end.launch_beats_per_day) + 10)}" y="${n(y(end.fading_death_day_median) + 4)}">${h} h</text>`;
  });
  const ky = g.h - 4;
  s += `<circle class="cb-dot cb-g3" cx="${g.l + 6}" cy="${ky - 4}" r="5"/><text class="cb-lab" x="${g.l + 16}" y="${ky}">a healthy coin this busy never arrests</text>`;
  s += `<circle class="cb-ring cb-o3" cx="${g.l + 286}" cy="${ky - 4}" r="4"/><text class="cb-lab" x="${g.l + 296}" y="${ky}">a healthy coin this busy arrests too</text>`;
  return { svg: s + '</svg>', hover: { ...g, points } };
}

/** Minutes from first compression to ROSC, by the scars the coin already has. */
export function pairChart(rows) {
  const g = WIDE;
  const teams = [...new Set(rows.map(r => r.team))];
  const x = lin(0, 22, g.l, g.w - g.r);
  const y = lin(0, 30, g.h - g.b, g.t);
  let s = open(g.w, g.h, 'Minutes to a save by scars, for three teams');
  s += `<rect class="cb-goal" x="${g.l}" y="${n(y(12))}" width="${g.w - g.r - g.l}" height="${n(y(4) - y(12))}"/>`;
  s += frame(g, {
    title: 'How long a save takes, by old damage',
    sub: 'First compression to ROSC: median and 10% to 90% band. Shaded: the goal, 4 to 12 minutes.',
    x, y, xTicks: [0, 5, 10, 15, 20], yTicks: [0, 10, 20, 30], yFmt: m => `${m} min`, xLabel: 'scars before the arrest', edge: false,
  });
  const hover = { ...g, xs: [], px: [], xName: v => `${v} scars`, series: [] };
  const scars = [...new Set(rows.map(r => r.scars))].sort((a, b) => a - b);
  hover.xs = scars;
  hover.px = scars.map(x);
  teams.forEach((team, i) => {
    const cls = i + 1;
    const line = rows.filter(r => r.team === team && r.saved_share >= 0.5 && r.minutes_p50 !== '');
    s += band(line, x, y, 'scars', 'minutes_p10', 'minutes_p90', `cb-f${cls}`);
    s += `<path class="cb-line cb-s${cls}" d="${path(line.map(r => [x(r.scars), y(r.minutes_p50)]))}"/>`;
    const end = line[line.length - 1];
    s += `<circle class="cb-dot cb-f${cls}" cx="${n(x(end.scars))}" cy="${n(y(end.minutes_p50))}" r="4"/>`;
    const lab = i === 0 ? [x(end.scars) + 8, y(end.minutes_p50) + 4, 'start'] : [x(end.scars) - 4, y(end.minutes_p50) - 9, 'end'];
    s += `<text class="cb-lab" x="${n(lab[0])}" y="${n(lab[1])}" text-anchor="${lab[2]}">${esc(team)}</text>`;
    hover.series.push({
      label: team, cls: `cb-s${cls}`,
      text: scars.map(sc => {
        const r = line.find(row => row.scars === sc);
        return r ? `${r.minutes_p50} min, ${r.rounds_p50} rounds` : 'not saveable';
      }),
    });
  });
  return { svg: s + '</svg>', hover };
}

/** Lays panels out in a grid inside one SVG, for a file that stands alone. */
export function composite(panels, { cols = 3, title, legend = [] } = {}) {
  const g = P;
  const gap = 16, top = title ? 56 : 12;
  const rowsN = Math.ceil(panels.length / cols);
  const w = cols * g.w + (cols + 1) * gap;
  const h = top + rowsN * (g.h + gap);
  let s = `<svg class="cb" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}" role="img" aria-label="${esc(title ?? 'Charts')}"><style>${CSS}</style>`;
  s += `<rect class="cb-surface" width="${w}" height="${h}"/>`;
  if (title) s += `<text class="cb-title" x="${gap + g.l}" y="24" style="font-size:15px">${esc(title)}</text>`;
  let kx = gap + g.l;
  for (const [cls, label] of legend) {
    s += `<path class="cb-line ${cls}" d="M${kx} 40h18"/><text class="cb-lab" x="${kx + 24}" y="44">${esc(label)}</text>`;
    kx += 44 + label.length * 7;
  }
  panels.forEach((p, i) => {
    const px = gap + (i % cols) * (g.w + gap), py = top + Math.floor(i / cols) * (g.h + gap);
    s += p.svg.replace('<svg class="cb"', `<svg x="${px}" y="${py}" width="${g.w}" height="${g.h}"`);
  });
  return s + '</svg>';
}

/** One wide chart as a file that stands alone. */
export function standalone(chart) {
  return chart.svg
    .replace(/(<svg class="cb"[^>]*>)/, `$1<style>${CSS}</style><rect class="cb-surface" width="100%" height="100%"/>`);
}
