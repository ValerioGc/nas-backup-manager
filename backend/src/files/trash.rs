//! What a backup removes or replaces goes to the disk trash `.nas-trash`, or is deleted while the
//! trash is off, marked by `.nas-trash-off`.

use crate::files::sync::free_name;
use std::{
    fs,
    io::{self, ErrorKind},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

/// Trash folder of a disk, also the prefix of its off marker. It is never backed up.
pub const TRASH: &str = ".nas-trash";
pub const TRASH_OFF: &str = ".nas-trash-off";

/// Trash of the disk that holds a destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trash {
    disk: PathBuf,
}

impl Trash {
    pub fn new(disk: PathBuf) -> Self {
        Self { disk }
    }

    /// Moves a file to the disk trash, or deletes it while the trash is off.
    pub fn discard(&self, file: &Path) -> io::Result<()> {
        let relative = file
            .strip_prefix(&self.disk)
            .map_err(|_| io::Error::new(ErrorKind::InvalidInput, "the file is not on the disk"))?;
        if relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "the file is not inside the disk",
            ));
        }
        if self.disk.join(TRASH_OFF).exists() {
            return fs::remove_file(file);
        }
        let trash = self.disk.join(TRASH);
        let target = trash.join(relative);
        prepare(&trash, &target)?;
        if fs::symlink_metadata(&target).is_ok() {
            let folder = target.parent().unwrap_or(&trash);
            let name = target
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            fs::rename(&target, folder.join(free_name(folder, &name)))?;
        }
        fs::rename(file, &target)
    }
}

/// Creates the trash folders down to `target`, refusing any link on the way.
fn prepare(trash: &Path, target: &Path) -> io::Result<()> {
    let mut directory = trash.to_path_buf();
    let mut folders = vec![trash.to_path_buf()];
    if let Some(parent) = target.parent() {
        for component in parent
            .strip_prefix(trash)
            .map_err(io::Error::other)?
            .components()
        {
            directory.push(component);
            folders.push(directory.clone());
        }
    }
    for folder in folders {
        match fs::symlink_metadata(&folder) {
            Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
                return Err(io::Error::new(
                    ErrorKind::InvalidInput,
                    "the trash path is not a folder",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {
                fs::create_dir(&folder)?;
                fs::set_permissions(&folder, fs::Permissions::from_mode(0o2770))?;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/files/trash.rs"]
mod tests;
