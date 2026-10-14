//! Which files of the origin a rule takes. A filter names the files it is about (all of them, or
//! some extensions) and one condition: updated or created before or after a day, larger or smaller
//! than a size in MB, or a name that matches a pattern with `*` and `?`. A file is taken when it
//! matches at least one filter that includes (or there is none) and no filter that excludes.
//!
//! The days are read once per run, in the local time of the NAS: today and yesterday start at
//! midnight, the last week seven days ago at midnight, the last month thirty days ago at midnight,
//! this month on its first day.

use crate::rules::OriginFilter;
use std::{
    fs::Metadata,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::process::Command;

pub const DATE_VALUES: [&str; 5] = ["today", "yesterday", "lastWeek", "lastMonth", "thisMonth"];
const MAX_EXTENSIONS: usize = 32;
const MAX_MEGABYTES: f64 = 1_000_000_000.0;

/// The days the date conditions are measured against, as Unix seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    pub today: i64,
    pub yesterday: i64,
    pub last_week: i64,
    pub last_month: i64,
    pub this_month: i64,
}

impl Clock {
    /// The days of now, asked of `date`, which knows the time zone of the NAS.
    pub async fn now() -> Result<Self, String> {
        let first = date(&["+%Y-%m-01"]).await?;
        Ok(Self {
            today: seconds_at("today 00:00").await?,
            yesterday: seconds_at("yesterday 00:00").await?,
            last_week: seconds_at("7 days ago 00:00").await?,
            last_month: seconds_at("30 days ago 00:00").await?,
            this_month: seconds_at(&first).await?,
        })
    }

    fn day(&self, value: &str) -> Option<i64> {
        match value {
            "today" => Some(self.today),
            "yesterday" => Some(self.yesterday),
            "lastWeek" => Some(self.last_week),
            "lastMonth" => Some(self.last_month),
            "thisMonth" => Some(self.this_month),
            _ => None,
        }
    }
}

async fn date(arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("date")
        .args(arguments)
        .output()
        .await
        .map_err(|error| format!("the clock is not readable: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "the clock does not understand {}",
            arguments.join(" ")
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// The Unix seconds of a moment `date` understands, like `yesterday 00:00`.
async fn seconds_at(moment: &str) -> Result<i64, String> {
    date(&["+%s", "-d", moment])
        .await?
        .parse()
        .map_err(|_| format!("the clock does not understand {moment}"))
}

#[derive(Debug, Clone, PartialEq)]
enum Test {
    ModifiedBefore(i64),
    ModifiedAfter(i64),
    CreatedBefore(i64),
    CreatedAfter(i64),
    Larger(u64),
    Smaller(u64),
    Name(String),
}

#[derive(Debug, Clone, PartialEq)]
struct Condition {
    /// The extensions, lowercase with their dot, or `None` for every file.
    extensions: Option<Vec<String>>,
    test: Test,
}

/// The filters of a rule, ready to be asked about each file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Filters {
    include: Vec<Condition>,
    exclude: Vec<Condition>,
}

/// The extensions of a filter: `*`, or up to 32 like `.jpg, .png`, as they are stored.
pub fn normalize_extensions(value: &str) -> Result<String, String> {
    if value.trim() == "*" {
        return Ok("*".to_owned());
    }
    let mut extensions: Vec<String> = Vec::new();
    for part in value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let extension = part.to_lowercase();
        let valid = extension.len() > 1
            && extension.len() <= 17
            && extension.starts_with('.')
            && extension[1..].chars().all(|c| c.is_ascii_alphanumeric());
        if !valid {
            return Err(format!("{part} is not an extension like .pdf"));
        }
        if !extensions.contains(&extension) {
            extensions.push(extension);
        }
    }
    if extensions.is_empty() || extensions.len() > MAX_EXTENSIONS {
        return Err("A filter names all files, or 1 to 32 extensions.".to_owned());
    }
    Ok(extensions.join(","))
}

/// A size in MB of a filter, in bytes.
fn megabytes(value: &str) -> Option<u64> {
    let megabytes: f64 = value.trim().replace(',', ".").parse().ok()?;
    (megabytes.is_finite() && (0.0..=MAX_MEGABYTES).contains(&megabytes))
        .then_some((megabytes * 1024.0 * 1024.0) as u64)
}

/// Whether a filter holds together: its action, its files and a condition its operator fits.
pub fn validate(filter: &OriginFilter) -> Result<(), String> {
    if !matches!(filter.action.as_str(), "include" | "exclude") {
        return Err("A filter includes or excludes files.".to_owned());
    }
    normalize_extensions(&filter.file_type)?;
    let operator = filter.operator.as_deref();
    let valid = match filter.status.as_str() {
        "updated" | "created" => {
            matches!(operator, Some("before" | "after"))
                && DATE_VALUES.contains(&filter.value.as_str())
        }
        "size" => {
            matches!(operator, Some("greater" | "less")) && megabytes(&filter.value).is_some()
        }
        "name" => {
            operator.is_none() && !filter.value.trim().is_empty() && filter.value.len() <= 255
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err("Filter status, operator and value do not match.".to_owned())
    }
}

/// The filters of a rule against the days of this run.
pub fn compile(filters: &[OriginFilter], clock: &Clock) -> Result<Filters, String> {
    let mut compiled = Filters::default();
    for filter in filters {
        validate(filter)?;
        let extensions = normalize_extensions(&filter.file_type)?;
        let extensions =
            (extensions != "*").then(|| extensions.split(',').map(str::to_owned).collect());
        let day = || clock.day(&filter.value).unwrap_or_default();
        let test = match (filter.status.as_str(), filter.operator.as_deref()) {
            ("updated", Some("before")) => Test::ModifiedBefore(day()),
            ("updated", _) => Test::ModifiedAfter(day()),
            ("created", Some("before")) => Test::CreatedBefore(day()),
            ("created", _) => Test::CreatedAfter(day()),
            ("size", Some("greater")) => Test::Larger(megabytes(&filter.value).unwrap_or_default()),
            ("size", _) => Test::Smaller(megabytes(&filter.value).unwrap_or_default()),
            _ => Test::Name(filter.value.trim().to_lowercase()),
        };
        let condition = Condition { extensions, test };
        if filter.action == "exclude" {
            compiled.exclude.push(condition);
        } else {
            compiled.include.push(condition);
        }
    }
    Ok(compiled)
}

/// What the filters need to know of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts {
    pub size: u64,
    /// Unix seconds
    pub modified: i64,
    /// Unix seconds; the time of modification where the filesystem does not keep the creation.
    pub created: i64,
}

impl Facts {
    pub fn of(metadata: &Metadata) -> Self {
        let seconds = |time: std::io::Result<SystemTime>| {
            time.ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs() as i64)
        };
        let modified = seconds(metadata.modified()).unwrap_or_default();
        Self {
            size: metadata.len(),
            modified,
            created: seconds(metadata.created()).unwrap_or(modified),
        }
    }
}

impl Condition {
    fn matches(&self, name: &str, facts: &Facts) -> bool {
        let lower = name.to_lowercase();
        if let Some(extensions) = &self.extensions {
            if !extensions
                .iter()
                .any(|extension| lower.ends_with(extension.as_str()))
            {
                return false;
            }
        }
        match &self.test {
            Test::ModifiedBefore(day) => facts.modified < *day,
            Test::ModifiedAfter(day) => facts.modified >= *day,
            Test::CreatedBefore(day) => facts.created < *day,
            Test::CreatedAfter(day) => facts.created >= *day,
            Test::Larger(bytes) => facts.size > *bytes,
            Test::Smaller(bytes) => facts.size < *bytes,
            Test::Name(pattern) => glob(pattern, &lower),
        }
    }
}

impl Filters {
    /// Whether the rule takes a file.
    pub fn admits(&self, name: &str, facts: &Facts) -> bool {
        (self.include.is_empty()
            || self
                .include
                .iter()
                .any(|filter| filter.matches(name, facts)))
            && !self
                .exclude
                .iter()
                .any(|filter| filter.matches(name, facts))
    }
}

/// Whether a name matches a pattern where `*` is any text and `?` one character.
pub fn glob(pattern: &str, name: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let name: Vec<char> = name.chars().collect();
    let (mut p, mut n) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while n < name.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == name[n]) {
            p += 1;
            n += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some((p, n));
            p += 1;
        } else if let Some((star_p, star_n)) = star {
            p = star_p + 1;
            n = star_n + 1;
            star = Some((star_p, star_n + 1));
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|c| *c == '*')
}

#[cfg(test)]
#[path = "../../tests/unit/axum/filters.rs"]
mod tests;
