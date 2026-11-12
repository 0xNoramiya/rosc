// Who is here and whether the page can press for them.
import { useState } from 'react';
import type { Rescuer } from '../chain/rescuer';
import { rosc, short, toUnits } from '../lib/format';

export interface Chart {
  registered: boolean;
  balance: string;
  claimable: string;
  storageFee: string;
}

interface Props {
  rescuer: Rescuer;
  chart: Chart;
  minBalance: string;
  alive: boolean;
}

export function Bedside({ rescuer, chart, minBalance, alive }: Props) {
  const [to, setTo] = useState('');
  const [amount, setAmount] = useState('10');
  const [localId, setLocalId] = useState('');
  const [localKey, setLocalKey] = useState('');
  const eligible = BigInt(chart.balance) >= BigInt(minBalance);

  return (
    <section className="bedside" aria-label="Scrub in">
      <h2>Scrub in</h2>
      <ol className="steps">
        <li className={rescuer.accountId ? 'is-done' : ''}>
          <span className="step-text">{rescuer.accountId ? `At the bedside as ${short(rescuer.accountId)}.` : 'Connect a wallet.'}</span>
          {rescuer.walletAccountId ? (
            <button type="button" onClick={rescuer.disconnect}>
              Sign out
            </button>
          ) : (
            !rescuer.accountId && (
              <button type="button" className="primary" onClick={rescuer.connect}>
                Connect wallet
              </button>
            )
          )}
        </li>
        <li className={chart.registered && eligible ? 'is-done' : ''}>
          <span className="step-text">
            {chart.registered ? `Holding ${rosc(chart.balance)} ROSC.` : 'Register with the coin and take one faucet claim.'}
            {chart.registered && !eligible ? ` Rescuers hold at least ${rosc(minBalance)}.` : ''}
          </span>
          {rescuer.walletAccountId && !chart.registered && (
            <button type="button" className="primary" onClick={() => rescuer.admit(chart.storageFee)}>
              Register and claim 1,000,000 ROSC
            </button>
          )}
        </li>
        <li className={rescuer.keyStatus === 'ready' ? 'is-done' : ''}>
          <span className="step-text">
            {rescuer.keyStatus === 'ready'
              ? 'The page holds a key for the rescue calls. Presses need no wallet prompt.'
              : rescuer.keyStatus === 'missing'
                ? `The key in this browser is not a key of ${short(rescuer.accountId)}. Check the account name, or add a new key.`
                : 'Approve one transaction that lets this page send rescue calls for you. It cannot move tokens.'}
          </span>
          {rescuer.walletAccountId && rescuer.keyStatus !== 'ready' && (
            <button type="button" className="primary" onClick={rescuer.scrubIn}>
              Add the page key
            </button>
          )}
          {rescuer.keyStatus === 'ready' && (
            <button type="button" onClick={rescuer.forgetKey}>
              Forget the key
            </button>
          )}
        </li>
      </ol>
      {rescuer.busy && (
        <p className="note" role="status">
          {rescuer.busy}…
        </p>
      )}
      {rescuer.error && (
        <p className="note is-alarm" role="alert">
          {rescuer.error}
        </p>
      )}

      {rescuer.accountId && alive && (
        <form
          className="beat-form"
          onSubmit={event => {
            event.preventDefault();
            if (rescuer.sender && !rescuer.walletAccountId) rescuer.sender.send('ft_transfer', { receiver_id: to.trim(), amount: toUnits(amount) }, 1n);
            else rescuer.transfer(to.trim(), toUnits(amount));
          }}
        >
          <h3>Send a beat</h3>
          <p className="note">A transfer between two accounts is a heartbeat. You pay 0.5% oxygen burn and 0.5% to the crash cart on top.</p>
          <label htmlFor="beat-to">To</label>
          <input id="beat-to" value={to} onChange={e => setTo(e.target.value)} placeholder="someone.testnet" autoComplete="off" spellCheck={false} required />
          <label htmlFor="beat-amount">ROSC</label>
          <input id="beat-amount" value={amount} onChange={e => setAmount(e.target.value)} inputMode="decimal" pattern="[0-9]*\.?[0-9]*" required />
          <button type="submit" className="primary">
            Transfer
          </button>
        </form>
      )}

      <details className="local-key">
        <summary>Use a key kept on this device instead</summary>
        <p className="note">For bots and testing. The secret stays in this browser's storage. Any key on the account works: a function-call key can only press.</p>
        <form
          onSubmit={event => {
            event.preventDefault();
            rescuer.useLocalKey(localId, localKey);
            setLocalKey('');
            // Leave the form, so the next key press is a compression and not a character.
            (document.activeElement as HTMLElement | null)?.blur();
          }}
        >
          <label htmlFor="local-id">Account</label>
          <input id="local-id" value={localId} onChange={e => setLocalId(e.target.value)} autoComplete="off" spellCheck={false} required />
          <label htmlFor="local-key">Secret key</label>
          <input id="local-key" type="password" value={localKey} onChange={e => setLocalKey(e.target.value)} placeholder="ed25519:…" autoComplete="off" required />
          <button type="submit">Use this key</button>
        </form>
      </details>
    </section>
  );
}

/// The real numbers, kept apart from the jokes.
export function CprCard() {
  return (
    <aside className="cpr-card" aria-label="Real CPR">
      <h2>Real CPR, for a real person</h2>
      <dl>
        <div>
          <dt>Rate</dt>
          <dd>100 to 120 compressions a minute</dd>
        </div>
        <div>
          <dt>Ratio</dt>
          <dd>30 compressions to 2 breaths</dd>
        </div>
        <div>
          <dt>Pauses</dt>
          <dd>never more than 10 seconds</dd>
        </div>
        <div>
          <dt>Swap</dt>
          <dd>change compressor every 2 minutes, sooner if tired</dd>
        </div>
      </dl>
      <p>
        From the American Heart Association's 2025 guidelines. This page is a toy and not training.{' '}
        <a href="https://cpr.heart.org/" target="_blank" rel="noreferrer">
          Find a CPR course
        </a>
        .
      </p>
    </aside>
  );
}
