//! Runs real-time rules on the paths that changed, once the changes settle. Network disks and
//! origins inotify cannot watch are rescanned every ten minutes instead.

use crate::{
    files::{
        archive,
        sync::{self, TEMPORARY},
        trash::TRASH,
    },
    rules,
    runs::runner::Runner,
};
use notify::{
    event::{EventKind, Flag},
    Event, RecursiveMode, Watcher,
};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{sync::mpsc, task::JoinHandle, time::Instant};

#[cfg(not(test))]
const SETTLE: Duration = Duration::from_secs(5);
#[cfg(test)]
const SETTLE: Duration = Duration::from_millis(300);
const LONGEST: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_secs(10 * 60);
const RETRY: Duration = Duration::from_secs(60);

/// Maximum pending changes before the whole origin is read again.
const QUEUE: usize = 8192;

/// Real-time rule as the watcher follows it. Any change to it restarts the watch.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Watched {
    origin: String,
    own: Option<String>,
}

enum Change {
    Paths(Vec<String>),
    Everything,
}

async fn wanted(runner: &Runner) -> HashMap<String, Watched> {
    let Ok(rules) = rules::load_all(runner.database(), runner.cipher()).await else {
        return HashMap::new();
    };
    rules
        .into_iter()
        .map(|(stored, _)| stored.rule)
        .filter(|rule| rule.enabled && rule.rule_type == "rtl")
        .filter_map(|rule| {
            let own = rule
                .same_destination
                .then(|| {
                    rule.operation_configurations
                        .archive
                        .as_ref()
                        .map(|archive| archive.archive_name.trim().to_owned())
                })
                .flatten();
            Some((
                rule.id,
                Watched {
                    origin: rule.folders.origin?,
                    own,
                },
            ))
        })
        .collect()
}

pub async fn run(runner: Arc<Runner>) {
    let mut watching: HashMap<String, (Watched, JoinHandle<()>)> = HashMap::new();
    loop {
        let wanted = wanted(&runner).await;
        watching.retain(|id, (watched, task)| {
            let keep = wanted.get(id) == Some(watched);
            if !keep {
                task.abort();
            }
            keep
        });
        for (id, watched) in wanted {
            if let std::collections::hash_map::Entry::Vacant(place) = watching.entry(id) {
                let task = tokio::spawn(follow(
                    Arc::clone(&runner),
                    place.key().clone(),
                    watched.clone(),
                ));
                place.insert((watched, task));
            }
        }
        runner.watch_changed().await;
    }
}

/// Tells whether a changed path comes from the rule itself, its temporary files or its archive.
fn ignored(relative: &str, own: Option<&str>) -> bool {
    relative
        .split('/')
        .any(|part| part.starts_with(TEMPORARY) || part.starts_with(TRASH))
        || own.is_some_and(|archive| !relative.contains('/') && archive::is_own(relative, archive))
}

/// Waits until the origin of the rule is available, retrying while its disk is missing.
async fn wait_for_origin(runner: &Runner, id: &str, origin: &str) -> (PathBuf, bool) {
    loop {
        match runner
            .storage()
            .and_then(|storage| storage.resolve(origin, false))
        {
            Ok(folder) => break (folder.path, folder.network),
            Err(reason) => {
                eprintln!("<4>{id} waits for its origin: {reason}");
                tokio::time::sleep(RETRY).await;
            }
        }
    }
}

/// Watches one rule until it is disabled, changed or removed.
async fn follow(runner: Arc<Runner>, id: String, watched: Watched) {
    let (origin, network) = wait_for_origin(&runner, &id, &watched.origin).await;
    let (sender, mut changes) = mpsc::channel::<Change>(QUEUE);
    let overflowed = Arc::new(AtomicBool::new(false));
    let watcher = if network {
        Err("a network disk tells nothing of the changes of other computers".to_owned())
    } else {
        watch(
            &origin,
            watched.own.clone(),
            sender,
            Arc::clone(&overflowed),
        )
        .await
    };
    if let Err(reason) = &watcher {
        eprintln!("<4>{id} cannot watch {}: {reason}", origin.display());
    }
    // Full run first, for changes made while nobody watched.
    Runner::run_and_wait(&runner, &id, None).await;
    let Ok(_watcher) = watcher else {
        loop {
            tokio::time::sleep(POLL).await;
            Runner::run_and_wait(&runner, &id, None).await;
        }
    };
    while let Some(first) = changes.recv().await {
        let mut paths: HashSet<String> = HashSet::new();
        let mut everything = false;
        let mut take = |change: Change| match change {
            Change::Paths(changed) => paths.extend(changed),
            Change::Everything => everything = true,
        };
        take(first);
        let deadline = Instant::now() + LONGEST;
        loop {
            let settle = (Instant::now() + SETTLE).min(deadline);
            match tokio::time::timeout_at(settle, changes.recv()).await {
                Ok(Some(change)) => take(change),
                Ok(None) => return,
                Err(_) => break,
            }
        }
        everything |= overflowed.swap(false, Ordering::Relaxed);
        let changed = (!everything).then(|| paths.into_iter().collect());
        Runner::run_and_wait(&runner, &id, changed).await;
    }
}

/// Inotify watch of an origin, reporting changes as paths relative to it.
async fn watch(
    origin: &Path,
    own: Option<String>,
    sender: mpsc::Sender<Change>,
    overflowed: Arc<AtomicBool>,
) -> Result<notify::RecommendedWatcher, String> {
    let root = origin.to_path_buf();
    let handler = move |event: notify::Result<Event>| {
        let change = match event {
            Ok(event) if event.need_rescan() || event.flag() == Some(Flag::Rescan) => {
                Change::Everything
            }
            Ok(event) if matches!(event.kind, EventKind::Access(_)) => return,
            Ok(event) => {
                let paths: Vec<String> = event
                    .paths
                    .iter()
                    .filter_map(|path| sync::relative_to(&root, path))
                    .filter(|relative| !ignored(relative, own.as_deref()))
                    .collect();
                if paths.is_empty() {
                    return;
                }
                Change::Paths(paths)
            }
            Err(_) => Change::Everything,
        };
        // With a full queue the paths are lost and the next run reads the whole origin.
        if let Err(mpsc::error::TrySendError::Full(_)) = sender.try_send(change) {
            overflowed.store(true, Ordering::Relaxed);
        }
    };
    let path: PathBuf = origin.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut watcher =
            notify::recommended_watcher(handler).map_err(|error| error.to_string())?;
        watcher
            .watch(&path, RecursiveMode::Recursive)
            .map_err(|error| error.to_string())?;
        Ok(watcher)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
#[path = "../../tests/unit/runs/watcher.rs"]
mod tests;
