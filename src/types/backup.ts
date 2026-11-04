export type SectionCode = 'scheduled' | 'cloud' | 'rtl';

export type ListColumn = { key: string; label: string };

export type FolderKind = 'origin' | 'destination' | 'cloud';
export type RuleFolders = Record<FolderKind, string | null>;

/** Whether the destination mirrors the origin, deletions included, or only receives copies. */
export type SyncMode = 'mirror' | 'copy';

export type NewBackupRule = {
  type: SectionCode;
  name: string;
  enabled: boolean;
  folders: RuleFolders;
  sameDestination: boolean;
  syncMode: SyncMode;
  schedule: RuleSchedule | null;
  filters: OriginFilterRule[];
  operations: FileOperation[];
  operationConfigurations: FileOperationConfigurations;
  duplicateResolution: DuplicateResolution | null;
  verifyArchivedFiles: boolean;
};

export type ScheduleFrequency = 'daily' | 'weekly' | 'monthly' | 'yearly';

/**
 * Schedule of a scheduled or cloud rule at an `HH:MM` NAS local time. `weekday`, 1 for Monday to
 * 7 for Sunday, belongs to weekly rules, `day` to monthly and yearly ones and `month`, 1 to 12, to
 * yearly ones. Fields the frequency does not use are null.
 */
export type RuleSchedule = {
  frequency: ScheduleFrequency;
  time: string;
  weekday: number | null;
  day: number | null;
  month: number | null;
};

export type DuplicateResolution = 'overwrite' | 'ignore' | 'copy';
export type FileOperation = 'batchRename' | 'archive' | 'encrypt';
/** Operations the form offers, since encryption is not available yet. */
export type OfferedOperation = Exclude<FileOperation, 'encrypt'>;

export type BatchRenameConfiguration = {
  filename: string;
  separator: '-' | '_';
  prefix: string;
  position: 'prefix' | 'suffix';
};

export type ArchiveConfiguration = {
  archiveName: string;
  password: string;
  encryptFileInfo: boolean;
  verifyArchive: boolean;
};

export type CryptConfiguration = {
  mode: 'encrypt' | 'decrypt';
  output: 'single' | 'folder' | 'singlefile';
  password: string;
};

export type FileOperationConfigurations = {
  batchRename: BatchRenameConfiguration | null;
  archive: ArchiveConfiguration | null;
  encrypt: CryptConfiguration | null;
};

/** Steps of the rule form. */
export type StepId = 'folders' | 'schedule' | 'filtering' | 'operations' | 'destination';

/** Rule as the form edits it, always with a schedule to fill. */
export type RuleForm = Omit<NewBackupRule, 'type' | 'enabled' | 'schedule'> & {
  schedule: RuleSchedule;
};

export type BackupRule = NewBackupRule & {
  id: string;
  createdAt: string;
};

export type RunStatus = 'ok' | 'failed' | 'interrupted';

/** Outcome of the last run of a rule, with `at` in Unix milliseconds. */
export type LastRun = {
  at: number;
  status: RunStatus;
  message: string | null;
};

/**
 * Rule as listed, never with its passwords, with the outcome of its last run, the next run in Unix
 * milliseconds and whether it is running.
 */
export type ListedRule = BackupRule & {
  lastRun: LastRun | null;
  nextRunAt: number | null;
  running: boolean;
};

export type OriginFilterStatus = 'updated' | 'created' | 'size' | 'name';
export type OriginFilterOperator = 'before' | 'after' | 'greater' | 'less' | null;

export type OriginFilterAction = 'include' | 'exclude';

export type OriginFilterRule = {
  id: number;
  action: OriginFilterAction;
  fileType: string;
  status: OriginFilterStatus;
  operator: OriginFilterOperator;
  value: string;
};
