//! The batch rename of a rule: a text added before the name of each file or after it, before its
//! extension, with a separator: `report.pdf` becomes `backup_report.pdf` or `report_backup.pdf`.
//! A name that already carries the text where it goes is left as it is, so a file renamed in place
//! is never renamed twice.

use crate::rules::BatchRenameConfiguration;

/// The name and the extension of a file, the dot included; a name that starts with its only dot
/// (`.profile`) has no extension.
fn split(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(dot) if dot > 0 => name.split_at(dot),
        _ => (name, ""),
    }
}

/// The name a file takes.
pub fn renamed(name: &str, rename: &BatchRenameConfiguration) -> String {
    if carries(name, rename) {
        return name.to_owned();
    }
    let text = rename.prefix.trim();
    let separator = &rename.separator;
    if rename.position == "prefix" {
        format!("{text}{separator}{name}")
    } else {
        let (stem, extension) = split(name);
        format!("{stem}{separator}{text}{extension}")
    }
}

/// Whether a name already carries the text of the rename where it would go.
pub fn carries(name: &str, rename: &BatchRenameConfiguration) -> bool {
    let affix = |text: &str| {
        if rename.position == "prefix" {
            format!("{text}{}", rename.separator)
        } else {
            format!("{}{text}", rename.separator)
        }
    };
    let affix = affix(rename.prefix.trim());
    if rename.position == "prefix" {
        name.starts_with(&affix)
    } else {
        split(name).0.ends_with(&affix)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/axum/rename.rs"]
mod tests;
