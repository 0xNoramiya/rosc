import react from '@vitejs/plugin-react';
import { mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import type { Plugin } from 'vite';
import { defineConfig } from 'vite';

const at = (path: string) => fileURLToPath(new URL(path, import.meta.url));

/// Dev only. The lab's Save button writes the look the page reads, and keeps
/// every save beside it so an earlier one can be brought back.
function lab(): Plugin {
  return {
    name: 'codeblue-lab',
    apply: 'serve',
    configureServer(server) {
      server.middlewares.use('/__lab/saved', (_req, res) => {
        mkdirSync(at('./src/look/saved'), { recursive: true });
        const files = readdirSync(at('./src/look/saved')).filter(f => f.endsWith('.json')).sort();
        res.setHeader('content-type', 'application/json');
        res.end(JSON.stringify(files.map(f => ({ name: f, look: JSON.parse(readFileSync(at(`./src/look/saved/${f}`), 'utf8')) }))));
      });
      server.middlewares.use('/__lab/save', (req, res) => {
        let body = '';
        req.on('data', chunk => (body += chunk));
        req.on('end', () => {
          try {
            const text = `${JSON.stringify(JSON.parse(body), null, 2)}\n`;
            mkdirSync(at('./src/look/saved'), { recursive: true });
            const name = `${new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19)}.json`;
            writeFileSync(at(`./src/look/saved/${name}`), text);
            writeFileSync(at('./src/look/look.json'), text);
            res.setHeader('content-type', 'application/json');
            res.end(JSON.stringify({ path: 'web/src/look/look.json', kept: `web/src/look/saved/${name}` }));
          } catch (error) {
            res.statusCode = 400;
            res.end(String(error));
          }
        });
      });
    },
  };
}

export default defineConfig({
  plugins: [react(), lab()],
  resolve: { alias: { util: at('./src/lib/util-shim.ts') } },
  // The Simulation screen draws with sim/charts.mjs, one directory up.
  server: { fs: { allow: ['..'] } },
});
