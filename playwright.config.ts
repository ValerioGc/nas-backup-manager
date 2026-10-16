import { defineConfig, devices } from '@playwright/test';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

// Every run starts with empty disks and no rules: the backend backs up between these folders.
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
      // Keep release tests independent of the host's native file watcher limit.
      env: { CHOKIDAR_USEPOLLING: 'true', NAS_APP_SOCKET: socket },
      url: baseURL,
      // Other NAS apps share the development port: always start this checkout's own server.
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
