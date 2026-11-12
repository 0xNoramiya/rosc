// After final death the monitor becomes a headstone.
import { useEffect, useState } from 'react';
import { provider } from '../chain/rpc';
import type { Deployment, Monitor } from '../chain/types';
import { rosc, span } from '../lib/format';
import { useLook } from '../look/LookProvider';
import art from '../../public/art/manifest.json';

export function Headstone({ monitor, deployment, rpc, blockMs }: { monitor: Monitor; deployment: Deployment; rpc: string; blockMs: number }) {
  const block = (monitor.status as { Deceased: { block: number } }).Deceased.block;
  const [date, setDate] = useState<string | null>(null);
  // The clay stone is a photograph; the drawn one stands in if it is missing.
  const { look } = useLook();
  const [broken, setBroken] = useState(false);
  const clay = look.patient === 'clay' && !broken;

  useEffect(() => {
    provider(rpc)
      .viewBlock({ blockId: block })
      .then(
        (b: any) => setDate(new Date(Number(BigInt(b.header.timestamp_nanosec ?? b.header.timestamp) / 1_000_000n)).toUTCString()),
        () => setDate(null),
      );
  }, [rpc, block]);

  const born = deployment.deployed_at_block;
  const burned = BigInt(monitor.config.total_supply) - BigInt(monitor.supply);
  return (
    <article className={`headstone${clay ? ' is-clay' : ''}`} aria-label="Headstone">
      {clay && (
        <div className="clay-stone">
          <img src={`${import.meta.env.BASE_URL}art/${art.sprites.headstone.file}`} width={art.sprites.headstone.size[0]} height={art.sprites.headstone.size[1]} alt="" onError={() => setBroken(true)} />
          <p className="clay-stone-face">
            <span className="stone-name">Code Blue</span>
            <span className="stone-ticker">$ROSC</span>
          </p>
        </div>
      )}
      <div className="stone">
        {!clay && (
          <>
            <p className="stone-name">Code Blue</p>
            <p className="stone-ticker">$ROSC</p>
          </>
        )}
        <p className="stone-line">Time of death: block {block.toLocaleString('en-US')}</p>
        {date && <p className="stone-line">{date}</p>}
        <dl>
          {born !== undefined && (
            <div>
              <dt>Lived</dt>
              <dd>
                {(block - born).toLocaleString('en-US')} blocks, about {span(block - born, blockMs)}
              </dd>
            </div>
          )}
          <div>
            <dt>Arrests survived</dt>
            <dd>{monitor.totals.saves}</dd>
          </div>
          <div>
            <dt>Arrests in all</dt>
            <dd>{monitor.totals.arrests}</dd>
          </div>
          <div>
            <dt>Burned</dt>
            <dd>{rosc(burned)} ROSC</dd>
          </div>
        </dl>
        <p className="stone-foot">No circulation, no circulation.</p>
      </div>
      {!clay && (
        <svg className="flower" viewBox="0 0 60 90" aria-hidden="true">
          <path className="f-stem" d="M30 88C30 60 28 50 30 34M30 66c-10-2-14-10-14-14 8 0 13 6 14 14z" />
          {[0, 72, 144, 216, 288].map(turn => (
            <ellipse key={turn} className="f-petal" cx="30" cy="14" rx="8" ry="12" transform={`rotate(${turn} 30 26)`} />
          ))}
          <circle className="f-heart" cx="30" cy="26" r="7" />
        </svg>
      )}
      <p className="note">
        "Time of death" and "deceased" here describe a token on a test network. Its transfers are frozen for good, as its rules said they would be.
      </p>
    </article>
  );
}
