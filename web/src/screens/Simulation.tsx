// The simulator's files, drawn with the simulator's own chart code.
import { useEffect, useState } from 'react';
// @ts-expect-error: a plain module shared with sim/render-charts.mjs
import { CSS, lifePanel, pairChart, parseCsv, survivalPanel, sweepChart } from '../../../sim/charts.mjs';

const SCENARIOS: [string, string, string][] = [
  ['healthy', 'Healthy', 'Steady holders. Does the coin go 60 days with no arrest?'],
  ['fading', 'Fading', 'Attention halves every 10 days. When does it die?'],
  ['abandoned', 'Abandoned', 'Everyone leaves on day 20. How long until final death?'],
  ['paced', 'Paced', 'One bot, no community. It never arrests.'],
  ['farmed', 'Farmed', 'One serial rescuer on a dead coin.'],
  ['night-shift', 'Night shift', 'Strong day and night swing, slow rescuers at night.'],
];

type Rows = Record<string, string | number>[];

async function csv(path: string): Promise<Rows> {
  const response = await fetch(`${import.meta.env.BASE_URL}sim/${path}`);
  if (!response.ok) throw new Error(`${path}: ${response.status}`);
  return parseCsv(await response.text());
}

const Svg = ({ svg, caption }: { svg: string; caption?: string }) => (
  <figure>
    {/* Built here from the simulator's own numbers; no outside text goes in. */}
    <div dangerouslySetInnerHTML={{ __html: svg }} />
    {caption && <figcaption>{caption}</figcaption>}
  </figure>
);

export function SimulationScreen() {
  const [data, setData] = useState<{ scenarios: Record<string, Rows>; goals: Rows; pair: Rows; sweep: Rows } | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const scenarios: Record<string, Rows> = {};
        for (const [name] of SCENARIOS) scenarios[name] = await csv(`${name}.csv`);
        setData({ scenarios, goals: await csv('goals.csv'), pair: await csv('pair.csv'), sweep: await csv('sweep.csv') });
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    })();
  }, []);

  if (error) return <p className="note is-alarm">The simulator's files are missing ({error}). Run make sim, then reload.</p>;
  if (!data) return <p className="note">Reading the simulator's files…</p>;

  return (
    <div className="sim">
      <style>{CSS}</style>
      <section>
        <h2>Sixty simulated days, a thousand times each</h2>
        <p className="note">
          The simulator runs the same rulebook as the contract under made-up people: holders who lose interest, rescuers who are sometimes late. The live preset was chosen from
          these files.
        </p>
        <div className="scroll">
          <table>
            <thead>
              <tr>
                <th>Design goal</th>
                <th>Measured</th>
                <th>Verdict</th>
              </tr>
            </thead>
            <tbody>
              {data.goals.map(goal => (
                <tr key={String(goal.goal)}>
                  <td>{goal.goal}</td>
                  <td>{goal.measured}</td>
                  <td>{goal.verdict === 'pass' ? '✓ Pass' : '✕ Fail'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
      <section>
        <h2>Survival</h2>
        <p className="note">Share of runs in which the coin is not yet dead for good, by day.</p>
        <div className="sim-grid">
          {SCENARIOS.map(([name, title, question]) => (
            <Svg key={name} svg={survivalPanel(title, '', [{ label: '', rows: data.scenarios[name] }]).svg} caption={question} />
          ))}
        </div>
      </section>
      <section>
        <h2>Supply and crash cart</h2>
        <p className="note">Median of the runs, with the 10% to 90% band. Supply only moves much when the coin dies.</p>
        <div className="sim-grid">
          {SCENARIOS.map(([name, title]) => (
            <Svg key={name} svg={lifePanel(title, '', data.scenarios[name]).svg} />
          ))}
        </div>
      </section>
      <section className="sim-wide">
        <h2>Why 12 hours</h2>
        <div className="scroll">
          <Svg svg={sweepChart(data.sweep).svg} />
        </div>
        <div className="scroll">
          <Svg svg={pairChart(data.pair).svg} />
        </div>
      </section>
    </div>
  );
}
