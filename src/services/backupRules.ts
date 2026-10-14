import type {
  BackupRule,
  DuplicateResolution,
  FolderKind,
  FileOperation,
  FileOperationConfigurations,
  LastRun,
  ListedRule,
  NewBackupRule,
  OriginFilterAction,
  OriginFilterOperator,
  OriginFilterRule,
  OriginFilterStatus,
  RuleFolders,
  RuleSchedule,
  RunStatus,
  ScheduleFrequency,
  SectionCode,
  SyncMode,
} from '@/types/backup';

import { minuteOf } from '@valeriogc/nas-base-app';

// The minute of the day of an `HH:MM` time, or null when it is not one.
export { minuteOf };

const endpoint = 'api/rules';
const sectionCodes = new Set<SectionCode>(['scheduled', 'cloud', 'rtl']);
const fileOperations = new Set<FileOperation>(['batchRename', 'archive', 'encrypt']);
const filterStatuses = new Set<OriginFilterStatus>(['updated', 'created', 'size', 'name']);
const filterActions = new Set<OriginFilterAction>(['include', 'exclude']);
const syncModes = new Set<SyncMode>(['mirror', 'copy']);
const runStatuses = new Set<RunStatus>(['ok', 'failed', 'interrupted']);
const filterOperators = new Set<OriginFilterOperator>(['before', 'after', 'greater', 'less', null]);
const duplicateResolutions = new Set<DuplicateResolution | null>([
  'overwrite',
  'ignore',
  'copy',
  null,
]);
const frequencies = new Set<ScheduleFrequency>(['daily', 'weekly', 'monthly', 'yearly']);

/** The longest day of each month, February 29th included. */
export const monthDays = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/**
 * The folders each kind of backup needs before its rule can go on: a real-time rule watches its
 * origin, and writes into it when it uses no destination of its own.
 */
export function requiredFolders(section: SectionCode, sameDestination = false): FolderKind[] {
  if (section === 'cloud') {
    return ['cloud'];
  }
  return section === 'rtl' && sameDestination ? ['origin'] : ['origin', 'destination'];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** `*`, or 1 to 32 extensions like `.jpg, .png`, as the backend accepts them. */
export function extensionsValid(value: string): boolean {
  if (value.trim() === '*') {
    return true;
  }
  const parts = value
    .split(',')
    .map((part) => part.trim())
    .filter(Boolean);
  return (
    parts.length > 0 &&
    parts.length <= 32 &&
    parts.every((part) => /^\.[A-Za-z0-9]{1,16}$/.test(part))
  );
}

/** Whether a filter can be saved: its files, and a value its condition can use. */
export function filterComplete(filter: OriginFilterRule): boolean {
  if (!extensionsValid(filter.fileType)) {
    return false;
  }
  switch (filter.status) {
    case 'size': {
      const megabytes = Number(filter.value.trim().replace(',', '.'));
      return filter.value.trim() !== '' && Number.isFinite(megabytes) && megabytes >= 0;
    }
    case 'name':
      return filter.value.trim().length > 0 && filter.value.length <= 255;
    default:
      return filter.value !== '';
  }
}

export function filtersComplete(filters: OriginFilterRule[]): boolean {
  return filters.every(filterComplete);
}

function within(value: number | null, last: number): boolean {
  return value !== null && Number.isInteger(value) && value >= 1 && value <= last;
}

/**
 * Whether a schedule can be saved: a real time and exactly the fields its frequency uses, as the
 * backend checks it. A day past the end of a shorter month runs on its last day.
 */
export function scheduleComplete(schedule: RuleSchedule): boolean {
  const { frequency, weekday, day, month } = schedule;
  if (minuteOf(schedule.time) === null) {
    return false;
  }
  switch (frequency) {
    case 'daily':
      return weekday === null && day === null && month === null;
    case 'weekly':
      return within(weekday, 7) && day === null && month === null;
    case 'monthly':
      return weekday === null && within(day, 31) && month === null;
    case 'yearly':
      return (
        weekday === null && within(month, 12) && within(day, monthDays[(month ?? 1) - 1] ?? 31)
      );
  }
}

function isRuleSchedule(value: unknown): value is RuleSchedule {
  return (
    isRecord(value) &&
    frequencies.has(value.frequency as ScheduleFrequency) &&
    typeof value.time === 'string' &&
    (['weekday', 'day', 'month'] as const).every(
      (field) => value[field] === null || typeof value[field] === 'number',
    ) &&
    scheduleComplete(value as RuleSchedule)
  );
}

function isOriginFilter(value: unknown): value is OriginFilterRule {
  if (!isRecord(value)) {
    return false;
  }

  return (
    typeof value.id === 'number' &&
    Number.isInteger(value.id) &&
    filterActions.has(value.action as OriginFilterAction) &&
    typeof value.fileType === 'string' &&
    filterStatuses.has(value.status as OriginFilterStatus) &&
    filterOperators.has(value.operator as OriginFilterOperator) &&
    typeof value.value === 'string'
  );
}

function isFileOperationConfigurations(value: unknown): value is FileOperationConfigurations {
  if (!isRecord(value)) {
    return false;
  }

  const batchRename = value.batchRename;
  const archive = value.archive;
  const encrypt = value.encrypt;

  return (
    (batchRename === null ||
      (isRecord(batchRename) &&
        typeof batchRename.filename === 'string' &&
        (batchRename.separator === '-' || batchRename.separator === '_') &&
        typeof batchRename.prefix === 'string' &&
        (batchRename.position === 'prefix' || batchRename.position === 'suffix'))) &&
    (archive === null ||
      (isRecord(archive) &&
        typeof archive.archiveName === 'string' &&
        typeof archive.password === 'string' &&
        typeof archive.encryptFileInfo === 'boolean' &&
        typeof archive.verifyArchive === 'boolean')) &&
    (encrypt === null ||
      (isRecord(encrypt) &&
        (encrypt.mode === 'encrypt' || encrypt.mode === 'decrypt') &&
        (encrypt.output === 'single' ||
          encrypt.output === 'folder' ||
          encrypt.output === 'singlefile') &&
        typeof encrypt.password === 'string'))
  );
}

function isRuleFolders(value: unknown): value is RuleFolders {
  return (
    isRecord(value) &&
    (['origin', 'destination', 'cloud'] as const).every(
      (kind) => value[kind] === null || typeof value[kind] === 'string',
    )
  );
}

function isBackupRule(value: unknown): value is BackupRule {
  if (
    !isRecord(value) ||
    typeof value.id !== 'string' ||
    !sectionCodes.has(value.type as SectionCode) ||
    typeof value.name !== 'string' ||
    typeof value.enabled !== 'boolean' ||
    typeof value.createdAt !== 'string' ||
    !isRuleFolders(value.folders) ||
    typeof value.sameDestination !== 'boolean' ||
    !syncModes.has(value.syncMode as SyncMode) ||
    !(value.schedule === null || isRuleSchedule(value.schedule)) ||
    !duplicateResolutions.has(value.duplicateResolution as DuplicateResolution | null) ||
    typeof value.verifyArchivedFiles !== 'boolean' ||
    !Array.isArray(value.filters) ||
    !value.filters.every(isOriginFilter) ||
    !Array.isArray(value.operations) ||
    !value.operations.every((operation) => fileOperations.has(operation as FileOperation)) ||
    new Set(value.operations).size !== value.operations.length ||
    !isFileOperationConfigurations(value.operationConfigurations)
  ) {
    return false;
  }

  const configurations = value.operationConfigurations;
  return (
    (configurations.batchRename === null || value.operations.includes('batchRename')) &&
    (configurations.archive === null || value.operations.includes('archive')) &&
    (configurations.encrypt === null || value.operations.includes('encrypt'))
  );
}

async function responseBody(response: Response): Promise<unknown> {
  const text = await response.text();
  if (!text) {
    return null;
  }

  try {
    return JSON.parse(text) as unknown;
  } catch {
    throw new Error(`Il backend ha restituito una risposta non valida (HTTP ${response.status}).`);
  }
}

function errorMessage(payload: unknown, status: number): string {
  if (isRecord(payload) && typeof payload.error === 'string') {
    return payload.error;
  }
  return `Richiesta al backend non riuscita (HTTP ${status}).`;
}

function isLastRun(value: unknown): value is LastRun {
  return (
    isRecord(value) &&
    typeof value.at === 'number' &&
    runStatuses.has(value.status as RunStatus) &&
    (value.message === null || typeof value.message === 'string')
  );
}

function isListedRule(value: unknown): value is ListedRule {
  if (!isBackupRule(value)) {
    return false;
  }
  const listed: Record<string, unknown> = value;
  return (
    (listed.lastRun === null || isLastRun(listed.lastRun)) &&
    (listed.nextRunAt === null || typeof listed.nextRunAt === 'number') &&
    typeof listed.running === 'boolean'
  );
}

/** A request to the backend, which answers with JSON or nothing, or says why it refused. */
async function send(path: string, init?: RequestInit): Promise<unknown> {
  const response = await fetch(path, init);
  const payload = await responseBody(response);
  if (!response.ok) {
    throw new Error(errorMessage(payload, response.status));
  }
  return payload;
}

const json = (method: string, body: unknown): RequestInit => ({
  method,
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify(body),
});

export async function fetchBackupRules(): Promise<ListedRule[]> {
  const payload = await send(endpoint);
  if (!Array.isArray(payload) || !payload.every(isListedRule)) {
    throw new Error('Il backend ha restituito un elenco di regole non valido.');
  }
  return payload;
}

/**
 * Saves a new rule, or an existing one in place. An empty password keeps the one the rule has.
 */
export async function saveBackupRule(rule: BackupRule, existing = false): Promise<BackupRule> {
  const body: NewBackupRule & Pick<BackupRule, 'id' | 'createdAt'> = {
    id: rule.id,
    type: rule.type,
    name: rule.name.trim(),
    enabled: rule.enabled,
    createdAt: rule.createdAt,
    folders: { ...rule.folders },
    sameDestination: rule.sameDestination,
    syncMode: rule.syncMode,
    schedule: rule.schedule ? { ...rule.schedule } : null,
    filters: rule.filters.map((filter) => ({ ...filter })),
    operations: [...rule.operations],
    operationConfigurations: {
      batchRename: rule.operationConfigurations.batchRename
        ? { ...rule.operationConfigurations.batchRename }
        : null,
      archive: rule.operationConfigurations.archive
        ? { ...rule.operationConfigurations.archive }
        : null,
      encrypt: rule.operationConfigurations.encrypt
        ? { ...rule.operationConfigurations.encrypt }
        : null,
    },
    duplicateResolution: rule.duplicateResolution,
    verifyArchivedFiles: rule.verifyArchivedFiles,
  };
  const payload = await send(
    existing ? `${endpoint}/${encodeURIComponent(rule.id)}` : endpoint,
    json(existing ? 'PUT' : 'POST', body),
  );
  if (!isBackupRule(payload)) {
    throw new Error('Il backend ha restituito una regola salvata non valida.');
  }
  return payload;
}

const ruleAddress = (id: string, action = '') =>
  `${endpoint}/${encodeURIComponent(id)}` + (action ? '/' + action : '');

export async function deleteBackupRule(id: string): Promise<void> {
  await send(ruleAddress(id), { method: 'DELETE' });
}

export async function switchBackupRule(id: string, enabled: boolean): Promise<void> {
  await send(ruleAddress(id), json('PATCH', { enabled }));
}

/** Starts a run now; the run goes on in the backend. */
export async function runBackupRule(id: string): Promise<void> {
  await send(ruleAddress(id, 'run'), { method: 'POST' });
}

export async function stopBackupRule(id: string): Promise<void> {
  await send(ruleAddress(id, 'stop'), { method: 'POST' });
}
