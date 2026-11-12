import { NearProvider } from 'near-connect-hooks';
import { Component, StrictMode } from 'react';
import type { ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App.tsx';
import { LookProvider } from './look/LookProvider';
import look from './look/look.json';
import './index.css';

/// Whatever goes wrong, the page says so instead of going blank.
class Failsafe extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  render() {
    if (!this.state.error) return this.props.children;
    const reset = () => {
      try {
        for (const key of Object.keys(localStorage)) if (key.startsWith('codeblue:')) localStorage.removeItem(key);
      } catch {
        /* nothing stored */
      }
      location.reload();
    };
    return (
      <div className="ward">
        <div className="bedside" role="alert">
          <h2>The page hit an error</h2>
          <p>{this.state.error.message}</p>
          <p className="note">Nothing on chain is affected. Reloading clears the key saved in this browser, so you will need to add it again.</p>
          <button type="button" className="primary" onClick={reset}>
            Forget the saved key and reload
          </button>
        </div>
      </div>
    );
  }
}

// The look lab rewrites look.json when it saves. An open page reloads to
// print with the new look; without this the dev server reloads the lab too.
if (import.meta.hot) import.meta.hot.accept('./look/look.json', () => location.reload());

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Failsafe>
      <LookProvider look={look as never}>
        <NearProvider config={{ network: 'testnet' }}>
          <App />
        </NearProvider>
      </LookProvider>
    </Failsafe>
  </StrictMode>,
);
