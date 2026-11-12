// The trace, burned onto chart paper. It paints the marks from `lib/trace.ts`
// and nothing else, in the one colour a thermal head can make, on a grid
// whose squares mean something.
import { useEffect, useRef } from 'react';
import type { Mark, Reading } from '../lib/trace';
import { seeded } from '../lib/trace';
import type { Style } from '../look/look';
import { useLook } from '../look/LookProvider';

interface Props {
  reading: Reading;
  /** The newest block the chain reported, and when that report arrived. */
  block: number;
  at: number;
  blockMs: number;
  /** The fewest blocks the strip must show. */
  sweep: number;
}

const NICE = [0.5, 1, 2, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000, 10000];
/// Small squares kept clear at the left for the calibration pulse.
const MARGIN = 9;

/// Blocks per small square: the smallest round number that fits the sweep.
export function scaleOf(width: number, mm: number, sweep: number): number {
  const squares = Math.max(10, width / mm - MARGIN);
  return NICE.find(n => n * squares >= sweep) ?? NICE[NICE.length - 1];
}

/// One heartbeat, as a height between -1 and 1 at `t` complex-widths from its centre.
function complex(t: number): number {
  const bump = (centre: number, width: number, height: number) => height * Math.exp(-(((t - centre) / width) ** 2));
  return bump(-0.55, 0.16, 0.12) + bump(-0.1, 0.035, -0.14) + bump(0, 0.045, 1) + bump(0.11, 0.04, -0.24) + bump(0.62, 0.2, 0.2);
}

function paint(canvas: HTMLCanvasElement, props: Props, strip: Style['strip'], now: number) {
  const ratio = window.devicePixelRatio || 1;
  const [w, h] = [canvas.clientWidth, canvas.clientHeight];
  if (canvas.width !== Math.round(w * ratio) || canvas.height !== Math.round(h * ratio)) {
    canvas.width = Math.round(w * ratio);
    canvas.height = Math.round(h * ratio);
  }
  const ctx = canvas.getContext('2d')!;
  ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
  const mm = strip.mm;
  const perSquare = scaleOf(w, mm, props.sweep);
  const perBlock = mm / perSquare;
  const left = MARGIN * mm;
  const shown = (w - left) / perBlock;
  const from = now - shown;
  const x = (block: number) => left + (block - from) * perBlock;
  // The baseline sits on a heavy line, and a beat stands ten small squares tall.
  const base = Math.round((h * 0.66) / (mm * 5)) * mm * 5;
  const tall = Math.min(10 * mm, base - mm * 2);

  // The paper: fine lines every square, a heavier one every fifth. The grid
  // is printed on the paper, so it feeds past with the trace.
  ctx.fillStyle = strip.paper;
  ctx.fillRect(0, 0, w, h);
  const feed = (now * perBlock) % (mm * 5);
  for (let i = 0, gx = w - feed; gx > -mm; gx -= mm, i++) {
    ctx.fillStyle = i % 5 === 0 ? strip.heavy : strip.fine;
    ctx.fillRect(Math.round(gx), 0, 1, h);
  }
  for (let i = 0, gy = base; gy > 0; gy -= mm, i++) {
    ctx.fillStyle = i % 5 === 0 ? strip.heavy : strip.fine;
    ctx.fillRect(0, gy, w, 1);
  }
  for (let i = 1, gy = base + mm; gy < h; gy += mm, i++) {
    ctx.fillStyle = i % 5 === 0 ? strip.heavy : strip.fine;
    ctx.fillRect(0, gy, w, 1);
  }

  const { marks } = props.reading;
  // A complex is about two blocks wide, and never thinner than two squares.
  const width = Math.max(1.1, (mm * 2) / perBlock);
  const beats = marks.filter((m): m is Extract<Mark, { kind: 'beat' }> => m.kind === 'beat' && m.block > from - width);
  const quiet = marks.filter((m): m is Extract<Mark, { kind: 'electrical' }> => m.kind === 'electrical' && m.block > from - width);
  const presses = marks.filter((m): m is Extract<Mark, { kind: 'compression' }> => m.kind === 'compression' && m.block > from - 2);
  const flat = marks.find((m): m is Extract<Mark, { kind: 'flatline' }> => m.kind === 'flatline');
  const phase = flat ? seeded(flat.from) : null;
  const waves = phase ? [0, 1, 2, 3].map(() => ({ period: 2.2 + phase() * 5, shift: phase() * 6.28, size: 0.5 + phase() * 0.5 })) : [];

  ctx.strokeStyle = strip.burn;
  ctx.fillStyle = strip.burn;
  ctx.lineWidth = strip.trace;
  ctx.lineJoin = 'round';
  ctx.lineCap = 'round';
  ctx.beginPath();
  for (let px = left; px <= w; px++) {
    const block = from + (px - left) / perBlock;
    let y = 0;
    for (const beat of beats) {
      const t = (block - beat.block) / width;
      if (t > -1.4 && t < 1.4) y += complex(t);
    }
    for (const mark of quiet) {
      const t = (block - mark.block) / width;
      if (t > -1.4 && t < 1.4) y += 0.22 * complex(t);
    }
    if (flat && block >= flat.from) {
      if (flat.rhythm === 'Vf') {
        // Fibrillation: a chaotic wave, the same for everyone watching this arrest.
        for (const wave of waves) y += 0.16 * wave.size * Math.sin(((block - flat.from) / wave.period) * 6.28 * (shown / 150) + wave.shift);
      } else {
        // A flat line with a slight wander.
        y += 0.018 * Math.sin((block - flat.from) / 9 + (waves[0]?.shift ?? 0));
      }
      for (const press of presses) {
        const t = (block - press.block) / Math.max(0.9, mm / perBlock);
        if (t > -0.5 && t < 0.5) y += 0.42 * Math.cos(t * Math.PI) ** 2;
      }
    }
    const py = base - y * tall;
    if (px === left) ctx.moveTo(px, py);
    else ctx.lineTo(px, py);
  }
  ctx.stroke();

  // The calibration pulse: five squares wide, ten tall. It says how tall one beat is.
  ctx.fillStyle = strip.paper;
  ctx.fillRect(0, 0, left - mm, h);
  for (let gy = base % mm; gy < h; gy += mm) {
    ctx.fillStyle = Math.round((base - gy) / mm) % 5 === 0 ? strip.heavy : strip.fine;
    ctx.fillRect(0, gy, left - mm, 1);
  }
  ctx.strokeStyle = strip.burn;
  ctx.beginPath();
  ctx.moveTo(mm, base);
  ctx.lineTo(mm * 2, base);
  ctx.lineTo(mm * 2, base - tall);
  ctx.lineTo(mm * 7, base - tall);
  ctx.lineTo(mm * 7, base);
  ctx.lineTo(mm * 8, base);
  ctx.stroke();

  // What the eye cannot tell from the line alone, printed by the same head.
  ctx.fillStyle = strip.burn;
  ctx.font = `600 12px ${strip.font}`;
  ctx.textAlign = 'left';
  ctx.fillText(`1 square = ${perSquare} ${perSquare === 1 ? 'block' : 'blocks'}`, mm, h - 6);
  ctx.textAlign = 'center';
  ctx.lineWidth = 1;
  for (const beat of beats) {
    const bx = x(beat.block);
    if (bx < left + 4) continue;
    if (beat.paced) {
      ctx.beginPath();
      ctx.moveTo(bx - width * 0.32 * perBlock, base - tall * 0.55);
      ctx.lineTo(bx - width * 0.32 * perBlock, base + tall * 0.2);
      ctx.stroke();
    }
    if (beat.bot) ctx.fillText('bot', bx, Math.min(h - 20, base + mm * 4));
  }
  if (quiet.length) {
    ctx.textAlign = 'left';
    ctx.fillText('NO PULSE', Math.max(left + 4, x(quiet[0].block)), base - tall * 0.4);
  }
}

export function Ecg(props: Props) {
  const ref = useRef<HTMLCanvasElement>(null);
  const { style } = useLook();
  const latest = useRef({ props, style });
  latest.current = { props, style };

  useEffect(() => {
    const canvas = ref.current!;
    const still = matchMedia('(prefers-reduced-motion: reduce)').matches;
    let frame = 0;
    let shown = -1;
    const tick = () => {
      const { props: p, style: s } = latest.current;
      // The paper feeds in steps, an eighth of a block at a time, as a pose would.
      const ahead = still ? 0 : Math.min((performance.now() - p.at) / p.blockMs, 2);
      const now = Math.floor((p.block + ahead) * 8) / 8;
      const key = now + canvas.clientWidth * 1e-6 + p.reading.marks.length * 1e-9;
      if (key !== shown) {
        shown = key;
        paint(canvas, p, s.strip, now);
      }
      frame = requestAnimationFrame(tick);
    };
    tick();
    return () => cancelAnimationFrame(frame);
  }, []);

  // A new look repaints at once, moving or not.
  useEffect(() => {
    if (ref.current) paint(ref.current, props, style.strip, Math.floor(props.block * 8) / 8);
  }, [style]);

  const { marks, rhythm } = props.reading;
  const beats = marks.filter(m => m.kind === 'beat').length;
  return (
    <canvas
      ref={ref}
      className="ecg"
      role="img"
      aria-label={`ECG strip. Rhythm: ${rhythm}. ${beats} beats in the ring, ${marks.filter(m => m.kind === 'compression').length} compressions on the paper.`}
    />
  );
}
