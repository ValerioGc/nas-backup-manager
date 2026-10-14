export type SectionCode = 'scheduled' | 'cloud' | 'rtl';

export type ListColumn = { key: string; label: string };
export type ListRow = { id: string; [key: string]: string | number | null };

export type FolderKind = 'origin' | 'destination' | 'cloud';
export type RuleFolders = Record<FolderKind, string | null>;

/** Whether the destination follows the origin, removals included, or only receives copies. */
export type SyncMode = 'mirror' | 'copy';

export type NewBackupRule = {
  type: SectionCode;
  name: string;
  enabled: boolean;
  folders: RuleFolders;
  /** A real-time rule that writes what it makes into the folder it watches. */
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
 * When a scheduled or cloud rule runs, at an `HH:MM` time of the NAS. `weekday` (1 Monday to
 * 7 Sunday) belongs to a weekly rule, `day` to a monthly or yearly one, `month` (1 to 12) to a
 * yearly one: the fields a frequency does not use are null.
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

export type BackupRule = NewBackupRule & {
  id: string;
  createdAt: string;
};

export type RunStatus = 'ok' | 'failed' | 'interrupted';

/** How the last run of a rule ended; `at` in Unix milliseconds. */
export type LastRun = {
  at: number;
  status: RunStatus;
  message: string | null;
};

/**
 * A rule as the list shows it: its passwords are never sent back, and it says how its last run
 * went, when it runs next (Unix milliseconds) and whether it is running.
 */
export type ListedRule = BackupRule & {
  lastRun: LastRun | null;
  nextRunAt: number | null;
  running: boolean;
};

export type StepLabels = {
  folders: string;
  cloudFolder: string;
  schedule: string;
  filtering: string;
  operations: string;
  destination: string;
};

export type OriginFilterStatus = 'updated' | 'created' | 'size' | 'name';
export type OriginFilterOperator = 'before' | 'after' | 'greater' | 'less' | null;

export type OriginFilterAction = 'include' | 'exclude';

export type OriginFilterRule = {
  id: number;
  action: OriginFilterAction;
  /** `*`, or extensions like `.jpg, .png` */
  fileType: string;
  status: OriginFilterStatus;
  operator: OriginFilterOperator;
  value: string;
};
