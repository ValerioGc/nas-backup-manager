//! Batch rename, a text added before the name or before the extension. A name already carrying
//! the text is left alone, so a file renamed in place is never renamed twice.

use crate::rules::BatchRenameConfiguration;

/// Splits a name into stem and extension, dot included. A name whose only dot is its first
/// character, like `.profile`, has no extension.
fn split(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(dot) if dot > 0 => name.split_at(dot),
        _ => (name, ""),
    }
}

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

pub fn carries(name: &str, rename: &BatchRenameConfiguration) -> bool {
    let (text, separator) = (rename.prefix.trim(), &rename.separator);
    if rename.position == "prefix" {
        name.starts_with(&format!("{text}{separator}"))
    } else {
        split(name).0.ends_with(&format!("{separator}{text}"))
    }
}

#[cfg(test)]
#[path = "../../tests/unit/rules/rename.rs"]
mod tests;
