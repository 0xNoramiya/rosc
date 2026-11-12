import { defineConfig } from '@playwright/test';

// The page's acceptance tests run a real browser against real testnet coins.
// They need the deployments in ../deployments (make deploy-demo, make demo).
export default defineConfig({
  testDir: 'tests',
  timeout: 15 * 60_000,
  workers: 1,
  reporter: 'list',
  use: { baseURL: 'http://localhost:4173', viewport: { width: 1280, height: 900 } },
  webServer: { command: 'npm run build && npx vite preview --port 4173 --strictPort', url: 'http://localhost:4173', reuseExistingServer: true, timeout: 180_000 },
});
