import { defineConfig, devices } from '@playwright/test';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

export const storage = join(tmpdir(), 'nas-backup-manager-e2e-storage');
const database = join(tmpdir(), 'nas-backup-manager-e2e.sqlite3');
const socket = join(tmpdir(), 'nas-backup-manager-e2e.sock');
const port = 14217;
const baseURL = `http://127.0.0.1:${port}`;

export default defineConfig({
  testDir: 'src/tests/e2e',
  fullyParallel: true,
  reporter: 'list',
  use: { ...devices['Desktop Chrome'], baseURL, trace: 'on-first-retry' },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: [
    {
      command: `npm run dev -- --host 127.0.0.1 --port ${port} --strictPort`,
      env: { CHOKIDAR_USEPOLLING: 'true', NAS_APP_SOCKET: socket },
      url: baseURL,
      reuseExistingServer: false,
      timeout: 120_000,
    },
    {
      command: `rm -rf ${storage} ${database}* && mkdir -p ${storage} && npm run build && npm run backend:dev`,
      env: { NAS_STORAGE_ROOT: storage, NAS_DB_PATH: database, NAS_APP_SOCKET: socket },
      url: `${baseURL}/api/rules`,
      reuseExistingServer: false,
      timeout: 180_000,
    },
  ],
});
