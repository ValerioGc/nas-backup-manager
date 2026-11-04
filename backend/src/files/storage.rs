//! Resolves rule folders on the disks of the launcher registry `NAS_STORAGE_REGISTRY`. A disk must
//! be mounted, because the empty folder of a disconnected disk would make a mirror wipe the backup.

use std::{
    fs,
    path::{Component, Path, PathBuf},
    time::Duration,
};

/// Time a network disk has to answer before a run gives up.
const ANSWER_WITHIN: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Volume {
    pub id: String,
    /// Disk name as shown in Explorer.
    pub name: String,
    pub path: PathBuf,
    /// Share of another machine, `network` in the registry, changed from elsewhere.
    pub network: bool,
}

#[derive(Debug, Clone)]
pub enum Storage {
    /// Data disks of the NAS, which must be mounted.
    Volumes(Vec<Volume>),
    /// Single folder standing in for the disks in development.
    Root(PathBuf),
}

/// Disk as the page names it, the first segment of a logical path and its name.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VolumeName {
    pub id: String,
    pub name: String,
}

/// Rule folder on the machine and the disk it is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    pub path: PathBuf,
    pub volume: PathBuf,
    pub network: bool,
}

pub fn parse_registry(content: &str) -> Result<Vec<Volume>, String> {
    let mut volumes = Vec::new();
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        let fields: Vec<&str> = line.split('\t').collect();
        if !matches!(fields.len(), 3 | 4) || fields.iter().any(|field| field.is_empty()) {
            return Err("the registry of the disks is not readable".to_owned());
        }
        volumes.push(Volume {
            id: fields[0].to_owned(),
            name: fields[1].to_owned(),
            path: PathBuf::from(fields[2]),
            network: fields.get(3) == Some(&"network"),
        });
    }
    Ok(volumes)
}

impl Storage {
    /// Disks granted by the launcher, or the development folder.
    pub fn from_env() -> Result<Self, String> {
        let setting = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
        if let Some(registry) = setting("NAS_STORAGE_REGISTRY") {
            let content = fs::read_to_string(&registry)
                .map_err(|error| format!("the registry of the disks is not readable: {error}"))?;
            return parse_registry(&content).map(Self::Volumes);
        }
        if let Some(root) = setting("NAS_STORAGE_ROOT") {
            return Ok(Self::Root(PathBuf::from(root)));
        }
        if cfg!(debug_assertions) {
            return Ok(Self::Root(
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/storage"),
            ));
        }
        Err("The data disks are not available to this application.".to_owned())
    }

    /// Resolves a logical path on its disk, following links, and refuses one that leads outside it.
    /// The folder must exist unless `create` is true.
    pub fn resolve(&self, logical: &str, create: bool) -> Result<Folder, String> {
        let trimmed = logical.trim_matches('/');
        let mut segments = trimmed.split('/').filter(|segment| !segment.is_empty());
        if segments
            .clone()
            .any(|segment| segment == "." || segment == "..")
        {
            return Err(format!("{logical} is not a folder of the disks"));
        }
        let (volume, network, rest): (PathBuf, bool, Vec<&str>) = match self {
            Self::Volumes(volumes) => {
                let id = segments
                    .next()
                    .ok_or_else(|| format!("{logical} names no disk"))?;
                let volume = volumes
                    .iter()
                    .find(|volume| volume.id == id)
                    .ok_or_else(|| format!("the disk of {logical} is not connected"))?;
                (volume.path.clone(), volume.network, segments.collect())
            }
            Self::Root(root) => (root.clone(), false, segments.collect()),
        };
        let volume = volume
            .canonicalize()
            .map_err(|error| format!("the disk of {logical} is not readable: {error}"))?;
        if matches!(self, Self::Volumes(_)) && !is_mounted(&volume) {
            return Err(format!("the disk of {logical} is not mounted"));
        }
        let wanted = rest
            .iter()
            .fold(volume.clone(), |path, segment| path.join(segment));
        if create {
            fs::create_dir_all(&wanted)
                .map_err(|error| format!("{logical} cannot be created: {error}"))?;
        }
        let path = wanted
            .canonicalize()
            .map_err(|error| format!("{logical} is not reachable: {error}"))?;
        if !path.starts_with(&volume) || !path.is_dir() {
            return Err(format!("{logical} is not a folder of its disk"));
        }
        Ok(Folder {
            path,
            volume,
            network,
        })
    }

    /// Disk names the page shows instead of ids, empty without a registry.
    pub fn names(&self) -> Vec<VolumeName> {
        match self {
            Self::Volumes(volumes) => volumes
                .iter()
                .map(|volume| VolumeName {
                    id: volume.id.clone(),
                    name: volume.name.clone(),
                })
                .collect(),
            Self::Root(_) => Vec::new(),
        }
    }
}

/// Tells within a timeout whether a network folder holds anything. A share whose server is gone
/// fails or hangs, and a run must neither wait for it nor take it as empty.
pub async fn holds_files(path: &Path) -> Result<bool, String> {
    let folder = path.to_path_buf();
    let read = tokio::task::spawn_blocking(move || {
        fs::read_dir(&folder).map(|mut entries| entries.next().is_some())
    });
    match tokio::time::timeout(ANSWER_WITHIN, read).await {
        Err(_) => Err(format!("{} does not answer", path.display())),
        Ok(Err(_)) => Err("the network disk could not be read".to_owned()),
        Ok(Ok(listed)) => {
            listed.map_err(|error| format!("{} cannot be read: {error}", path.display()))
        }
    }
}

/// Tells whether a folder is a mount point, since a disconnected disk leaves an empty folder
/// behind.
pub fn is_mounted(path: &Path) -> bool {
    let Ok(mounts) = fs::read_to_string("/proc/self/mountinfo") else {
        return false;
    };
    mounts.lines().any(|line| {
        line.split_whitespace()
            .nth(4)
            .is_some_and(|mount| Path::new(&unescape(mount)) == path)
    })
}

/// Decodes a mountinfo path, where space, tab, newline and backslash are octal escaped.
fn unescape(field: &str) -> String {
    field
        .replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\012", "\n")
        .replace("\\134", "\\")
}

/// Tells whether `inner` is `outer` or lies inside it.
pub fn nested(inner: &Path, outer: &Path) -> bool {
    inner.starts_with(outer)
}

/// Relative path in the `/` separated form the records keep.
pub fn relative(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
#[path = "../../tests/unit/files/storage.rs"]
mod tests;
