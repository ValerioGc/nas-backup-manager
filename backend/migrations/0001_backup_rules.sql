-- Backup rules. File operations and their passwords are one encrypted JSON document in
-- `operation_data`.
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

-- Filters of a rule in order. The unique position is also the index used to read them.
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

-- Files copied by a rule, the only ones a mirror may remove. `parent` is relative to the origin
-- and `destination` to the destination folder. `size` and `modified`, in nanoseconds, describe the
-- original at copy time.
CREATE TABLE rule_files (
    rule_id     TEXT    NOT NULL REFERENCES backup_rules(id) ON DELETE CASCADE,
    parent      TEXT    NOT NULL,
    name        TEXT    NOT NULL,
    destination TEXT    NOT NULL,
    size        INTEGER NOT NULL,
    modified    INTEGER NOT NULL,
    PRIMARY KEY (rule_id, parent, name)
) STRICT, WITHOUT ROWID;
