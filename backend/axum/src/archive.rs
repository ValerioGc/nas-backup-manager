//! The 7z archive of a rule (the `7zip` package of Debian): AES-256 with the password of the rule,
//! the names of the files encrypted too when asked, not solid so that it is updated in place.
//!
//! The files the filters take are not copied anywhere first: a folder of links in the state
//! directory of the application, with the names the rename gives them, stands for them, and 7z
//! stores what each link leads to. A mirror keeps one archive, `<name>.7z`, which `7z u` brings in
//! line with that folder (new and changed files in, removed ones out); a copy writes a new
//! `<name>-<date>-<time>.7z` at each run. When nothing changed since the last archive (the same
//! paths, sizes and times), nothing is written.
//!
//! The password goes to 7z on its input, never on its command line, where any user of the machine
//! could read it while it runs. A new archive is written under a temporary name and renamed into
//! place, and 7z updates an archive through a copy of its own: a stopped run leaves the last whole
//! archive where it was.

use crate::{
    filters::Filters,
    rename,
    rules::{ArchiveConfiguration, BatchRenameConfiguration, DuplicateResolution},
    sync::{self, free_name, join, Stop, TEMPORARY},
};
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    io,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{io::AsyncWriteExt, process::Command};

/// Keeps a mirror's archive in line with the folder it is given: adds what is new or changed,
/// removes what is gone.
const SYNC: &str = "-up0q0r2x2y2z1w2";

/// What a rule archives and where.
pub struct Archive<'a> {
    pub seven_zip: &'a str,
    pub origin: &'a Path,
    pub destination: &'a Path,
    /// The folder of links, in the state directory: emptied and filled at each run.
    pub links: &'a Path,
    pub filters: &'a Filters,
    pub rename: Option<&'a BatchRenameConfiguration>,
    pub options: &'a ArchiveConfiguration,
    pub mirror: bool,
    pub resolution: DuplicateResolution,
    pub verify: bool,
    /// The archive the rule wrote last, relative to the destination, and what it held.
    pub previous: Option<&'a str>,
    pub fingerprint: Option<&'a str>,
    /// `YYYYMMDD-HHMM`, for the name of a copy.
    pub stamp: &'a str,
    pub cancel: &'a Arc<AtomicBool>,
    pub copied: &'a AtomicU64,
}

/// What a run of the archive left: the archive, relative to the destination, and what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Made {
    pub path: String,
    pub fingerprint: String,
    /// Nothing changed: no archive was written.
    pub unchanged: bool,
}

/// Whether a name at the top of the origin is one of the archives of the rule, which it never
/// takes as a file of its origin when it writes into it.
pub fn is_own(name: &str, archive: &str) -> bool {
    name.starts_with(TEMPORARY)
        || name.starts_with(&format!("{archive}.7z"))
        || (name.starts_with(&format!("{archive}-")) && name.ends_with(".7z"))
}

fn prepare_links(links: &Path) -> io::Result<()> {
    match fs::remove_dir_all(links) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::create_dir_all(links)
}

/// Fills the folder of links with the files the rule takes, and returns their count and what
/// they are: their paths in the archive, sizes and times.
pub fn link(
    origin: &Path,
    links: &Path,
    filters: &Filters,
    rename: Option<&BatchRenameConfiguration>,
    archive: &str,
    same_folder: bool,
    cancel: &AtomicBool,
) -> Result<(u64, String), Stop> {
    let failed = |what: &Path, error: io::Error| {
        Stop::Failed(format!("{} cannot be prepared: {error}", what.display()))
    };
    prepare_links(links).map_err(|error| failed(links, error))?;
    let mut hasher = DefaultHasher::new();
    let mut count = 0;
    let mut pending = vec![String::new()];
    while let Some(folder) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
            return Err(Stop::Interrupted);
        }
        let entries = sync::list(&origin.join(&folder))
            .map_err(|error| failed(&origin.join(&folder), error))?;
        for entry in entries {
            if folder.is_empty() && same_folder && is_own(&entry.name, archive) {
                continue;
            }
            let Some(facts) = entry.file else {
                pending.push(join(&folder, &entry.name));
                continue;
            };
            if !filters.admits(&entry.name, &facts) {
                continue;
            }
            let name = rename.map_or_else(
                || entry.name.clone(),
                |rename| rename::renamed(&entry.name, rename),
            );
            let inside = join(&folder, &name);
            let place = links.join(&inside);
            if let Some(parent) = place.parent() {
                fs::create_dir_all(parent).map_err(|error| failed(parent, error))?;
            }
            symlink(origin.join(join(&folder, &entry.name)), &place)
                .map_err(|error| failed(&place, error))?;
            (inside, facts.size, entry.modified).hash(&mut hasher);
            count += 1;
        }
    }
    Ok((count, format!("{count}:{:016x}", hasher.finish())))
}

impl Archive<'_> {
    pub async fn run(&self, same_folder: bool) -> Result<Made, Stop> {
        let (origin, links, filters, rename) = (
            self.origin.to_path_buf(),
            self.links.to_path_buf(),
            self.filters.clone(),
            self.rename.cloned(),
        );
        let (name, cancel) = (
            self.options.archive_name.trim().to_owned(),
            Arc::clone(self.cancel),
        );
        let (count, fingerprint) = tokio::task::spawn_blocking(move || {
            link(
                &origin,
                &links,
                &filters,
                rename.as_ref(),
                &name,
                same_folder,
                &cancel,
            )
        })
        .await
        .map_err(|error| Stop::Failed(format!("the archive could not be prepared: {error}")))??;
        let name = self.options.archive_name.trim();

        let previous = self
            .previous
            .filter(|path| self.destination.join(path).is_file());
        if let (Some(previous), Some(before)) = (previous, self.fingerprint) {
            if before == fingerprint {
                return Ok(Made {
                    path: previous.to_owned(),
                    fingerprint,
                    unchanged: true,
                });
            }
        }
        if count == 0 && !self.mirror {
            return Err(Stop::Failed("there is nothing to archive".to_owned()));
        }

        let wanted = if self.mirror {
            format!("{name}.7z")
        } else {
            format!("{name}-{}.7z", self.stamp)
        };
        let path = match previous {
            // A mirror updates the archive it made.
            Some(previous) if self.mirror => {
                self.update(&self.destination.join(previous)).await?;
                previous.to_owned()
            }
            _ => {
                let mut path = wanted.clone();
                if self.destination.join(&wanted).exists() {
                    match self.resolution {
                        DuplicateResolution::Overwrite => {}
                        DuplicateResolution::Ignore => {
                            return Err(Stop::Failed(format!(
                            "{wanted} is already in the destination and the rule leaves it alone"
                        )))
                        }
                        DuplicateResolution::Copy => path = free_name(self.destination, &wanted),
                    }
                }
                self.create(&self.destination.join(&path)).await?;
                path
            }
        };
        if self.verify || self.options.verify_archive {
            self.seven(&["t", "-bd"], &self.destination.join(&path), 1, None)
                .await?;
        }
        self.copied.fetch_add(count, Ordering::Relaxed);
        Ok(Made {
            path,
            fingerprint,
            unchanged: false,
        })
    }

    /// A new archive, written aside and renamed into place.
    async fn create(&self, target: &Path) -> Result<(), Stop> {
        let temporary = target.with_file_name(format!("{TEMPORARY}{}.7z", std::process::id()));
        let _ = fs::remove_file(&temporary);
        let outcome = async {
            // The password, and its confirmation for the 7-Zip that asks it (p7zip 16.02).
            self.seven(&self.writing("a"), &temporary, 2, Some(self.links))
                .await?;
            fs::rename(&temporary, target).map_err(|error| {
                Stop::Failed(format!(
                    "{} cannot be put in place: {error}",
                    target.display()
                ))
            })
        }
        .await;
        if outcome.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        outcome
    }

    /// The mirror's archive brought in line with the folder of links.
    async fn update(&self, archive: &Path) -> Result<(), Stop> {
        let mut arguments = self.writing("u");
        arguments.push(SYNC);
        // One password to open the archive, one for what goes in and its confirmation.
        self.seven(&arguments, archive, 3, Some(self.links)).await
    }

    fn writing(&self, command: &'static str) -> Vec<&'static str> {
        let mut arguments = vec![command, "-bd", "-t7z", "-mx=5", "-ms=off", "-mmt=2", "-p"];
        if self.options.encrypt_file_info {
            arguments.push("-mhe=on");
        }
        arguments
    }

    /// Runs 7z on an archive, with the password on its input `times` times; from `folder`, whose
    /// content it adds, when there is one.
    async fn seven(
        &self,
        arguments: &[&str],
        archive: &Path,
        times: usize,
        folder: Option<&Path>,
    ) -> Result<(), Stop> {
        let mut command = Command::new(self.seven_zip);
        command
            .args(arguments)
            .arg(archive)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(folder) = folder {
            command.current_dir(folder).arg(".");
        }
        let mut child = command.spawn().map_err(|error| {
            Stop::Failed(format!(
                "7-Zip cannot be started ({}): {error}",
                self.seven_zip
            ))
        })?;
        if let Some(mut input) = child.stdin.take() {
            let password = format!("{}\n", self.options.password).repeat(times);
            let _ = input.write_all(password.as_bytes()).await;
        }
        let output = async {
            let output = child.wait_with_output().await;
            output.map_err(|error| Stop::Failed(format!("7-Zip stopped: {error}")))
        };
        tokio::pin!(output);
        let output = loop {
            tokio::select! {
                output = &mut output => break output?,
                () = tokio::time::sleep(Duration::from_millis(500)) => {
                    if self.cancel.load(Ordering::Relaxed) {
                        // Dropping the child kills 7z.
                        return Err(Stop::Interrupted);
                    }
                }
            }
        };
        if output.status.success() {
            return Ok(());
        }
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail
            .lines()
            .rfind(|line| !line.trim().is_empty())
            .unwrap_or("no detail");
        Err(Stop::Failed(format!(
            "7-Zip ended with {} on {}: {detail}",
            output.status,
            archive.display()
        )))
    }
}

/// Where the links of a rule are prepared, in the state directory of the application.
pub fn links_of(work: &Path, rule_id: &str) -> PathBuf {
    let safe: String = rule_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    work.join("links").join(safe)
}

#[cfg(test)]
#[path = "../../tests/unit/axum/archive.rs"]
mod tests;
