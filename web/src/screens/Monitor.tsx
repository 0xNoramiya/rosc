import type { Monitor } from '../chain/types';
import { statusName } from '../chain/types';
import { span } from '../lib/format';
import type { Reading } from '../lib/trace';
import { Ecg } from './Ecg';

interface Props {
  monitor: Monitor;
  reading: Reading;
  blockMs: number;
  at: number;
}

/// Blocks across the trace: one and a half arrest windows, so a healthy coin
/// always shows beats and a dying one shows the silence growing.
export const sweepOf = (monitor: Monitor) => Math.round(monitor.config.arrest_blocks * 1.5);

function heartRate(monitor: Monitor, reading: Reading, blockMs: number): { value: string; unit: string } {
  if (statusName(monitor.status) !== 'alive') return { value: '0', unit: 'beats a minute' };
  const minutes = (monitor.config.arrest_blocks * blockMs) / 60000;
  const perMinute = reading.beatsInWindow / minutes;
  if (perMinute >= 1) return { value: perMinute.toFixed(perMinute < 10 ? 1 : 0), unit: 'beats a minute' };
  const perHour = perMinute * 60;
  return perHour >= 1 ? { value: perHour.toFixed(1), unit: 'beats an hour' } : { value: (perHour * 24).toFixed(1), unit: 'beats a day' };
}

export function MonitorScreen({ monitor, reading, blockMs, at }: Props) {
  const status = statusName(monitor.status);
  const rate = heartRate(monitor, reading, blockMs);
  const alarm = status !== 'alive';

  let clock: { label: string; value: string };
  if (typeof monitor.status === 'object' && 'Alive' in monitor.status) {
    clock = { label: 'Arrest in', value: span(monitor.status.Alive.arrest_block - monitor.block, blockMs) };
  } else if (monitor.code) {
    clock = { label: 'No pulse for', value: span(monitor.block - monitor.code.arrest_block, blockMs) };
  } else if (typeof monitor.status === 'object' && 'Morgue' in monitor.status) {
    clock = { label: 'Next code opens in', value: span(monitor.status.Morgue.until_block - monitor.block, blockMs) };
  } else {
    clock = { label: 'Time of death', value: `block ${(monitor.status as { Deceased: { block: number } }).Deceased.block}` };
  }

  return (
    <div className="scope">
      <div className="trace-head">
        <span className="lead-name">Lead II</span>
        <span className={`rhythm ${alarm ? 'is-alarm' : ''}`} role="status">
          {reading.rhythm}
        </span>
        {alarm && <span className="stamp">Code blue</span>}
      </div>
      <Ecg reading={reading} block={monitor.block} at={at} blockMs={blockMs} sweep={sweepOf(monitor)} />
      <div className="vitals" aria-label="Vitals">
        <div className="vital v-hr">
          <span className="v-name">HR</span>
          <span className="v-num">{rate.value}</span>
          <span className="v-unit">{rate.unit}</span>
        </div>
        <div className={`vital v-clock ${alarm ? 'is-alarm' : ''}`}>
          <span className="v-name">{clock.label}</span>
          <span className="v-num">{clock.value}</span>
        </div>
        <div className="vital v-ef">
          <span className="v-name">EF</span>
          <span className="v-num">{monitor.ejection_fraction}%</span>
          <span className="v-unit">
            {monitor.scars} {monitor.scars === 1 ? 'scar' : 'scars'}
          </span>
        </div>
      </div>
      <p className="trace-foot">
        Each spike is a transfer that counted as a beat. Last beat at block {monitor.last_beat_block.toLocaleString('en-US')}.
      </p>
    </div>
  );
}
