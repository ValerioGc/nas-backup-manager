//! Runs the rules, one run per rule and at most two at a time, shown among the launcher operations.
//! It also tracks for the list whether each rule runs and when it runs next.

use crate::{
    files::{
        archive::{self, Archive},
        storage::{self, Storage},
        sync::{Progress, Stop, Sync},
        trash::Trash,
    },
    launcher::client::Launcher,
    rules::{
        self,
        filters::{self, Clock},
        DuplicateResolution, FileOperation, StoredRule, SyncMode,
    },
    runs::scheduler,
    ApiError,
};
use aes_gcm::Aes256Gcm;
use serde_json::json;
use sqlx::SqlitePool;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{Notify, Semaphore};

const RUNS_AT_ONCE: usize = 2;
/// Interval of progress reports and launcher stop checks.
const REPORT: Duration = Duration::from_secs(3);
/// Time given to runs to stop when the app shuts down.
const STOP_WAIT: Duration = Duration::from_secs(20);
const MAX_MESSAGE: usize = 1000;

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as i64)
}

/// Disk source, the launcher registry or a fixed one in tests.
#[derive(Debug, Clone)]
pub enum Disks {
    Granted,
    #[cfg(test)]
    Fixed(Storage),
}

impl Disks {
    fn storage(&self) -> Result<Storage, String> {
        match self {
            Self::Granted => Storage::from_env(),
            #[cfg(test)]
            Self::Fixed(storage) => Ok(storage.clone()),
        }
    }
}

pub struct Runner {
    database: SqlitePool,
    cipher: Arc<Aes256Gcm>,
    launcher: Launcher,
    disks: Disks,
    /// App state directory, home of the archive symlink folders.
    work: PathBuf,
    seven_zip: String,
    active: Mutex<HashMap<String, Arc<AtomicBool>>>,
    permits: Semaphore,
    next: Mutex<HashMap<String, i64>>,
    schedule: Notify,
    watch: Notify,
}

impl Runner {
    pub fn new(
        database: SqlitePool,
        cipher: Arc<Aes256Gcm>,
        launcher: Launcher,
        disks: Disks,
        work: PathBuf,
    ) -> Self {
        Self {
            database,
            cipher,
            launcher,
            disks,
            work,
            seven_zip: std::env::var("NAS_BACKUP_7Z").unwrap_or_else(|_| "7z".to_owned()),
            active: Mutex::new(HashMap::new()),
            permits: Semaphore::new(RUNS_AT_ONCE),
            next: Mutex::new(HashMap::new()),
            schedule: Notify::new(),
            watch: Notify::new(),
        }
    }

    /// Reads the current disks, since the registry changes as disks come and go.
    pub fn storage(&self) -> Result<Storage, String> {
        self.disks.storage()
    }

    #[cfg(test)]
    pub fn with_seven_zip(mut self, program: &str) -> Self {
        program.clone_into(&mut self.seven_zip);
        self
    }

    pub fn database(&self) -> &SqlitePool {
        &self.database
    }

    pub fn launcher(&self) -> &Launcher {
        &self.launcher
    }

    pub fn cipher(&self) -> &Aes256Gcm {
        &self.cipher
    }

    pub fn is_running(&self, id: &str) -> bool {
        self.active
            .lock()
            .is_ok_and(|active| active.contains_key(id))
    }

    /// Stops a run, returning false when the rule is not running.
    pub fn stop(&self, id: &str) -> bool {
        self.active
            .lock()
            .ok()
            .and_then(|active| active.get(id).cloned())
            .map(|cancel| cancel.store(true, Ordering::Relaxed))
            .is_some()
    }

    pub fn next_run(&self, id: &str) -> Option<i64> {
        self.next.lock().ok()?.get(id).copied()
    }

    pub fn set_next(&self, id: &str, at: Option<i64>) {
        if let Ok(mut next) = self.next.lock() {
            match at {
                Some(at) => next.insert(id.to_owned(), at),
                None => next.remove(id),
            };
        }
    }

    /// Drops the next runs of every rule not in `ids`.
    pub fn keep_next(&self, ids: &[String]) {
        if let Ok(mut next) = self.next.lock() {
            next.retain(|id, _| ids.contains(id));
        }
    }

    pub fn forget_next(&self) {
        if let Ok(mut next) = self.next.lock() {
            next.clear();
        }
    }

    pub async fn schedule_changed(&self) {
        self.schedule.notified().await;
    }

    pub async fn watch_changed(&self) {
        self.watch.notified().await;
    }

    /// Propagates a rule change to its next run, the scheduler, the watcher and the launcher.
    pub async fn rules_changed(&self, id: Option<&str>) {
        if let Some(id) = id {
            scheduler::refresh(self, id).await;
        }
        self.schedule.notify_one();
        self.watch.notify_one();
        self.declare_background().await;
    }

    pub async fn declare_background(&self) {
        let needed: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM backup_rules
             WHERE enabled = 1 AND rule_type IN ('scheduled', 'rtl'))",
        )
        .fetch_one(&self.database)
        .await
        .unwrap_or(false);
        self.launcher.background(needed).await;
    }

    /// Starts a run on the whole origin or, for a real-time rule, on the changed paths. It returns
    /// at once while the run goes on in the background.
    pub async fn start(
        this: &Arc<Self>,
        id: &str,
        changed: Option<Vec<String>>,
        by_hand: bool,
    ) -> Result<tokio::task::JoinHandle<()>, ApiError> {
        let stored = rules::load(&this.database, &this.cipher, id)
            .await?
            .ok_or_else(|| ApiError::not_found("No such rule."))?;
        if stored.rule.rule_type == "cloud" {
            return Err(ApiError::bad_request("Cloud backups do not run yet."));
        }
        let cancel = {
            let mut active = this
                .active
                .lock()
                .map_err(|_| ApiError::internal("the list of the runs is not readable"))?;
            if active.contains_key(id) {
                return Err(ApiError::conflict("The rule is running already."));
            }
            let cancel = Arc::new(AtomicBool::new(false));
            active.insert(id.to_owned(), Arc::clone(&cancel));
            cancel
        };
        let runner = Arc::clone(this);
        Ok(tokio::spawn(async move {
            let id = stored.rule.id.clone();
            runner.run(stored, changed, by_hand, &cancel).await;
            if let Ok(mut active) = runner.active.lock() {
                active.remove(&id);
            }
        }))
    }

    /// Run started by the watcher. It waits for any manual run to end, then for its own.
    pub async fn run_and_wait(this: &Arc<Self>, id: &str, changed: Option<Vec<String>>) {
        while this.is_running(id) {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        match Self::start(this, id, changed, false).await {
            Ok(run) => {
                let _ = run.await;
            }
            Err(error) => eprintln!("<4>{id} did not run: {}", error.message),
        }
    }

    /// Stops every run and briefly waits for them to report the interruption.
    pub async fn shutdown(&self) {
        if let Ok(active) = self.active.lock() {
            for cancel in active.values() {
                cancel.store(true, Ordering::Relaxed);
            }
        }
        let deadline = tokio::time::Instant::now() + STOP_WAIT;
        while tokio::time::Instant::now() < deadline
            && self.active.lock().is_ok_and(|active| !active.is_empty())
        {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn run(
        &self,
        stored: StoredRule,
        changed: Option<Vec<String>>,
        by_hand: bool,
        cancel: &Arc<AtomicBool>,
    ) {
        let _permit = self.permits.acquire().await;
        let rule = &stored.rule;
        let task = format!(
            "nas-backup-manager-{}-{}",
            rule.id
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
                .take(64)
                .collect::<String>(),
            now_ms()
        );
        let shown = self
            .launcher
            .start_task(&task, &format!("Backup: {}", rule.name))
            .await;
        let progress = Arc::new(Progress::default());
        let reporter = shown.then(|| {
            let (launcher, task, progress, cancel) = (
                self.launcher.clone(),
                task.clone(),
                Arc::clone(&progress),
                Arc::clone(cancel),
            );
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(REPORT).await;
                    if launcher.cancelled(&task).await {
                        cancel.store(true, Ordering::Relaxed);
                    }
                    launcher
                        .update_task(&task, &json!({ "detail": detail(&progress) }))
                        .await;
                }
            })
        });

        let outcome = self
            .work(&stored, changed, by_hand, cancel, &progress)
            .await;
        if let Some(reporter) = reporter {
            reporter.abort();
        }
        let errors = progress.errors();
        let (status, message) = match outcome {
            Err(Stop::Interrupted) => ("interrupted", Some("Stopped before the end.".to_owned())),
            Err(Stop::Failed(reason)) => ("failed", Some(reason)),
            Ok(()) if !errors.is_empty() => (
                "failed",
                Some(format!(
                    "{} files were not copied or removed: {}",
                    errors.len(),
                    errors.join("; ")
                )),
            ),
            Ok(()) => ("ok", None),
        };
        let message = message.map(|message| message.chars().take(MAX_MESSAGE).collect::<String>());
        if let Some(reason) = &message {
            eprintln!("<3>backup {} ({}) {status}: {reason}", rule.name, rule.id);
        }
        if shown {
            let state = match status {
                "ok" => "done",
                "interrupted" => "cancelled",
                _ => "failed",
            };
            let detail = message.clone().unwrap_or_else(|| detail(&progress));
            self.launcher
                .update_task(
                    &task,
                    &json!({ "state": state, "detail": detail.chars().take(500).collect::<String>() }),
                )
                .await;
        }
        let saved = sqlx::query(
            "UPDATE backup_rules SET last_run_at = ?, last_run_status = ?, last_run_message = ?
             WHERE id = ?",
        )
        .bind(now_ms())
        .bind(status)
        .bind(&message)
        .bind(&rule.id)
        .execute(&self.database)
        .await;
        if let Err(error) = saved {
            eprintln!("<3>the outcome of {} was not saved: {error}", rule.id);
        }
    }

    async fn work(
        &self,
        stored: &StoredRule,
        changed: Option<Vec<String>>,
        by_hand: bool,
        cancel: &Arc<AtomicBool>,
        progress: &Progress,
    ) -> Result<(), Stop> {
        let rule = &stored.rule;
        if rule.operations.contains(&FileOperation::Encrypt) {
            return Err(Stop::Failed("encryption is not available yet".to_owned()));
        }
        let storage = self.storage().map_err(Stop::Failed)?;
        let origin_path = rule
            .folders
            .origin
            .as_deref()
            .ok_or_else(|| Stop::Failed("the rule has no origin".to_owned()))?;
        let origin = storage.resolve(origin_path, false).map_err(Stop::Failed)?;
        let destination = if rule.same_destination {
            origin.clone()
        } else {
            let path = rule
                .folders
                .destination
                .as_deref()
                .ok_or_else(|| Stop::Failed("the rule has no destination".to_owned()))?;
            storage.resolve(path, true).map_err(Stop::Failed)?
        };
        // Symlinks may make separate paths overlap.
        if !rule.same_destination
            && (storage::nested(&destination.path, &origin.path)
                || storage::nested(&origin.path, &destination.path))
        {
            return Err(Stop::Failed(
                "origin and destination are one inside the other on the disk".to_owned(),
            ));
        }
        let clock = Clock::now().await.map_err(Stop::Failed)?;
        let filters = filters::compile(&rule.filters, &clock).map_err(Stop::Failed)?;
        let configurations = &rule.operation_configurations;
        let rename = configurations.batch_rename.as_ref();
        let resolution = rule
            .duplicate_resolution
            .unwrap_or(DuplicateResolution::Ignore);
        let mirror = rule.sync_mode == SyncMode::Mirror && !rule.same_destination;
        self.network_ready(&rule.id, &origin, &destination, mirror)
            .await?;

        if let Some(options) = &configurations.archive {
            let stamp = stamp().await;
            let links = archive::links_of(&self.work, &rule.id);
            let made = Archive {
                seven_zip: &self.seven_zip,
                origin: &origin.path,
                destination: &destination.path,
                links: &links,
                filters: &filters,
                rename,
                options,
                mirror,
                resolution,
                verify: rule.verify_archived_files,
                previous: stored.archive_path.as_deref(),
                fingerprint: stored.archive_fingerprint.as_deref(),
                stamp: &stamp,
                cancel,
                copied: &progress.copied,
            }
            .run(rule.same_destination)
            .await;
            let _ = std::fs::remove_dir_all(&links);
            let made = made?;
            if !made.unchanged {
                sqlx::query(
                    "UPDATE backup_rules SET archive_path = ?, archive_fingerprint = ? WHERE id = ?",
                )
                .bind(&made.path)
                .bind(&made.fingerprint)
                .bind(&rule.id)
                .execute(&self.database)
                .await?;
            }
            return Ok(());
        }

        // Network disks have no NAS trash, so removed copies are deleted.
        let trash = (!destination.network).then(|| Trash::new(destination.volume.clone()));
        let removable = if by_hand {
            u64::MAX
        } else {
            removable(self.copies(&rule.id).await?)
        };
        let sync = Sync {
            db: &self.database,
            rule_id: &rule.id,
            origin: &origin.path,
            destination: &destination.path,
            filters: &filters,
            rename,
            mirror,
            resolution,
            verify: rule.verify_archived_files,
            cancel,
            progress,
            trash: trash.as_ref(),
            removable,
        };
        sync.run_parts(rule.same_destination, changed).await
    }

    async fn copies(&self, rule: &str) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar("SELECT COUNT(*) FROM rule_files WHERE rule_id = ?")
            .bind(rule)
            .fetch_one(&self.database)
            .await
    }

    /// A network origin that looks empty while a mirror holds copies from it is a lost share, not
    /// deleted files, so the run stops and leaves the backup untouched.
    async fn network_ready(
        &self,
        rule: &str,
        origin: &storage::Folder,
        destination: &storage::Folder,
        mirror: bool,
    ) -> Result<(), Stop> {
        if destination.network {
            storage::holds_files(&destination.volume)
                .await
                .map_err(Stop::Failed)?;
        }
        if !origin.network
            || storage::holds_files(&origin.path)
                .await
                .map_err(Stop::Failed)?
        {
            return Ok(());
        }
        if mirror && self.copies(rule).await? > 0 {
            return Err(Stop::Failed(
                "the network disk shows no file, so nothing was removed from the backup".to_owned(),
            ));
        }
        Ok(())
    }
}

const REMOVABLE_AT_LEAST: u64 = 100;

/// Copies an automatic run may remove, a quarter of the total and at least `REMOVABLE_AT_LEAST`.
pub fn removable(copies: i64) -> u64 {
    (copies.max(0) as u64 / 4).max(REMOVABLE_AT_LEAST)
}

fn detail(progress: &Progress) -> String {
    let copied = progress.copied.load(Ordering::Relaxed);
    let removed = progress.removed.load(Ordering::Relaxed);
    if removed == 0 {
        format!("{copied} file")
    } else {
        format!("{copied} file, {removed} rimossi")
    }
}

/// `YYYYMMDD-HHMM` in the NAS local time, for archive names.
async fn stamp() -> String {
    tokio::process::Command::new("date")
        .arg("+%Y%m%d-%H%M")
        .output()
        .await
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|stamp| stamp.len() == 13)
        .unwrap_or_else(|| (now_ms() / 1000).to_string())
}

#[cfg(test)]
#[path = "../../tests/unit/runs/runner.rs"]
mod tests;
