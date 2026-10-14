import { defineConfig, devices } from '@playwright/test';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

// Every run starts with empty disks and no rules: the backend backs up between these folders.
export const storage = join(tmpdir(), 'nas-backup-manager-e2e-storage');
const database = join(tmpdir(), 'nas-backup-manager-e2e.sqlite3');

export default defineConfig({
  testDir: 'src/tests/e2e',
  fullyParallel: true,
  reporter: 'list',
  use: { ...devices['Desktop Chrome'], baseURL: 'http://127.0.0.1:1420', trace: 'on-first-retry' },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: [
    {
      command: 'npm run dev -- --host 127.0.0.1 --port 1420 --strictPort',
      // Keep release tests independent of the host's native file watcher limit.
      env: { CHOKIDAR_USEPOLLING: 'true' },
      url: 'http://127.0.0.1:1420',
      reuseExistingServer: !process.env.CI,
      timeout: 120_000,
    },
    {
      command: `rm -rf ${storage} ${database}* && mkdir -p ${storage} && npm run build && npm run backend:dev`,
      env: { NAS_STORAGE_ROOT: storage, NAS_DB_PATH: database },
      url: 'http://127.0.0.1:1420/api/rules',
      reuseExistingServer: false,
      timeout: 180_000,
    },
  ],
});
