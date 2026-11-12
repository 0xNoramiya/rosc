// Puts a look on the page: its CSS variables on the root, and the values the
// canvas and the patient need in context.
import { createContext, useContext, useEffect, useMemo } from 'react';
import type { ReactNode } from 'react';
import type { Look, Style } from './look';
import { normalise, styleOf } from './look';

const Context = createContext<{ look: Look; style: Style } | null>(null);

export function LookProvider({ look, children }: { look: Partial<Look>; children: ReactNode }) {
  const value = useMemo(() => {
    const whole = normalise(look);
    return { look: whole, style: styleOf(whole) };
  }, [look]);

  useEffect(() => {
    const root = document.documentElement;
    for (const [name, text] of Object.entries(value.style.vars)) root.style.setProperty(name, text);
  }, [value]);

  return (
    <Context.Provider value={value}>
      {/* The boil: a slight wobble of whatever has been handled, re-seeded on every pose. */}
      <svg width="0" height="0" style={{ position: 'absolute' }} aria-hidden="true">
        <filter id="boil" x="-5%" y="-5%" width="110%" height="110%">
          <feTurbulence id="boil-noise" type="fractalNoise" baseFrequency="0.018" numOctaves="1" seed="1" result="n" />
          <feDisplacementMap in="SourceGraphic" in2="n" scale={value.look.boil * 4} xChannelSelector="R" yChannelSelector="G" />
        </filter>
      </svg>
      <Boil step={value.style.step * 2} on={value.look.boil > 0} />
      {children}
    </Context.Provider>
  );
}

/// Re-seeds the boil on twos. Nothing boils when motion is reduced.
function Boil({ step, on }: { step: number; on: boolean }) {
  useEffect(() => {
    if (!on || matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    let seed = 1;
    const timer = setInterval(() => {
      seed = (seed % 4) + 1;
      document.getElementById('boil-noise')?.setAttribute('seed', String(seed));
    }, step);
    return () => clearInterval(timer);
  }, [step, on]);
  return null;
}

export function useLook() {
  const value = useContext(Context);
  if (!value) throw new Error('useLook needs a LookProvider');
  return value;
}
