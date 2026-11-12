// The chart: every burn and payment the ledger ring still holds, and a sheet
// for each code.
import { useEffect, useState } from 'react';
import { view } from '../chain/rpc';
import type { CodeSheet, LedgerLine } from '../chain/types';
import { rosc, short } from '../lib/format';
import { Sheet } from './Code';

const KIND: Record<string, string> = {
  Oxygen: 'Oxygen burn',
  NoFlow: 'No-flow burn',
  Epinephrine: 'Epinephrine',
  Death: 'Death burn',
  Estate: 'Estate burned',
  Faucet: 'Faucet claim',
  Bounty: 'Bounty claimed',
};

function describe(kind: LedgerLine['line']['kind']): { label: string; burn: boolean } {
  if (kind === 'Bounty') return { label: 'Bounty credited', burn: false };
  if ('Burn' in kind) return { label: KIND[kind.Burn] ?? kind.Burn, burn: true };
  return { label: KIND[kind.Paid] ?? kind.Paid, burn: false };
}

interface Props {
  rpc: string;
  contract: string;
  sheets: CodeSheet[];
  /** The last code has ended but the chain has not written its sheet yet. */
  sheetDue?: boolean;
  blockMs: number;
  block: number;
}

export function ChartScreen({ rpc, contract, sheets, sheetDue = false, blockMs, block }: Props) {
  const [lines, setLines] = useState<LedgerLine[]>([]);
  const [filed, setFiled] = useState(0);
  const [error, setError] = useState<string | null>(null);

  // The ring holds 256 lines; two pages of 128 read all of it.
  const refresh = Math.floor(block / 20);
  useEffect(() => {
    let dropped = false;
    (async () => {
      try {
        const first = await view<{ filed: number; next: number; entries: LedgerLine[] }>(rpc, contract, 'get_ledger', { limit: 128 });
        const second = first.next < first.filed ? await view<{ entries: LedgerLine[] }>(rpc, contract, 'get_ledger', { from: first.next, limit: 128 }) : { entries: [] };
        if (dropped) return;
        setLines([...first.entries, ...second.entries].reverse());
        setFiled(first.filed);
        setError(null);
      } catch (e) {
        if (!dropped) setError(e instanceof Error ? e.message : String(e));
      }
    })();
    return () => {
      dropped = true;
    };
  }, [rpc, contract, refresh]);

  return (
    <div className="chart">
      <section>
        <h2>Ledger</h2>
        <p className="note">
          Every burn and payment, newest first. The coin was minted once, at launch; nothing has been minted since. Oxygen burns are added up per monitor bucket.
          {filed > lines.length ? ` Showing the newest ${lines.length} of ${filed} lines; the rest are in the contract's event log.` : ''}
        </p>
        {error && <p className="note is-alarm">The ledger could not be read: {error}</p>}
        <div className="scroll">
          <table>
            <thead>
              <tr>
                <th>Block</th>
                <th>What</th>
                <th>Who</th>
                <th className="num">ROSC</th>
              </tr>
            </thead>
            <tbody>
              {lines.map(({ seq, line }) => {
                const { label, burn } = describe(line.kind);
                return (
                  <tr key={seq}>
                    <td>{line.block}</td>
                    <td>
                      {label}
                      {line.count > 1 ? ` (${line.count} transfers)` : ''}
                    </td>
                    <td>{short(line.account) || (burn ? 'burned' : '')}</td>
                    <td className="num">{rosc(line.amount)}</td>
                  </tr>
                );
              })}
              {lines.length === 0 && (
                <tr>
                  <td colSpan={4}>Nothing filed yet. The first transfer will burn a little oxygen.</td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </section>
      <section>
        <h2>Code sheets</h2>
        {sheetDue && <p className="note">The last code is over. The chain writes its sheet, and its lines in the ledger, with the next transaction on the coin.</p>}
        {sheets.length === 0 && !sheetDue && <p className="note">No code has ended yet.</p>}
        {[...sheets].reverse().map(sheet => (
          <Sheet key={sheet.code_no} sheet={sheet} blockMs={blockMs} />
        ))}
      </section>
    </div>
  );
}
