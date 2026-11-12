// The ward: the patient, its drip and its lives, and the strip coming off
// the recorder. The page and the lab both draw this.
import type { Deployment, Monitor } from '../chain/types';
import { statusName } from '../chain/types';
import type { Reading } from '../lib/trace';
import { Headstone } from './Headstone';
import { MonitorScreen } from './Monitor';
import { CrashCart, Lives, Patient } from './Patient';

const CONFETTI = Array.from({ length: 30 }, (_, i) => ({ left: (i * 37) % 100, delay: (i % 7) * 60, turn: (i * 73) % 360, hue: i % 3 }));

interface Props {
  monitor: Monitor;
  reading: Reading;
  blockMs: number;
  at: number;
  deployment: Deployment;
  rpc: string;
  /** A save just happened: the patient does its take and paper falls. */
  party?: boolean;
  /** The lens closing on a death, or opening on a revival. */
  iris?: 'in' | 'out' | null;
}

export function Ward({ monitor, reading, blockMs, at, deployment, rpc, party = false, iris = null }: Props) {
  const status = statusName(monitor.status);
  return (
    <section className={`room is-${status}`} aria-label="The ward">
      {status === 'deceased' ? (
        <Headstone monitor={monitor} deployment={deployment} rpc={rpc} blockMs={blockMs} />
      ) : (
        <>
          <div className="room-side">
            <CrashCart monitor={monitor} />
            <Lives monitor={monitor} />
          </div>
          <Patient monitor={monitor} reading={reading} taking={party} />
          <MonitorScreen monitor={monitor} reading={reading} blockMs={blockMs} at={at} />
        </>
      )}
      {party && (
        <div className="confetti" aria-hidden="true">
          {CONFETTI.map((c, i) => (
            <i key={i} className={`hue-${c.hue}`} style={{ left: `${c.left}%`, animationDelay: `${c.delay}ms`, rotate: `${c.turn}deg` }} />
          ))}
        </div>
      )}
      {iris && <div className={`iris is-${iris}`} aria-hidden="true" />}
    </section>
  );
}
