import { expect, test } from '@playwright/test';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { storage } from '../../../playwright.config';

/** A scheduled rule between two folders of the test's own, as the wizard saves it. */
function rule(id: string, name: string) {
  return {
    id,
    type: 'scheduled',
    name,
    enabled: true,
    createdAt: new Date().toISOString(),
    folders: { origin: `/${id}/origin`, destination: `/${id}/backup/copy`, cloud: null },
    sameDestination: false,
    syncMode: 'mirror',
    schedule: { frequency: 'daily', time: '03:00', weekday: null, day: null, month: null },
    filters: [
      {
        id: 1,
        action: 'exclude',
        fileType: '*',
        status: 'name',
        operator: null,
        value: '*.tmp',
      },
    ],
    operations: ['batchRename'],
    operationConfigurations: {
      batchRename: { filename: 'x.txt', separator: '_', prefix: 'bak', position: 'suffix' },
      archive: null,
      encrypt: null,
    },
    duplicateResolution: 'overwrite',
    verifyArchivedFiles: true,
  };
}

function seed(id: string, files: Record<string, string>) {
  for (const [path, content] of Object.entries(files)) {
    const file = join(storage, id, 'origin', path);
    mkdirSync(join(file, '..'), { recursive: true });
    writeFileSync(file, content);
  }
}

test('opens the nas-backup-manager interface', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('main')).toBeVisible();
  await expect(page.getByRole('button').first()).toBeVisible();
});

test('runs a rule from its row and shows how it went', async ({ page }) => {
  seed('run', { 'report.pdf': 'report', 'sub/photo.jpg': 'photo', 'draft.tmp': 'draft' });
  const created = await page.request.post('/api/rules', { data: rule('run', 'Documents') });
  expect(created.status()).toBe(201);

  await page.goto('/');
  const row = page.getByRole('row', { name: /Documents/ });
  await expect(row).toBeVisible();
  await expect(row.getByTestId('run_status_never')).toBeVisible();
  await row.getByTestId('rule_run_run').click();
  await expect(row.getByTestId('run_status_ok')).toBeVisible({ timeout: 15_000 });

  const copy = join(storage, 'run', 'backup', 'copy');
  expect(readFileSync(join(copy, 'report_bak.pdf'), 'utf8')).toBe('report');
  expect(readFileSync(join(copy, 'sub', 'photo_bak.jpg'), 'utf8')).toBe('photo');
  expect(existsSync(join(copy, 'draft_bak.tmp'))).toBe(false);

  await row.getByTestId('run_status_ok').hover();
  await expect(page.getByRole('tooltip')).toContainText('Completata');
  await expect(page.getByRole('tooltip')).toContainText('Ultima esecuzione');
  await expect(row.getByRole('cell').nth(1)).toHaveText('Giornaliero · 03:00');
  await expect(row.getByRole('cell').nth(4)).toContainText('/');
  await page.screenshot({ path: '/tmp/nas-backup-rules.png' });
});

test('turns a rule off, edits its name and removes it', async ({ page }) => {
  seed('edit', { 'a.txt': 'a' });
  await page.request.post('/api/rules', { data: rule('edit', 'Old name') });

  await page.goto('/');
  const row = page.getByRole('row', { name: /Old name/ });
  await row.getByTestId('rule_enabled_edit').uncheck();
  await expect(row.getByRole('cell').nth(4)).toHaveText('Disattivata');

  await row.getByTestId('rule_edit_edit').click();
  await expect(page.getByRole('heading', { name: 'Modifica regola' })).toBeVisible();
  await page.getByTestId('rule_name').fill('New name');
  for (let step = 0; step < 4; step += 1) {
    await page.getByRole('button', { name: 'Avanti' }).click();
  }
  await page.getByRole('button', { name: 'Salva' }).click();
  const renamed = page.getByRole('row', { name: /New name/ });
  await expect(renamed).toBeVisible();
  await expect(renamed.getByTestId('rule_enabled_edit')).not.toBeChecked();

  await renamed.getByTestId('rule_delete_edit').click();
  await page.getByRole('button', { name: 'Elimina', exact: true }).click();
  await expect(page.getByRole('row', { name: /New name/ })).toHaveCount(0);
  const listed = await (await page.request.get('/api/rules')).json();
  expect(listed.some((item: { id: string }) => item.id === 'edit')).toBe(false);
});

test('in the dark theme of the launcher every icon is drawn light', async ({ page }) => {
  await page.route('**/backup-shell', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: '<iframe src="/?lang=it&theme=dark" style="position:fixed;inset:0;width:100%;height:100%;border:0"></iframe>',
    }),
  );
  await page.route('**/api/rules', (route) => route.fulfill({ json: [] }));
  await page.goto('/backup-shell');
  const app = page.frameLocator('iframe');
  await app.getByRole('button', { name: 'Cloud' }).click();
  await expect(app.locator('.empty_icon')).toBeVisible();

  // The icons are black strokes: in the dark theme each one is turned light, but the primary
  // buttons, whose dark icons sit on the accent.
  const filters = await app
    .locator('html')
    .evaluate((root) =>
      [...root.querySelectorAll('img')]
        .filter((icon) => !icon.closest('.app_button--primary'))
        .map((icon) => getComputedStyle(icon).filter),
    );
  expect(filters.length).toBeGreaterThan(3);
  expect(filters.every((filter) => filter.includes('invert(1)'))).toBe(true);
  await expect(app.locator('.empty_badge')).toHaveCSS('filter', 'none');
});
