// Copies what the page reads from the rest of the repo into public/:
// the list of deployed coins and the simulator's files.
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const web = join(dirname(fileURLToPath(import.meta.url)), '..');
const repo = join(web, '..');

const deployments = join(repo, 'deployments');
const coins = existsSync(deployments)
  ? readdirSync(deployments).filter(f => f.endsWith('.json')).sort()
      .map(f => ({ name: f.replace('.json', ''), ...JSON.parse(readFileSync(join(deployments, f), 'utf8')) }))
  : [];
writeFileSync(join(web, 'public/deployments.json'), JSON.stringify(coins, null, 2));

const out = join(repo, 'sim/out');
const sim = join(web, 'public/sim');
mkdirSync(sim, { recursive: true });
const keep = f => f.endsWith('.csv') && !f.endsWith('.runs.csv') && !f.endsWith('.codes.csv');
if (existsSync(out)) {
  for (const f of readdirSync(out).filter(keep)) cpSync(join(out, f), join(sim, f));
  const presets = join(out, 'presets');
  for (const preset of existsSync(presets) ? readdirSync(presets) : []) {
    mkdirSync(join(sim, preset), { recursive: true });
    for (const f of readdirSync(join(presets, preset)).filter(keep)) cpSync(join(presets, preset, f), join(sim, preset, f));
  }
}
console.log(`synced ${coins.length} deployment(s) and the simulator files`);
