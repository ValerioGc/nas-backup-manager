-- The backup rules. What the scheduler, the watcher and the list read stays in plain columns; the
-- file operations and their options, passwords included, are one encrypted JSON document
-- (`operation_data`), read whole when a rule runs.
--
-- A real-time rule watches its origin and may write into it (`same_destination`). `sync_mode`
-- decides whether the destination follows the origin (`mirror`, which removes what the rule copied
-- once its original is gone) or only receives copies (`copy`). The schedule is in the local time of
-- the NAS, the time as the minute of the day; a real-time rule has none, and a column the frequency
-- does not use stays NULL. The last run ended at `last_run_at` (Unix milliseconds) and how;
-- `archive_path` and `archive_fingerprint` are the archive the rule keeps and what was in it, so an
-- unchanged origin is not archived again.
CREATE TABLE backup_rules (
    id                   TEXT    PRIMARY KEY NOT NULL,
    rule_type            TEXT    NOT NULL CHECK (rule_type IN ('scheduled', 'cloud', 'rtl')),
    name                 TEXT    NOT NULL,
    created_at           TEXT    NOT NULL,
    enabled              INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    operation_data       TEXT    NOT NULL DEFAULT '{}',
    origin_path          TEXT,
    destination_path     TEXT,
    cloud_path           TEXT,
    same_destination     INTEGER NOT NULL DEFAULT 0 CHECK (same_destination IN (0, 1)),
    sync_mode            TEXT    NOT NULL DEFAULT 'mirror' CHECK (sync_mode IN ('mirror', 'copy')),
    schedule_frequency   TEXT    CHECK (schedule_frequency IN ('daily', 'weekly', 'monthly', 'yearly')),
    schedule_minute      INTEGER CHECK (schedule_minute BETWEEN 0 AND 1439),
    schedule_weekday     INTEGER CHECK (schedule_weekday BETWEEN 1 AND 7),
    schedule_day         INTEGER CHECK (schedule_day BETWEEN 1 AND 31),
    schedule_month       INTEGER CHECK (schedule_month BETWEEN 1 AND 12),
    duplicate_resolution TEXT    CHECK (duplicate_resolution IN ('overwrite', 'ignore', 'copy')),
    verify_archived      INTEGER NOT NULL DEFAULT 0 CHECK (verify_archived IN (0, 1)),
    last_run_at          INTEGER,
    last_run_status      TEXT    CHECK (last_run_status IN ('ok', 'failed', 'interrupted')),
    last_run_message     TEXT,
    archive_path         TEXT,
    archive_fingerprint  TEXT
) STRICT;

-- The filters of a rule, in their order: each includes or excludes the files it matches. The
-- unique order is also the index the rules are read through.
CREATE TABLE origin_filters (
    rule_id   TEXT    NOT NULL REFERENCES backup_rules(id) ON DELETE CASCADE,
    position  INTEGER NOT NULL CHECK (position >= 0),
    filter_id INTEGER NOT NULL,
    action    TEXT    NOT NULL DEFAULT 'include' CHECK (action IN ('include', 'exclude')),
    file_type TEXT    NOT NULL,
    status    TEXT    NOT NULL CHECK (status IN ('updated', 'created', 'size', 'name')),
    operator  TEXT    CHECK (operator IN ('before', 'after', 'greater', 'less')),
    value     TEXT    NOT NULL,
    PRIMARY KEY (rule_id, filter_id),
    UNIQUE (rule_id, position)
) STRICT;

-- The files a rule copied, and only those: mirroring removes from the destination what is listed
-- here and gone from the origin, never a file that was there before the rule. `parent` is the
-- folder of the original, relative to the origin ('' at its top); `destination` is the copy,
-- relative to the destination folder. Size and time (nanoseconds) are those of the original when
-- it was copied. Read one folder at a time, by its key: stored in it, without a separate rowid.
CREATE TABLE rule_files (
    rule_id     TEXT    NOT NULL REFERENCES backup_rules(id) ON DELETE CASCADE,
    parent      TEXT    NOT NULL,
    name        TEXT    NOT NULL,
    destination TEXT    NOT NULL,
    size        INTEGER NOT NULL,
    modified    INTEGER NOT NULL,
    PRIMARY KEY (rule_id, parent, name)
) STRICT, WITHOUT ROWID;
