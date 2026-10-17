//! The backup rules: what they hold, how they are checked and how they are kept in the database.
//!
//! What the scheduler and the watcher query stays in plain columns (the type, the folders, the
//! schedule, the destination options, the outcome of the last run) and the filters in their own
//! table. The file operations and their options, passwords included, are one encrypted document:
//! they are read whole, only when a rule runs. A password never goes back to the browser: the list
//! sends it empty, and a rule saved with an empty one keeps the password it had.

use crate::{
    filters,
    runner::Runner,
    schedule::{self, Frequency, RuleSchedule},
    secrets::{decrypt_secret, encrypt_secret},
    ApiError, AppState,
};
use aes_gcm::Aes256Gcm;
use axum::{
    extract::{Path as UrlPath, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};
use std::{
    collections::HashMap,
    path::{Component, Path},
};

const MAX_NAME: usize = 80;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRule {
    pub id: String,
    #[serde(rename = "type")]
    pub rule_type: String,
    #[serde(default)]
    pub name: String,
    pub created_at: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub folders: RuleFolders,
    /// A real-time rule that writes what it makes into the folder it watches.
    #[serde(default)]
    pub same_destination: bool,
    #[serde(default)]
    pub sync_mode: SyncMode,
    #[serde(default)]
    pub schedule: Option<RuleSchedule>,
    pub filters: Vec<OriginFilter>,
    #[serde(default)]
    pub operations: Vec<FileOperation>,
    #[serde(default)]
    pub operation_configurations: FileOperationConfigurations,
    #[serde(default)]
    pub duplicate_resolution: Option<DuplicateResolution>,
    #[serde(default)]
    pub verify_archived_files: bool,
}

fn enabled() -> bool {
    true
}

/// The folders of a rule: the logical paths chosen in Explorer, or a network share's mount point.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleFolders {
    #[serde(default)]
    pub origin: Option<String>,
    #[serde(default)]
    pub destination: Option<String>,
    #[serde(default)]
    pub cloud: Option<String>,
}

/// Whether the destination follows the origin, files removed included, or only receives copies.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SyncMode {
    #[default]
    Mirror,
    Copy,
}

impl SyncMode {
    pub fn id(self) -> &'static str {
        match self {
            Self::Mirror => "mirror",
            Self::Copy => "copy",
        }
    }

    fn parse(id: &str) -> Self {
        if id == "copy" {
            Self::Copy
        } else {
            Self::Mirror
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FileOperation {
    BatchRename,
    Archive,
    Encrypt,
}

/// What a rule does with a file the destination already holds and the rule did not put there.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DuplicateResolution {
    Overwrite,
    Ignore,
    Copy,
}

impl DuplicateResolution {
    fn id(self) -> &'static str {
        match self {
            Self::Overwrite => "overwrite",
            Self::Ignore => "ignore",
            Self::Copy => "copy",
        }
    }

    fn parse(id: &str) -> Option<Self> {
        [Self::Overwrite, Self::Ignore, Self::Copy]
            .into_iter()
            .find(|resolution| resolution.id() == id)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileOperationConfigurations {
    #[serde(default)]
    pub batch_rename: Option<BatchRenameConfiguration>,
    #[serde(default)]
    pub archive: Option<ArchiveConfiguration>,
    #[serde(default)]
    pub encrypt: Option<CryptConfiguration>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BatchRenameConfiguration {
    /// The example of the preview: it renames nothing.
    pub filename: String,
    pub separator: String,
    /// The text added to the name.
    pub prefix: String,
    pub position: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveConfiguration {
    pub archive_name: String,
    pub password: String,
    pub encrypt_file_info: bool,
    pub verify_archive: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CryptConfiguration {
    pub mode: String,
    pub output: String,
    pub password: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredOperationData {
    #[serde(default)]
    pub operations: Vec<FileOperation>,
    #[serde(default)]
    pub operation_configurations: FileOperationConfigurations,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OriginFilter {
    pub id: i64,
    #[serde(default = "include")]
    pub action: String,
    pub file_type: String,
    pub status: String,
    pub operator: Option<String>,
    pub value: String,
}

fn include() -> String {
    "include".to_owned()
}

/// How the last run of a rule ended.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LastRun {
    /// Unix milliseconds
    pub at: i64,
    /// `ok`, `failed` or `interrupted`
    pub status: String,
    pub message: Option<String>,
}

/// A rule as the list shows it: without its passwords, with its last and next runs.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleView {
    #[serde(flatten)]
    pub rule: BackupRule,
    pub last_run: Option<LastRun>,
    /// Unix milliseconds; none for a real-time rule, one turned off or one not run yet
    pub next_run_at: Option<i64>,
    pub running: bool,
}

/// A rule as it is run: its passwords readable, and what it keeps of the archive it made.
#[derive(Clone, Debug)]
pub struct StoredRule {
    pub rule: BackupRule,
    pub archive_path: Option<String>,
    pub archive_fingerprint: Option<String>,
}

/// The columns of a rule, in one place for every query that reads one.
macro_rules! columns {
    () => {
        "id, rule_type, name, created_at, enabled, operation_data, origin_path,
         destination_path, cloud_path, same_destination, sync_mode, schedule_frequency,
         schedule_minute, schedule_weekday, schedule_day, schedule_month, duplicate_resolution,
         verify_archived, last_run_at, last_run_status, last_run_message, archive_path,
         archive_fingerprint"
    };
}

fn origin_filter(row: &SqliteRow) -> Result<OriginFilter, sqlx::Error> {
    Ok(OriginFilter {
        id: row.try_get("filter_id")?,
        action: row.try_get("action")?,
        file_type: row.try_get("file_type")?,
        status: row.try_get("status")?,
        operator: row.try_get("operator")?,
        value: row.try_get("value")?,
    })
}

/// The filters of every rule, or of one, grouped by rule and in their order.
async fn filters_of(
    db: &SqlitePool,
    id: Option<&str>,
) -> Result<HashMap<String, Vec<OriginFilter>>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT rule_id, filter_id, action, file_type, status, operator, value
         FROM origin_filters WHERE ?1 IS NULL OR rule_id = ?1 ORDER BY rule_id, position",
    )
    .bind(id)
    .fetch_all(db)
    .await?;
    let mut filters: HashMap<String, Vec<OriginFilter>> = HashMap::new();
    for row in rows {
        filters
            .entry(row.try_get("rule_id")?)
            .or_default()
            .push(origin_filter(&row)?);
    }
    Ok(filters)
}

fn stored_rule(
    row: &SqliteRow,
    filters: &mut HashMap<String, Vec<OriginFilter>>,
    cipher: &Aes256Gcm,
) -> Result<(StoredRule, Option<LastRun>), ApiError> {
    let get = |error: sqlx::Error| ApiError::internal(error);
    let id: String = row.try_get("id").map_err(get)?;
    let operation_data: String = row.try_get("operation_data").map_err(get)?;
    let operation_data = decrypt_operation_data(&operation_data, cipher)?;
    let resolution: Option<String> = row.try_get("duplicate_resolution").map_err(get)?;
    let sync_mode: String = row.try_get("sync_mode").map_err(get)?;
    let last_at: Option<i64> = row.try_get("last_run_at").map_err(get)?;
    let last_status: Option<String> = row.try_get("last_run_status").map_err(get)?;
    let last_run = last_at.zip(last_status).map(|(at, status)| LastRun {
        at,
        status,
        message: row.try_get("last_run_message").ok().flatten(),
    });
    let rule = BackupRule {
        rule_type: row.try_get("rule_type").map_err(get)?,
        name: row.try_get("name").map_err(get)?,
        created_at: row.try_get("created_at").map_err(get)?,
        enabled: row.try_get("enabled").map_err(get)?,
        folders: RuleFolders {
            origin: row.try_get("origin_path").map_err(get)?,
            destination: row.try_get("destination_path").map_err(get)?,
            cloud: row.try_get("cloud_path").map_err(get)?,
        },
        same_destination: row.try_get("same_destination").map_err(get)?,
        sync_mode: SyncMode::parse(&sync_mode),
        schedule: stored_schedule(row).map_err(get)?,
        filters: filters.remove(&id).unwrap_or_default(),
        operations: operation_data.operations,
        operation_configurations: operation_data.operation_configurations,
        duplicate_resolution: resolution.as_deref().and_then(DuplicateResolution::parse),
        verify_archived_files: row.try_get("verify_archived").map_err(get)?,
        id,
    };
    Ok((
        StoredRule {
            rule,
            archive_path: row.try_get("archive_path").map_err(get)?,
            archive_fingerprint: row.try_get("archive_fingerprint").map_err(get)?,
        },
        last_run,
    ))
}

/// One rule as it is run, passwords readable, or none when there is no such rule.
pub async fn load(
    db: &SqlitePool,
    cipher: &Aes256Gcm,
    id: &str,
) -> Result<Option<StoredRule>, ApiError> {
    let row = sqlx::query(concat!(
        "SELECT ",
        columns!(),
        " FROM backup_rules WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(db)
    .await
    .map_err(ApiError::internal)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let mut filters = filters_of(db, Some(id)).await.map_err(ApiError::internal)?;
    stored_rule(&row, &mut filters, cipher).map(|(stored, _)| Some(stored))
}

/// Every rule, in the order they were made, with how their last run ended.
pub async fn load_all(
    db: &SqlitePool,
    cipher: &Aes256Gcm,
) -> Result<Vec<(StoredRule, Option<LastRun>)>, ApiError> {
    let rows = sqlx::query(concat!(
        "SELECT ",
        columns!(),
        " FROM backup_rules ORDER BY created_at, id"
    ))
    .fetch_all(db)
    .await
    .map_err(ApiError::internal)?;
    let mut filters = filters_of(db, None).await.map_err(ApiError::internal)?;
    rows.iter()
        .map(|row| stored_rule(row, &mut filters, cipher))
        .collect()
}

/// The schedule of a row: none for a real-time rule, or for one saved before schedules existed.
fn stored_schedule(row: &SqliteRow) -> Result<Option<RuleSchedule>, sqlx::Error> {
    let frequency: Option<String> = row.try_get("schedule_frequency")?;
    let minute: Option<u16> = row.try_get("schedule_minute")?;
    let (Some(frequency), Some(minute)) = (frequency.as_deref().and_then(Frequency::parse), minute)
    else {
        return Ok(None);
    };
    Ok(Some(RuleSchedule {
        frequency,
        time: schedule::time_of(minute),
        weekday: row.try_get("schedule_weekday")?,
        day: row.try_get("schedule_day")?,
        month: row.try_get("schedule_month")?,
    }))
}

/// A rule without its passwords, as the browser sees it.
pub fn without_passwords(mut rule: BackupRule) -> BackupRule {
    let configurations = &mut rule.operation_configurations;
    if let Some(archive) = &mut configurations.archive {
        archive.password.clear();
    }
    if let Some(encrypt) = &mut configurations.encrypt {
        encrypt.password.clear();
    }
    rule
}

pub async fn list_rules(State(state): State<AppState>) -> Result<Json<Vec<RuleView>>, ApiError> {
    let rules = load_all(&state.database, &state.secret_cipher).await?;
    let mut views = Vec::with_capacity(rules.len());
    for (stored, last_run) in rules {
        let id = stored.rule.id.clone();
        views.push(RuleView {
            next_run_at: state.runner.next_run(&id),
            running: state.runner.is_running(&id),
            rule: without_passwords(stored.rule),
            last_run,
        });
    }
    Ok(Json(views))
}

pub async fn create_rule(
    State(state): State<AppState>,
    Json(rule): Json<BackupRule>,
) -> Result<(StatusCode, Json<BackupRule>), ApiError> {
    let rule = normalized(rule)?;
    validate_rule(&rule)?;
    if load(&state.database, &state.secret_cipher, &rule.id)
        .await?
        .is_some()
    {
        return Err(ApiError::conflict("A rule with this id exists already."));
    }
    require_passwords(&rule)?;
    save(&state.database, &state.secret_cipher, &rule, false).await?;
    state.runner.rules_changed(Some(&rule.id)).await;
    Ok((StatusCode::CREATED, Json(without_passwords(rule))))
}

pub async fn update_rule(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<String>,
    Json(rule): Json<BackupRule>,
) -> Result<Json<BackupRule>, ApiError> {
    if rule.id != id {
        return Err(ApiError::bad_request(
            "The rule id does not match the address.",
        ));
    }
    let mut rule = normalized(rule)?;
    validate_rule(&rule)?;
    let current = load(&state.database, &state.secret_cipher, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("No such rule."))?;
    if state.runner.is_running(&id) {
        return Err(ApiError::conflict("The rule is running: stop it first."));
    }
    keep_passwords(&mut rule, &current.rule);
    require_passwords(&rule)?;
    // Another origin, destination or way of copying: what the rule copied before is no longer its
    // own to update or remove, and stays where it is.
    let forget = layout(&rule) != layout(&current.rule);
    save(&state.database, &state.secret_cipher, &rule, forget).await?;
    state.runner.rules_changed(Some(&id)).await;
    Ok(Json(without_passwords(rule)))
}

pub async fn delete_rule(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<String>,
) -> Result<StatusCode, ApiError> {
    if state.runner.is_running(&id) {
        return Err(ApiError::conflict("The rule is running: stop it first."));
    }
    let deleted = sqlx::query("DELETE FROM backup_rules WHERE id = ?")
        .bind(&id)
        .execute(&state.database)
        .await
        .map_err(ApiError::internal)?;
    if deleted.rows_affected() == 0 {
        return Err(ApiError::not_found("No such rule."));
    }
    state.runner.rules_changed(Some(&id)).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct Switch {
    pub enabled: bool,
}

pub async fn switch_rule(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<String>,
    Json(switch): Json<Switch>,
) -> Result<StatusCode, ApiError> {
    let switched = sqlx::query("UPDATE backup_rules SET enabled = ? WHERE id = ?")
        .bind(switch.enabled)
        .bind(&id)
        .execute(&state.database)
        .await
        .map_err(ApiError::internal)?;
    if switched.rows_affected() == 0 {
        return Err(ApiError::not_found("No such rule."));
    }
    state.runner.rules_changed(Some(&id)).await;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn run_rule(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<String>,
) -> Result<StatusCode, ApiError> {
    Runner::start(&state.runner, &id, None, true).await?;
    Ok(StatusCode::ACCEPTED)
}

pub async fn stop_rule(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<String>,
) -> Result<StatusCode, ApiError> {
    if state.runner.stop(&id) {
        Ok(StatusCode::ACCEPTED)
    } else {
        Err(ApiError::not_found("The rule is not running."))
    }
}

/// The rule as it is kept: a trimmed name, the extensions of the filters in one form, and a rule
/// that writes into its own origin only copying, never mirroring the origin onto itself.
fn normalized(mut rule: BackupRule) -> Result<BackupRule, ApiError> {
    rule.name = rule.name.trim().to_owned();
    for filter in &mut rule.filters {
        filter.file_type =
            filters::normalize_extensions(&filter.file_type).map_err(ApiError::bad_request)?;
    }
    if rule.same_destination {
        rule.sync_mode = SyncMode::Copy;
    }
    Ok(rule)
}

/// What decides which files of the destination the rule made: when it changes they are forgotten.
fn layout(rule: &BackupRule) -> String {
    let configurations = &rule.operation_configurations;
    serde_json::json!([
        rule.folders,
        rule.same_destination,
        rule.sync_mode,
        rule.operations,
        configurations.batch_rename,
        configurations
            .archive
            .as_ref()
            .map(|archive| &archive.archive_name),
    ])
    .to_string()
}

/// An empty password in a saved rule means the one it had.
fn keep_passwords(rule: &mut BackupRule, current: &BackupRule) {
    let saved = &current.operation_configurations;
    let configurations = &mut rule.operation_configurations;
    if let (Some(archive), Some(before)) = (&mut configurations.archive, &saved.archive) {
        if archive.password.is_empty() {
            archive.password.clone_from(&before.password);
        }
    }
    if let (Some(encrypt), Some(before)) = (&mut configurations.encrypt, &saved.encrypt) {
        if encrypt.password.is_empty() {
            encrypt.password.clone_from(&before.password);
        }
    }
}

fn require_passwords(rule: &BackupRule) -> Result<(), ApiError> {
    let configurations = &rule.operation_configurations;
    let archive = configurations
        .archive
        .as_ref()
        .is_some_and(|archive| archive.password.is_empty());
    let encrypt = configurations
        .encrypt
        .as_ref()
        .is_some_and(|encrypt| encrypt.password.is_empty());
    if archive || encrypt {
        return Err(ApiError::bad_request("A password is required."));
    }
    Ok(())
}

async fn save(
    db: &SqlitePool,
    cipher: &Aes256Gcm,
    rule: &BackupRule,
    forget_files: bool,
) -> Result<(), ApiError> {
    let operation_data = encrypt_operation_data(rule, cipher)?;
    let schedule = rule.schedule.as_ref();
    let mut transaction = db.begin().await.map_err(ApiError::internal)?;

    sqlx::query(
        "INSERT INTO backup_rules
         (id, rule_type, name, created_at, enabled, operation_data, origin_path, destination_path,
          cloud_path, same_destination, sync_mode, schedule_frequency, schedule_minute,
          schedule_weekday, schedule_day, schedule_month, duplicate_resolution, verify_archived)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
             rule_type = excluded.rule_type,
             name = excluded.name,
             enabled = excluded.enabled,
             operation_data = excluded.operation_data,
             origin_path = excluded.origin_path,
             destination_path = excluded.destination_path,
             cloud_path = excluded.cloud_path,
             same_destination = excluded.same_destination,
             sync_mode = excluded.sync_mode,
             schedule_frequency = excluded.schedule_frequency,
             schedule_minute = excluded.schedule_minute,
             schedule_weekday = excluded.schedule_weekday,
             schedule_day = excluded.schedule_day,
             schedule_month = excluded.schedule_month,
             duplicate_resolution = excluded.duplicate_resolution,
             verify_archived = excluded.verify_archived",
    )
    .bind(&rule.id)
    .bind(&rule.rule_type)
    .bind(&rule.name)
    .bind(&rule.created_at)
    .bind(rule.enabled)
    .bind(operation_data)
    .bind(&rule.folders.origin)
    .bind(&rule.folders.destination)
    .bind(&rule.folders.cloud)
    .bind(rule.same_destination)
    .bind(rule.sync_mode.id())
    .bind(schedule.map(|schedule| schedule.frequency.id()))
    .bind(schedule.and_then(|schedule| schedule::minute_of(&schedule.time)))
    .bind(schedule.and_then(|schedule| schedule.weekday))
    .bind(schedule.and_then(|schedule| schedule.day))
    .bind(schedule.and_then(|schedule| schedule.month))
    .bind(rule.duplicate_resolution.map(DuplicateResolution::id))
    .bind(rule.verify_archived_files)
    .execute(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;
    if forget_files {
        sqlx::query("DELETE FROM rule_files WHERE rule_id = ?")
            .bind(&rule.id)
            .execute(&mut *transaction)
            .await
            .map_err(ApiError::internal)?;
        sqlx::query(
            "UPDATE backup_rules SET archive_path = NULL, archive_fingerprint = NULL WHERE id = ?",
        )
        .bind(&rule.id)
        .execute(&mut *transaction)
        .await
        .map_err(ApiError::internal)?;
    }
    sqlx::query("DELETE FROM origin_filters WHERE rule_id = ?")
        .bind(&rule.id)
        .execute(&mut *transaction)
        .await
        .map_err(ApiError::internal)?;

    for (position, filter) in rule.filters.iter().enumerate() {
        sqlx::query(
            "INSERT INTO origin_filters
             (rule_id, position, filter_id, action, file_type, status, operator, value)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&rule.id)
        .bind(position as i64)
        .bind(filter.id)
        .bind(&filter.action)
        .bind(&filter.file_type)
        .bind(&filter.status)
        .bind(&filter.operator)
        .bind(&filter.value)
        .execute(&mut *transaction)
        .await
        .map_err(ApiError::internal)?;
    }

    transaction.commit().await.map_err(ApiError::internal)
}

pub fn validate_rule(rule: &BackupRule) -> Result<(), ApiError> {
    if rule.id.trim().is_empty() || rule.id.len() > 128 {
        return Err(ApiError::bad_request(
            "Rule id must contain 1 to 128 characters.",
        ));
    }
    if !matches!(rule.rule_type.as_str(), "scheduled" | "cloud" | "rtl") {
        return Err(ApiError::bad_request("Unknown backup rule type."));
    }
    if rule.name.trim().is_empty() || rule.name.chars().count() > MAX_NAME {
        return Err(ApiError::bad_request(
            "The name of the rule must contain 1 to 80 characters.",
        ));
    }
    if rule.created_at.trim().is_empty() || rule.created_at.len() > 64 {
        return Err(ApiError::bad_request("Rule creation date is invalid."));
    }
    if rule.same_destination && rule.rule_type != "rtl" {
        return Err(ApiError::bad_request(
            "Only a real-time backup writes into its own origin.",
        ));
    }
    validate_folders(rule)?;
    validate_schedule(&rule.rule_type, rule.schedule.as_ref())?;
    validate_operations(rule)?;
    if rule.rule_type != "cloud" && rule.duplicate_resolution.is_none() {
        return Err(ApiError::bad_request(
            "Choose what to do with a file the destination holds already.",
        ));
    }
    for filter in &rule.filters {
        filters::validate(filter).map_err(ApiError::bad_request)?;
    }
    Ok(())
}

/// A real-time rule runs on every change and has no schedule; the others cannot run without one.
pub fn validate_schedule(rule_type: &str, schedule: Option<&RuleSchedule>) -> Result<(), ApiError> {
    match (rule_type, schedule) {
        ("rtl", Some(_)) => Err(ApiError::bad_request("A real-time backup has no schedule.")),
        ("rtl", None) => Ok(()),
        (_, None) => Err(ApiError::bad_request("The schedule is required.")),
        (_, Some(schedule)) => schedule.validate().map_err(ApiError::bad_request),
    }
}

/// Each operation once, and each selected operation with its own options and no other options.
fn validate_operations(rule: &BackupRule) -> Result<(), ApiError> {
    for (index, operation) in rule.operations.iter().enumerate() {
        if rule.operations[..index].contains(operation) {
            return Err(ApiError::bad_request("Duplicate file operation."));
        }
    }
    if rule.operations.contains(&FileOperation::Encrypt) {
        return Err(ApiError::bad_request("Encryption is not available yet."));
    }
    let configurations = &rule.operation_configurations;
    for (operation, configured, name) in [
        (
            FileOperation::BatchRename,
            configurations.batch_rename.is_some(),
            "Batch rename",
        ),
        (
            FileOperation::Archive,
            configurations.archive.is_some(),
            "Archive",
        ),
        (
            FileOperation::Encrypt,
            configurations.encrypt.is_some(),
            "Encryption",
        ),
    ] {
        if configured != rule.operations.contains(&operation) {
            return Err(ApiError::bad_request(format!(
                "{name} options require the operation to be selected, and the reverse."
            )));
        }
    }
    if rule.same_destination && rule.operations.is_empty() {
        return Err(ApiError::bad_request(
            "A backup into its own origin needs a rename or an archive.",
        ));
    }
    validate_operation_options(configurations)
}

fn validate_operation_options(
    configurations: &FileOperationConfigurations,
) -> Result<(), ApiError> {
    if let Some(rename) = &configurations.batch_rename {
        let text = rename.prefix.trim();
        if rename.filename.trim().is_empty()
            || rename.filename.len() > 255
            || text.is_empty()
            || text.len() > 255
            || text.contains(['/', '\0'])
            || !matches!(rename.separator.as_str(), "-" | "_")
            || !matches!(rename.position.as_str(), "prefix" | "suffix")
        {
            return Err(ApiError::bad_request("Batch rename options are invalid."));
        }
    }
    if let Some(archive) = &configurations.archive {
        let name = archive.archive_name.trim();
        if name.is_empty()
            || name.len() > 200
            || name.starts_with('.')
            || name.contains(['/', '\0', '\\'])
            || archive.password.len() > 4096
        {
            return Err(ApiError::bad_request("Archive options are invalid."));
        }
    }
    if let Some(encrypt) = &configurations.encrypt {
        if !matches!(encrypt.mode.as_str(), "encrypt" | "decrypt")
            || !matches!(encrypt.output.as_str(), "single" | "folder" | "singlefile")
            || encrypt.password.len() > 4096
        {
            return Err(ApiError::bad_request("Encryption options are invalid."));
        }
    }
    Ok(())
}

/// Each type needs its own folders and no other: an absolute path without `..` or a NUL byte.
/// Origin and destination are never one inside the other, and a mirror never takes a whole disk.
pub fn validate_folders(rule: &BackupRule) -> Result<(), ApiError> {
    let folders = &rule.folders;
    let required: &[&str] = match (rule.rule_type.as_str(), rule.same_destination) {
        ("scheduled", _) | ("rtl", false) => &["origin", "destination"],
        ("rtl", true) => &["origin"],
        _ => &["cloud"],
    };
    for (kind, path) in [
        ("origin", &folders.origin),
        ("destination", &folders.destination),
        ("cloud", &folders.cloud),
    ] {
        match path {
            Some(_) if !required.contains(&kind) => {
                return Err(ApiError::bad_request(format!(
                    "This backup type has no {kind} folder."
                )))
            }
            Some(path) if !is_folder_path(path) => {
                return Err(ApiError::bad_request(format!(
                    "The {kind} folder is not a valid path."
                )))
            }
            None if required.contains(&kind) => {
                return Err(ApiError::bad_request(format!(
                    "The {kind} folder is required."
                )))
            }
            _ => {}
        }
    }
    if let (Some(origin), Some(destination)) = (&folders.origin, &folders.destination) {
        let (origin, destination) = (Path::new(origin), Path::new(destination));
        if destination.starts_with(origin) || origin.starts_with(destination) {
            return Err(ApiError::bad_request(
                "Origin and destination cannot be one inside the other.",
            ));
        }
        let depth = destination
            .components()
            .filter(|component| matches!(component, Component::Normal(_)))
            .count();
        if rule.sync_mode == SyncMode::Mirror && depth < 2 {
            return Err(ApiError::bad_request(
                "A mirror cannot use a whole disk as its destination: choose a folder in it.",
            ));
        }
    }
    Ok(())
}

fn is_folder_path(path: &str) -> bool {
    path.starts_with('/')
        && path.len() <= 4096
        && !path.contains('\0')
        && Path::new(path)
            .components()
            .all(|component| !matches!(component, Component::ParentDir))
}

fn encrypt_operation_data(rule: &BackupRule, cipher: &Aes256Gcm) -> Result<String, ApiError> {
    let mut data = StoredOperationData {
        operations: rule.operations.clone(),
        operation_configurations: rule.operation_configurations.clone(),
    };
    if let Some(archive) = &mut data.operation_configurations.archive {
        archive.password = encrypt_secret(&archive.password, cipher)?;
    }
    if let Some(encrypt) = &mut data.operation_configurations.encrypt {
        encrypt.password = encrypt_secret(&encrypt.password, cipher)?;
    }
    serde_json::to_string(&data).map_err(ApiError::internal)
}

fn decrypt_operation_data(
    serialized: &str,
    cipher: &Aes256Gcm,
) -> Result<StoredOperationData, ApiError> {
    let mut data: StoredOperationData =
        serde_json::from_str(serialized).map_err(ApiError::internal)?;
    if let Some(archive) = &mut data.operation_configurations.archive {
        archive.password = decrypt_secret(&archive.password, cipher)?;
    }
    if let Some(encrypt) = &mut data.operation_configurations.encrypt {
        encrypt.password = decrypt_secret(&encrypt.password, cipher)?;
    }
    Ok(data)
}

#[cfg(test)]
#[path = "../../tests/unit/axum/rules.rs"]
pub(crate) mod tests;
