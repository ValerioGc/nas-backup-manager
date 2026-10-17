//! What a backup removes or replaces in its destination goes to the trash of the disk
//! (nas-file-manager), never away for good: a file deleted by mistake or ruined in the origin does
//! not take its last good copy with it. The file goes to `<disk>/.nas-trash/<its path on the disk>`;
//! a version already there stays beside, numbered (`report (1).pdf`). While the trash of the disk is
//! off (`.nas-trash-off` at its root) the file is removed, as the person chose.

use std::{
    fs,
    io::{self, ErrorKind},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

pub const TRASH: &str = ".nas-trash";
pub const TRASH_OFF: &str = ".nas-trash-off";

/// The trash of the disk a destination is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trash {
    disk: PathBuf,
}

impl Trash {
    pub fn new(disk: PathBuf) -> Self {
        Self { disk }
    }

    /// Moves a file of the disk to its trash, or removes it while the trash is off.
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
            fs::rename(&target, numbered(&target))?;
        }
        fs::rename(file, &target)
    }
}

/// Makes the folders of the trash down to `target`, refusing a link on the way.
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

/// The first free name beside a taken one: `report (1).pdf`, `report (2).pdf`...
pub fn numbered(taken: &Path) -> PathBuf {
    let name = taken
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (stem, extension) = match name.rfind('.') {
        Some(dot) if dot > 0 => name.split_at(dot),
        _ => (name.as_str(), ""),
    };
    (1..)
        .map(|number| taken.with_file_name(format!("{stem} ({number}){extension}")))
        .find(|candidate| fs::symlink_metadata(candidate).is_err())
        .unwrap_or_else(|| taken.to_path_buf())
}

#[cfg(test)]
#[path = "../../tests/unit/axum/trash.rs"]
mod tests;
