// Draws sim/out/charts/*.svg from the simulator's CSV files.
//   node sim/render-charts.mjs
import { mkdirSync, readFileSync, writeFileSync, existsSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseCsv, lifePanel, survivalPanel, sweepChart, pairChart, composite, standalone } from './charts.mjs';

const out = join(dirname(fileURLToPath(import.meta.url)), 'out');
const read = (dir, file) => parseCsv(readFileSync(join(dir, file), 'utf8'));

function scenarios(dir) {
  const knobs = read(dir, 'scenarios.csv');
  return [...new Set(knobs.map(k => k.scenario))];
}

const TITLES = { healthy: 'Healthy', fading: 'Fading', abandoned: 'Abandoned', paced: 'Paced', farmed: 'Farmed', 'night-shift': 'Night shift' };

function renderPreset(dir, label) {
  if (!existsSync(join(dir, 'scenarios.csv'))) return;
  const charts = join(dir, 'charts');
  mkdirSync(charts, { recursive: true });
  const names = scenarios(dir);
  const life = names.map(name => lifePanel(TITLES[name], '', read(dir, `${name}.csv`)));
  writeFileSync(join(charts, 'supply.svg'), composite(life, {
    title: `Supply and crash cart over 60 days (${label})`,
    legend: [['cb-s1', 'Supply, median with 10% to 90% band'], ['cb-s2', 'Crash cart']],
  }));
  const survival = names.map(name => survivalPanel(TITLES[name], '', [{ label, rows: read(dir, `${name}.csv`) }]));
  writeFileSync(join(charts, 'survival.svg'), composite(survival, {
    title: `Share of 1,000 runs still alive, by day (${label})`,
  }));
  if (existsSync(join(dir, 'pair.csv'))) writeFileSync(join(charts, 'pair.svg'), standalone(pairChart(read(dir, 'pair.csv'))));
  if (existsSync(join(dir, 'sweep.csv'))) writeFileSync(join(charts, 'sweep.svg'), standalone(sweepChart(read(dir, 'sweep.csv'))));
  console.log(`charts: ${charts}`);
}

renderPreset(out, 'live preset');
const presets = join(out, 'presets');
const candidates = existsSync(presets) ? readdirSync(presets).sort() : [];
for (const name of candidates) renderPreset(join(presets, name), name);

// The candidates side by side: one survival curve per preset in each panel.
if (candidates.length) {
  const all = [['live', out], ...candidates.map(name => [name, join(presets, name)])];
  // Label each candidate by the one thing that differs: its arrest window.
  const hours = dir => `${read(dir, 'params.csv').find(r => r.parameter === 'ARREST_BLOCKS').value / 6000} h`;
  const panels = scenarios(out).map(name =>
    survivalPanel(TITLES[name], '', all.map(([, dir]) => ({ label: hours(dir), rows: read(dir, `${name}.csv`) }))));
  writeFileSync(join(out, 'charts', 'survival-candidates.svg'), composite(panels, {
    title: 'Share of 1,000 runs still alive, by day, for each candidate preset',
    legend: all.map(([label, dir], i) => [`cb-s${i + 1}`, `${label}: ${hours(dir)} of silence stops the heart`]),
  }));
}
