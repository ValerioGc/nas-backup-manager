import { afterEach, expect, it, vi } from 'vitest';
import {
  deleteBackupRule,
  extensionsValid,
  fetchBackupRules,
  filtersComplete,
  minuteOf,
  requiredFolders,
  runBackupRule,
  saveBackupRule,
  scheduleComplete,
  stopBackupRule,
  switchBackupRule,
} from '@/services/backupRules';
import type { BackupRule, ListedRule, OriginFilterRule, RuleSchedule } from '@/types/backup';

afterEach(() => vi.unstubAllGlobals());

const rule: BackupRule = {
  id: 'rule-1',
  type: 'rtl',
  name: 'Photos',
  enabled: true,
  createdAt: '2026-10-07T20:00:00.000Z',
  folders: { origin: '/Photos', destination: '/Backup/photos', cloud: null },
  sameDestination: false,
  syncMode: 'mirror',
  schedule: null,
  filters: [
    {
      id: 1,
      action: 'include',
      fileType: '.pdf',
      status: 'updated',
      operator: 'before',
      value: 'today',
    },
  ],
  operations: ['archive'],
  operationConfigurations: {
    batchRename: null,
    archive: {
      archiveName: 'daily',
      password: 'test-password',
      encryptFileInfo: true,
      verifyArchive: true,
    },
    encrypt: null,
  },
  duplicateResolution: 'copy',
  verifyArchivedFiles: true,
};

const listed: ListedRule = {
  ...rule,
  lastRun: { at: 1_790_000_000_000, status: 'failed', message: 'disk full' },
  nextRunAt: null,
  running: false,
};

it('loads saved rules from the relative backend endpoint', async () => {
  const request = vi
    .fn()
    .mockResolvedValue(new Response(JSON.stringify([listed]), { status: 200 }));
  vi.stubGlobal('fetch', request);

  await expect(fetchBackupRules()).resolves.toEqual([listed]);
  expect(request).toHaveBeenCalledWith('api/rules', undefined);
});

it('refuses a rule without how its runs went', async () => {
  for (const broken of [
    rule,
    { ...listed, running: 'no' },
    { ...listed, nextRunAt: 'soon' },
    { ...listed, lastRun: { at: 1, status: 'maybe', message: null } },
    { ...listed, name: 3 },
    { ...listed, syncMode: 'move' },
    { ...listed, filters: [{ ...listed.filters[0], action: 'keep' }] },
  ]) {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(new Response(JSON.stringify([broken]), { status: 200 })),
    );
    await expect(fetchBackupRules()).rejects.toThrow('elenco di regole non valido');
  }
});

it('refuses saved rules whose folders are not paths', async () => {
  vi.stubGlobal(
    'fetch',
    vi
      .fn()
      .mockResolvedValue(
        new Response(JSON.stringify([{ ...listed, folders: { origin: 1 } }]), { status: 200 }),
      ),
  );

  await expect(fetchBackupRules()).rejects.toThrow('elenco di regole non valido');
});

it('sends all configured rule options in one save request', async () => {
  const request = vi
    .fn()
    .mockImplementation(() => Promise.resolve(new Response(JSON.stringify(rule), { status: 201 })));
  vi.stubGlobal('fetch', request);

  await expect(saveBackupRule({ ...rule, name: '  Photos ' })).resolves.toEqual(rule);
  expect(request).toHaveBeenCalledWith('api/rules', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(rule),
  });

  await saveBackupRule(rule, true);
  expect(request).toHaveBeenLastCalledWith(
    'api/rules/rule-1',
    expect.objectContaining({ method: 'PUT' }),
  );
});

it('switches, runs, stops and deletes a rule at its own address', async () => {
  const request = vi
    .fn()
    .mockImplementation(() => Promise.resolve(new Response(null, { status: 204 })));
  vi.stubGlobal('fetch', request);

  await switchBackupRule('a b', false);
  expect(request).toHaveBeenLastCalledWith('api/rules/a%20b', {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ enabled: false }),
  });
  await runBackupRule('a');
  expect(request).toHaveBeenLastCalledWith('api/rules/a/run', { method: 'POST' });
  await stopBackupRule('a');
  expect(request).toHaveBeenLastCalledWith('api/rules/a/stop', { method: 'POST' });
  await deleteBackupRule('a');
  expect(request).toHaveBeenLastCalledWith('api/rules/a', { method: 'DELETE' });

  request.mockResolvedValueOnce(
    new Response(JSON.stringify({ error: 'The rule is running already.' }), { status: 409 }),
  );
  await expect(runBackupRule('a')).rejects.toThrow('The rule is running already.');
  request.mockResolvedValueOnce(new Response('{', { status: 200 }));
  await expect(stopBackupRule('a')).rejects.toThrow('risposta non valida');
  request.mockResolvedValueOnce(new Response('', { status: 500 }));
  await expect(deleteBackupRule('a')).rejects.toThrow('HTTP 500');
});

it('asks each kind of backup for its folders', () => {
  expect(requiredFolders('scheduled')).toEqual(['origin', 'destination']);
  expect(requiredFolders('rtl')).toEqual(['origin', 'destination']);
  expect(requiredFolders('rtl', true)).toEqual(['origin']);
  expect(requiredFolders('cloud')).toEqual(['cloud']);
});

it('checks the filters as the backend does', () => {
  expect(extensionsValid(' * ')).toBe(true);
  expect(extensionsValid('.jpg, .PNG')).toBe(true);
  expect(extensionsValid('jpg')).toBe(false);
  expect(extensionsValid('')).toBe(false);
  expect(extensionsValid(Array.from({ length: 33 }, (_, index) => `.e${index}`).join(','))).toBe(
    false,
  );
  const filter = (changes: Partial<OriginFilterRule>): OriginFilterRule => ({
    id: 1,
    action: 'include',
    fileType: '*',
    status: 'size',
    operator: 'greater',
    value: '1,5',
    ...changes,
  });
  expect(filtersComplete([filter({})])).toBe(true);
  expect(filtersComplete([filter({ value: '' })])).toBe(false);
  expect(filtersComplete([filter({ value: 'big' })])).toBe(false);
  expect(filtersComplete([filter({ fileType: 'pdf' })])).toBe(false);
  expect(filtersComplete([filter({ status: 'name', operator: null, value: ' ' })])).toBe(false);
  expect(filtersComplete([filter({ status: 'name', operator: null, value: '*.tmp' })])).toBe(true);
  expect(filtersComplete([filter({ status: 'updated', operator: 'before', value: 'today' })])).toBe(
    true,
  );
});

it('reports backend validation errors', async () => {
  vi.stubGlobal(
    'fetch',
    vi
      .fn()
      .mockResolvedValue(
        new Response(JSON.stringify({ error: 'Unknown backup rule type.' }), { status: 400 }),
      ),
  );

  await expect(saveBackupRule(rule)).rejects.toThrow('Unknown backup rule type.');
});

it('reads a time as the minute of the day', () => {
  expect(minuteOf('00:00')).toBe(0);
  expect(minuteOf('23:59')).toBe(1439);
  expect(minuteOf('24:00')).toBeNull();
  expect(minuteOf('2:30')).toBeNull();
  expect(minuteOf('')).toBeNull();
});

it('accepts a schedule with exactly the fields its frequency uses', () => {
  const at = (schedule: Partial<RuleSchedule>): RuleSchedule => ({
    frequency: 'daily',
    time: '02:00',
    weekday: null,
    day: null,
    month: null,
    ...schedule,
  });

  expect(scheduleComplete(at({}))).toBe(true);
  expect(scheduleComplete(at({ time: '' }))).toBe(false);
  expect(scheduleComplete(at({ weekday: 1 }))).toBe(false);
  expect(scheduleComplete(at({ frequency: 'weekly', weekday: 7 }))).toBe(true);
  expect(scheduleComplete(at({ frequency: 'weekly' }))).toBe(false);
  expect(scheduleComplete(at({ frequency: 'monthly', day: 31 }))).toBe(true);
  expect(scheduleComplete(at({ frequency: 'monthly', day: 0 }))).toBe(false);
  expect(scheduleComplete(at({ frequency: 'yearly', day: 29, month: 2 }))).toBe(true);
  expect(scheduleComplete(at({ frequency: 'yearly', day: 30, month: 2 }))).toBe(false);
  expect(scheduleComplete(at({ frequency: 'yearly', day: 1, month: 13 }))).toBe(false);
  expect(scheduleComplete(at({ frequency: 'yearly', day: 1.5, month: 1 }))).toBe(false);
});

it('loads a scheduled rule and refuses a schedule that does not match its frequency', async () => {
  const scheduled = {
    ...listed,
    type: 'scheduled',
    folders: { origin: '/Photos', destination: '/Backup', cloud: null },
    schedule: { frequency: 'weekly', time: '02:00', weekday: 1, day: null, month: null },
    nextRunAt: 1_790_000_000_000,
  };
  vi.stubGlobal(
    'fetch',
    vi.fn().mockResolvedValue(new Response(JSON.stringify([scheduled]), { status: 200 })),
  );
  await expect(fetchBackupRules()).resolves.toEqual([scheduled]);

  vi.stubGlobal(
    'fetch',
    vi
      .fn()
      .mockResolvedValue(
        new Response(
          JSON.stringify([{ ...scheduled, schedule: { ...scheduled.schedule, weekday: null } }]),
          { status: 200 },
        ),
      ),
  );
  await expect(fetchBackupRules()).rejects.toThrow('elenco di regole non valido');
});
