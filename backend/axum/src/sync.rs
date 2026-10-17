//! Copies the files a rule takes from its origin to its destination, one folder at a time, and
//! keeps the list of what it copied (`rule_files`): a file unchanged since its copy (same size, same
//! time) is not read again, and a mirror removes from the destination only a copy listed there
//! whose original is gone from the origin. A file of the destination the rule did not copy is
//! never removed; one in the way of a copy is overwritten, left alone or kept beside a numbered
//! copy, as the rule says. A file left out by the filters is not copied, and its copy is not
//! removed either: a filter like "updated after yesterday" would otherwise empty the backup.
//!
//! Every copy is written under a hidden temporary name and renamed into place once complete and,
//! when asked, compared byte by byte with its original: a run stopped halfway leaves no half file.
//! Each copy is written down as soon as it is in place, and a file of the destination identical
//! to its original (size and time) is taken as its copy: a run stopped by a crash copies nothing
//! twice and makes no numbered duplicate. The work on the disks runs on the blocking threads,
//! never on the two workers of the server.
//!
//! What a mirror removes, and a copy that a newer one replaces, goes to the trash of the disk of the
//! destination ([`Trash`]). An automatic run that would remove more than `removable` copies stops
//! before it: a burst of deletions in the origin is a mistake or damage until a person runs the
//! rule by hand.
//!
//! In place (a real-time rule that writes into its own origin) the rule renames the files it
//! takes, and a file that already carries the added text is left as it is.

use crate::{
    filters::{Facts, Filters},
    rename,
    rules::{BatchRenameConfiguration, DuplicateResolution},
    storage,
    trash::Trash,
};
use sqlx::{Row, SqlitePool};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self, Read},
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

/// The names the rule writes while it works, which it never takes as files of the origin.
pub const TEMPORARY: &str = ".nas-backup-";
/// The trash of a disk and the file that says it is off (nas-file-manager): never backed up.
pub const TRASH: &str = ".nas-trash";
/// A copy is read and written in pieces of this size, so a stop is heard within one of them.
const CHUNK: u64 = 64 * 1024 * 1024;
const MAX_ERRORS: usize = 20;

/// What a run did so far: read by the launcher's operation while it goes.
#[derive(Debug, Default)]
pub struct Progress {
    pub copied: AtomicU64,
    pub removed: AtomicU64,
    errors: Mutex<Vec<String>>,
}

impl Progress {
    pub fn fail(&self, error: String) {
        eprintln!("<4>{error}");
        if let Ok(mut errors) = self.errors.lock() {
            if errors.len() < MAX_ERRORS {
                errors.push(error);
            }
        }
    }

    pub fn errors(&self) -> Vec<String> {
        self.errors
            .lock()
            .map(|errors| errors.clone())
            .unwrap_or_default()
    }
}

/// Why a run stopped before its end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stop {
    Interrupted,
    Failed(String),
}

impl From<sqlx::Error> for Stop {
    fn from(error: sqlx::Error) -> Self {
        Self::Failed(format!(
            "the list of the copied files is not available: {error}"
        ))
    }
}

/// One rule at work: where it reads, where it writes and how.
pub struct Sync<'a> {
    pub db: &'a SqlitePool,
    pub rule_id: &'a str,
    pub origin: &'a Path,
    pub destination: &'a Path,
    pub filters: &'a Filters,
    pub rename: Option<&'a BatchRenameConfiguration>,
    pub mirror: bool,
    pub resolution: DuplicateResolution,
    pub verify: bool,
    pub cancel: &'a Arc<AtomicBool>,
    pub progress: &'a Progress,
    /// The trash of the disk of the destination; none on a network disk, where a copy is removed.
    pub trash: Option<&'a Trash>,
    /// How many copies the run may remove: past it, it stops before removing any more.
    pub removable: u64,
}

/// What a run does with a file of the origin.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Plan {
    /// Its copy is up to date, or the rule leaves it alone.
    Skip,
    /// The file already at this path of the destination is its copy: only written down.
    Adopt(String),
    /// Copied to this path of the destination.
    Copy(String),
}

/// An entry of a folder of the origin.
#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    /// `None` for a folder
    pub file: Option<Facts>,
    /// Nanoseconds since the epoch, what the list compares
    pub modified: i64,
}

#[derive(Debug, Clone)]
struct Record {
    destination: String,
    size: i64,
    modified: i64,
}

/// The files and folders of a folder, by name; links, the rule's own temporary files and the trash
/// of the disk are left out.
pub fn list(folder: &Path) -> io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(TEMPORARY) || name.starts_with(TRASH) {
            continue;
        }
        let kind = entry.file_type()?;
        if kind.is_dir() {
            entries.push(Entry {
                name,
                file: None,
                modified: 0,
            });
        } else if kind.is_file() {
            let metadata = entry.metadata()?;
            entries.push(Entry {
                name,
                file: Some(Facts::of(&metadata)),
                modified: nanoseconds(metadata.modified().ok()),
            });
        }
    }
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(entries)
}

fn nanoseconds(time: Option<SystemTime>) -> i64 {
    time.and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| {
            duration.as_nanos().min(i64::MAX as u128) as i64
        })
}

/// `parent/name`, or `name` at the top.
pub fn join(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    }
}

/// The folder of a relative path, `''` at the top.
fn parent_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

/// The first free name beside a taken one: `report (1).pdf`, `report (2).pdf`...
pub fn free_name(folder: &Path, name: &str) -> String {
    let (stem, extension) = match name.rfind('.') {
        Some(dot) if dot > 0 => name.split_at(dot),
        _ => (name, ""),
    };
    (1..)
        .map(|number| format!("{stem} ({number}){extension}"))
        .find(|candidate| !folder.join(candidate).exists())
        .unwrap_or_else(|| name.to_owned())
}

/// Writes the original into a temporary file, keeping its permissions and time.
fn write_temporary(source: &Path, temporary: &Path, cancel: &AtomicBool) -> Result<(), Stop> {
    let failed = |what: &str, error: io::Error| {
        Stop::Failed(format!("{} {what}: {error}", source.display()))
    };
    let mut input = File::open(source).map_err(|error| failed("cannot be read", error))?;
    let metadata = input
        .metadata()
        .map_err(|error| failed("cannot be read", error))?;
    let mut output = File::create(temporary).map_err(|error| failed("cannot be written", error))?;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Stop::Interrupted);
        }
        let copied = io::copy(&mut (&mut input).take(CHUNK), &mut output)
            .map_err(|error| failed("cannot be copied", error))?;
        if copied < CHUNK {
            break;
        }
    }
    output
        .set_permissions(metadata.permissions())
        .map_err(|error| failed("cannot keep its permissions", error))?;
    if let Ok(modified) = metadata.modified() {
        output
            .set_modified(modified)
            .map_err(|error| failed("cannot keep its time", error))?;
    }
    output
        .sync_all()
        .map_err(|error| failed("cannot be written", error))?;
    Ok(())
}

/// Copies a file into place: a temporary file beside the target, the time and the permissions
/// of the original, a comparison when asked, then a rename over the target.
pub fn copy_file(
    source: &Path,
    target: &Path,
    verify: bool,
    cancel: &AtomicBool,
    trash: Option<&Trash>,
) -> Result<(), Stop> {
    let failed = |what: &str, error: io::Error| {
        Stop::Failed(format!("{} {what}: {error}", source.display()))
    };
    let folder = target
        .parent()
        .ok_or_else(|| Stop::Failed(format!("{} has no folder", target.display())))?;
    fs::create_dir_all(folder).map_err(|error| failed("cannot get its folder", error))?;
    let temporary = folder.join(format!(
        "{TEMPORARY}{}-{}.tmp",
        std::process::id(),
        nanoseconds(Some(SystemTime::now()))
    ));
    let outcome = (|| {
        write_temporary(source, &temporary, cancel)?;
        if verify && !same_bytes(source, &temporary, cancel)? {
            return Err(Stop::Failed(format!(
                "{} differs from its copy",
                source.display()
            )));
        }
        // The copy it replaces goes to the trash, not away.
        if let Some(trash) = trash {
            if fs::symlink_metadata(target).is_ok() {
                trash
                    .discard(target)
                    .map_err(|error| failed("cannot keep the copy it replaces", error))?;
            }
        }
        fs::rename(&temporary, target).map_err(|error| failed("cannot be put in place", error))?;
        // The rename lasts through a power cut only once the folder is written too.
        if let Ok(directory) = File::open(folder) {
            let _ = directory.sync_all();
        }
        Ok(())
    })();
    if outcome.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    outcome
}

/// Whether the file at `path` is the copy of an original of that size and time.
fn same_file(path: &Path, facts: &Facts, modified: i64) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        metadata.is_file()
            && metadata.len() == facts.size
            && nanoseconds(metadata.modified().ok()) == modified
    })
}

/// Whether two files hold the same bytes.
fn same_bytes(left: &Path, right: &Path, cancel: &AtomicBool) -> Result<bool, Stop> {
    let open = |path: &Path| {
        File::open(path).map_err(|error| Stop::Failed(format!("{}: {error}", path.display())))
    };
    let (mut left, mut right) = (open(left)?, open(right)?);
    let (mut a, mut b) = (vec![0; 1 << 20], vec![0; 1 << 20]);
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Stop::Interrupted);
        }
        let read = |file: &mut File, buffer: &mut [u8]| -> Result<usize, Stop> {
            let mut filled = 0;
            while filled < buffer.len() {
                match file.read(&mut buffer[filled..]) {
                    Ok(0) => break,
                    Ok(read) => filled += read,
                    Err(error) => return Err(Stop::Failed(format!("comparison: {error}"))),
                }
            }
            Ok(filled)
        };
        let (got_a, got_b) = (read(&mut left, &mut a)?, read(&mut right, &mut b)?);
        if got_a != got_b || a[..got_a] != b[..got_b] {
            return Ok(false);
        }
        if got_a == 0 {
            return Ok(true);
        }
    }
}

/// Removes a copy, to the trash when there is one, then the folders it leaves empty up to the
/// destination.
fn remove_copy(destination: &Path, relative: &str, trash: Option<&Trash>) -> io::Result<()> {
    let path = destination.join(relative);
    let removed = match trash {
        Some(trash) if fs::symlink_metadata(&path).is_ok() => trash.discard(&path),
        _ => fs::remove_file(&path),
    };
    match removed {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let mut folder = path.parent();
    while let Some(current) = folder {
        if current == destination || !current.starts_with(destination) {
            break;
        }
        if fs::remove_dir(current).is_err() {
            break;
        }
        folder = current.parent();
    }
    Ok(())
}

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Result<T, Stop> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| Stop::Failed(format!("the work on the disk stopped: {error}")))
}

impl Sync<'_> {
    fn interrupted(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    async fn records(&self, parent: &str) -> Result<HashMap<String, Record>, Stop> {
        let rows = sqlx::query(
            "SELECT name, destination, size, modified FROM rule_files
             WHERE rule_id = ? AND parent = ?",
        )
        .bind(self.rule_id)
        .bind(parent)
        .fetch_all(self.db)
        .await?;
        let mut records = HashMap::with_capacity(rows.len());
        for row in rows {
            records.insert(
                row.try_get("name")?,
                Record {
                    destination: row.try_get("destination")?,
                    size: row.try_get("size")?,
                    modified: row.try_get("modified")?,
                },
            );
        }
        Ok(records)
    }

    /// Runs the whole origin or only the trees and folders affected by a real-time change.
    pub async fn run_parts(
        &self,
        in_place: bool,
        changed: Option<Vec<String>>,
    ) -> Result<(), Stop> {
        let parts = changed.and_then(|changed| affected(self.origin, &changed));
        match (in_place, parts) {
            (true, None) => self.rename_in_place("", true).await,
            (true, Some((trees, folders))) => {
                for tree in trees {
                    self.rename_in_place(&tree, true).await?;
                }
                for folder in folders {
                    self.rename_in_place(&folder, false).await?;
                }
                Ok(())
            }
            (false, None) => self.tree("").await,
            (false, Some((trees, folders))) => {
                for tree in trees {
                    self.tree(&tree).await?;
                }
                for folder in folders {
                    self.folder(&folder).await?;
                }
                Ok(())
            }
        }
    }

    /// The whole origin, or the folder `root` of it with everything under it.
    pub async fn tree(&self, root: &str) -> Result<(), Stop> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut unreadable: Vec<String> = Vec::new();
        let mut pending = vec![root.to_owned()];
        while let Some(folder) = pending.pop() {
            if self.interrupted() {
                return Err(Stop::Interrupted);
            }
            let path = self.origin.join(&folder);
            let entries = match blocking(move || list(&path)).await? {
                Ok(entries) => entries,
                Err(error) => {
                    self.progress.fail(format!(
                        "{} cannot be read: {error}",
                        self.origin.join(&folder).display()
                    ));
                    unreadable.push(folder);
                    continue;
                }
            };
            for entry in entries.iter().filter(|entry| entry.file.is_none()) {
                pending.push(join(&folder, &entry.name));
            }
            self.files(&folder, &entries).await?;
            visited.insert(folder);
        }
        if self.mirror {
            self.sweep(root, &visited, &unreadable).await?;
        }
        Ok(())
    }

    /// One folder of the origin, and the folders under it that are gone.
    pub async fn folder(&self, folder: &str) -> Result<(), Stop> {
        let path = self.origin.join(folder);
        let entries = match blocking(move || list(&path)).await? {
            Ok(entries) => entries,
            // Gone: its parent sees to it.
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(Stop::Failed(format!(
                    "{} cannot be read: {error}",
                    self.origin.join(folder).display()
                )))
            }
        };
        self.files(folder, &entries).await?;
        if self.mirror {
            let present: HashSet<String> = entries
                .iter()
                .filter(|entry| entry.file.is_none())
                .map(|entry| join(folder, &entry.name))
                .collect();
            let mut visited = HashSet::from([folder.to_owned()]);
            // Every folder under one still there counts as seen: only those whose top is gone go.
            for parent in self.parents_under(folder).await? {
                let top = top_under(folder, &parent);
                if present.contains(&top) {
                    visited.insert(parent);
                }
            }
            self.sweep(folder, &visited, &[]).await?;
        }
        Ok(())
    }

    /// The folders with copies under `root`, read through the index: `root/` up to `root0`, the
    /// character after the slash, holds every folder under it and nothing else.
    async fn parents_under(&self, root: &str) -> Result<Vec<String>, Stop> {
        let parents: Vec<String> = if root.is_empty() {
            sqlx::query_scalar("SELECT DISTINCT parent FROM rule_files WHERE rule_id = ?")
                .bind(self.rule_id)
                .fetch_all(self.db)
                .await?
        } else {
            sqlx::query_scalar(
                "SELECT DISTINCT parent FROM rule_files
                 WHERE rule_id = ? AND (parent = ? OR (parent >= ? AND parent < ?))",
            )
            .bind(self.rule_id)
            .bind(root)
            .bind(format!("{root}/"))
            .bind(format!("{root}0"))
            .fetch_all(self.db)
            .await?
        };
        Ok(parents
            .into_iter()
            .filter(|parent| under(root, parent))
            .collect())
    }

    /// Removes the copies listed in folders under `root` that were not seen, but never under one
    /// that could not be read.
    async fn sweep(
        &self,
        root: &str,
        visited: &HashSet<String>,
        unreadable: &[String],
    ) -> Result<(), Stop> {
        for parent in self.parents_under(root).await? {
            if visited.contains(&parent) || unreadable.iter().any(|folder| under(folder, &parent)) {
                continue;
            }
            let records = self.records(&parent).await?;
            let names: Vec<String> = records.keys().cloned().collect();
            self.remove(&parent, &names, &records).await?;
        }
        Ok(())
    }

    async fn remove(
        &self,
        parent: &str,
        names: &[String],
        records: &HashMap<String, Record>,
    ) -> Result<(), Stop> {
        let removed = self.progress.removed.load(Ordering::Relaxed);
        if removed.saturating_add(names.len() as u64) > self.removable {
            return Err(Stop::Failed(format!(
                "more than {} files are gone from the origin: nothing more was removed from the \
                 backup. Check the origin, then run the rule by hand to remove them",
                self.removable
            )));
        }
        let mut gone = Vec::new();
        for name in names {
            let Some(record) = records.get(name) else {
                continue;
            };
            let destination = self.destination.to_path_buf();
            let relative = record.destination.clone();
            let trash = self.trash.cloned();
            match blocking(move || remove_copy(&destination, &relative, trash.as_ref())).await? {
                Ok(()) => {
                    self.progress.removed.fetch_add(1, Ordering::Relaxed);
                    gone.push(name.clone());
                }
                Err(error) => self.progress.fail(format!(
                    "{} cannot be removed: {error}",
                    self.destination.join(&record.destination).display()
                )),
            }
        }
        let mut transaction = self.db.begin().await?;
        for name in gone {
            sqlx::query("DELETE FROM rule_files WHERE rule_id = ? AND parent = ? AND name = ?")
                .bind(self.rule_id)
                .bind(parent)
                .bind(name)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    /// Keeps an unchanged copy, takes an identical file as the copy, updates a mirror's own copy
    /// or resolves a conflicting name.
    fn plan(&self, folder: &str, entry: &Entry, facts: &Facts, record: Option<&Record>) -> Plan {
        if record.is_some_and(|record| {
            record.size == facts.size as i64
                && record.modified == entry.modified
                && self.destination.join(&record.destination).exists()
        }) {
            return Plan::Skip;
        }
        let name = self.rename.map_or_else(
            || entry.name.clone(),
            |rename| rename::renamed(&entry.name, rename),
        );
        let wanted = join(folder, &name);
        // A copy made before a crash, or by hand: the same size and time, nothing to copy again.
        if record.is_none() && same_file(&self.destination.join(&wanted), facts, entry.modified) {
            return Plan::Adopt(wanted);
        }
        // A mirror updates its own copy where it is; any other conflict follows the rule.
        match record {
            Some(record) if self.mirror => Plan::Copy(record.destination.clone()),
            _ if self.destination.join(&wanted).exists() => match self.resolution {
                DuplicateResolution::Overwrite => Plan::Copy(wanted),
                DuplicateResolution::Ignore => Plan::Skip,
                DuplicateResolution::Copy => Plan::Copy(join(
                    folder,
                    &free_name(&self.destination.join(folder), &name),
                )),
            },
            _ => Plan::Copy(wanted),
        }
    }

    /// Writes down the copy of a file, at once: a crash after it loses nothing.
    async fn note(
        &self,
        folder: &str,
        name: &str,
        destination: &str,
        size: i64,
        modified: i64,
    ) -> Result<(), Stop> {
        sqlx::query(
            "INSERT INTO rule_files (rule_id, parent, name, destination, size, modified)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT (rule_id, parent, name) DO UPDATE SET
                destination = excluded.destination, size = excluded.size,
                modified = excluded.modified",
        )
        .bind(self.rule_id)
        .bind(folder)
        .bind(name)
        .bind(destination)
        .bind(size)
        .bind(modified)
        .execute(self.db)
        .await?;
        Ok(())
    }

    /// The files of one folder: copied when new or changed, their copies removed when gone.
    async fn files(&self, folder: &str, entries: &[Entry]) -> Result<(), Stop> {
        let records = self.records(folder).await?;
        for entry in entries {
            if self.interrupted() {
                break;
            }
            let Some(facts) = entry.file else {
                continue;
            };
            if !self.filters.admits(&entry.name, &facts) {
                continue;
            }
            let relative = match self.plan(folder, entry, &facts, records.get(&entry.name)) {
                Plan::Skip => continue,
                Plan::Adopt(relative) => {
                    self.note(
                        folder,
                        &entry.name,
                        &relative,
                        facts.size as i64,
                        entry.modified,
                    )
                    .await?;
                    continue;
                }
                Plan::Copy(relative) => relative,
            };
            let (source, target) = (
                self.origin.join(join(folder, &entry.name)),
                self.destination.join(&relative),
            );
            let (verify, cancel, trash) =
                (self.verify, Arc::clone(self.cancel), self.trash.cloned());
            match blocking(move || copy_file(&source, &target, verify, &cancel, trash.as_ref()))
                .await?
            {
                Ok(()) => {
                    self.progress.copied.fetch_add(1, Ordering::Relaxed);
                    self.note(
                        folder,
                        &entry.name,
                        &relative,
                        facts.size as i64,
                        entry.modified,
                    )
                    .await?;
                }
                Err(Stop::Interrupted) => break,
                Err(Stop::Failed(error)) => self.progress.fail(error),
            }
        }
        if self.interrupted() {
            return Err(Stop::Interrupted);
        }
        if self.mirror {
            self.remove_missing(folder, entries, &records).await?;
        }
        Ok(())
    }

    async fn remove_missing(
        &self,
        folder: &str,
        entries: &[Entry],
        records: &HashMap<String, Record>,
    ) -> Result<(), Stop> {
        let present: HashSet<&str> = entries
            .iter()
            .filter(|entry| entry.file.is_some())
            .map(|entry| entry.name.as_str())
            .collect();
        let gone: Vec<String> = records
            .keys()
            .filter(|name| !present.contains(name.as_str()))
            .cloned()
            .collect();
        if !gone.is_empty() {
            self.remove(folder, &gone, records).await?;
        }
        Ok(())
    }

    async fn rename_file(
        &self,
        folder: &str,
        entry: &Entry,
        facts: &Facts,
        rename: &BatchRenameConfiguration,
    ) -> Result<(), Stop> {
        if rename::carries(&entry.name, rename) || !self.filters.admits(&entry.name, facts) {
            return Ok(());
        }
        let directory = self.origin.join(folder);
        let mut name = rename::renamed(&entry.name, rename);
        if directory.join(&name).exists() {
            match self.resolution {
                DuplicateResolution::Overwrite => {}
                DuplicateResolution::Ignore => return Ok(()),
                DuplicateResolution::Copy => name = free_name(&directory, &name),
            }
        }
        let (from, to) = (directory.join(&entry.name), directory.join(&name));
        match blocking(move || fs::rename(&from, &to)).await? {
            Ok(()) => {
                self.progress.copied.fetch_add(1, Ordering::Relaxed);
            }
            Err(error) => self.progress.fail(format!(
                "{} cannot be renamed: {error}",
                directory.join(&entry.name).display()
            )),
        }
        Ok(())
    }

    /// Renames in place the files of the origin the rule takes, under `root`.
    pub async fn rename_in_place(&self, root: &str, recursive: bool) -> Result<(), Stop> {
        let rename = self
            .rename
            .ok_or_else(|| Stop::Failed("there is nothing to rename".to_owned()))?;
        let mut pending = vec![root.to_owned()];
        while let Some(folder) = pending.pop() {
            if self.interrupted() {
                return Err(Stop::Interrupted);
            }
            let path = self.origin.join(&folder);
            let entries = match blocking(move || list(&path)).await? {
                Ok(entries) => entries,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => {
                    self.progress.fail(format!(
                        "{} cannot be read: {error}",
                        self.origin.join(&folder).display()
                    ));
                    continue;
                }
            };
            for entry in &entries {
                let Some(facts) = entry.file else {
                    if recursive {
                        pending.push(join(&folder, &entry.name));
                    }
                    continue;
                };
                self.rename_file(&folder, entry, &facts, rename).await?;
            }
        }
        Ok(())
    }
}

/// Whether a relative folder is `root` or under it.
pub fn under(root: &str, folder: &str) -> bool {
    root.is_empty() || folder == root || folder.starts_with(&format!("{root}/"))
}

/// The folder right under `root` that holds `folder`: `a/b` under `a` for `a/b/c`.
fn top_under(root: &str, folder: &str) -> String {
    let rest = if root.is_empty() {
        folder
    } else {
        folder.strip_prefix(&format!("{root}/")).unwrap_or(folder)
    };
    let first = rest.split('/').next().unwrap_or(rest);
    join(root, first)
}

/// The folders to look at again for a set of changed paths of the origin, relative to it: a
/// folder that changed is looked at whole, a file through its folder. `None` when the whole
/// origin has to be read again.
pub fn affected(origin: &Path, changed: &[String]) -> Option<(Vec<String>, Vec<String>)> {
    let mut trees: Vec<String> = Vec::new();
    let mut folders: Vec<String> = Vec::new();
    for path in changed {
        if path.is_empty() {
            return None;
        }
        if origin.join(path).is_dir() {
            trees.push(path.clone());
        } else {
            folders.push(parent_of(path).to_owned());
        }
    }
    trees.sort();
    trees.dedup();
    // A tree already under another is read with it.
    let mut kept: Vec<String> = Vec::new();
    for tree in trees {
        if !kept.iter().any(|outer| under(outer, &tree)) {
            kept.push(tree);
        }
    }
    folders.sort();
    folders.dedup();
    folders.retain(|folder| !kept.iter().any(|tree| under(tree, folder)));
    Some((kept, folders))
}

/// A path relative to a folder of the rule, from an absolute one inside it.
pub fn relative_to(folder: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(folder).ok().map(storage::relative)
}

#[cfg(test)]
#[path = "../../tests/unit/axum/sync.rs"]
mod tests;
