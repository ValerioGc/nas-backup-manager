//! Runs scheduled rules on time through `systemd-analyze calendar`, which knows the NAS time zone.
//! A time missed while the NAS was off is caught up once, with a notification.

use crate::{
    rules::{
        self,
        schedule::{Frequency, RuleSchedule},
    },
    runs::runner::Runner,
};
use std::{sync::Arc, time::Duration};
use tokio::process::Command;

/// Longest sleep before the calendar is read again, so clock and time zone changes are picked up.
const MAX_REST: Duration = Duration::from_secs(15 * 60);
/// Maximum delay of a run before its time counts as missed.
const LATE_MS: i64 = 5 * 60 * 1000;

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const SHORTEST_MONTH: u8 = 28;

/// Calendar expressions of a schedule. A day the month lacks, such as the 31st or February 30th,
/// runs on its last day.
pub fn calendar(schedule: &RuleSchedule) -> Vec<String> {
    let time = format!("{}:00", schedule.time);
    match schedule.frequency {
        Frequency::Daily => vec![format!("*-*-* {time}")],
        Frequency::Weekly => {
            let day = schedule
                .weekday
                .and_then(|day| WEEKDAYS.get(usize::from(day).wrapping_sub(1)))
                .unwrap_or(&"Mon");
            vec![format!("{day} *-*-* {time}")]
        }
        Frequency::Monthly => {
            let day = schedule.day.unwrap_or(1);
            if day <= SHORTEST_MONTH {
                vec![format!("*-*-{day:02} {time}")]
            } else if day == 31 {
                vec![format!("*-*~01 {time}")]
            } else {
                // The 29th and 30th where they exist, the last day of February otherwise.
                vec![format!("*-*-{day:02} {time}"), format!("*-02~01 {time}")]
            }
        }
        Frequency::Yearly => {
            let (month, day) = (schedule.month.unwrap_or(1), schedule.day.unwrap_or(1));
            if month == 2 && day == 29 {
                vec![format!("*-02~01 {time}")]
            } else {
                vec![format!("*-{month:02}-{day:02} {time}")]
            }
        }
    }
}

/// Next elapse of the expressions in Unix milliseconds.
pub async fn next_elapse(expressions: &[String]) -> Option<i64> {
    analyze(expressions, None).await
}

/// First elapse of a schedule after `after`, in Unix milliseconds.
pub async fn next_after(expressions: &[String], after: i64) -> Option<i64> {
    analyze(expressions, Some(after)).await
}

async fn analyze(expressions: &[String], after: Option<i64>) -> Option<i64> {
    let output = Command::new("systemd-analyze")
        .args(["calendar", "--iterations=1"])
        .args(after.map(|after| format!("--base-time=@{}", after.div_euclid(1000))))
        .args(expressions)
        .output()
        .await
        .ok()?;
    parse_elapses(&String::from_utf8_lossy(&output.stdout))
}

/// Creation time of a rule from the page timestamp, like `2026-10-17T10:00:00.000Z`.
pub fn created_at_ms(created_at: &str) -> Option<i64> {
    let (date, time) = created_at.split_once('T')?;
    let time = time.trim_end_matches('Z').split('.').next()?;
    utc_milliseconds(date, time)
}

/// Earliest `(in UTC): Wed 2026-10-14 01:00:00 UTC` line of a systemd-analyze answer. On a UTC
/// machine that line is missing and `Next elapse:` is already in UTC.
pub fn parse_elapses(text: &str) -> Option<i64> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            line.strip_prefix("(in UTC):")
                .or_else(|| line.strip_prefix("Next elapse:"))
        })
        .filter_map(|moment| {
            let mut parts = moment.split_whitespace().skip(1);
            let (date, time) = (parts.next()?, parts.next()?);
            (parts.next() == Some("UTC")).then(|| utc_milliseconds(date, time))?
        })
        .min()
}

/// Converts a UTC date like `2026-10-14` and a time like `01:00:00` to Unix milliseconds.
pub fn utc_milliseconds(date: &str, time: &str) -> Option<i64> {
    let mut date = date.split('-').map(str::parse::<i64>);
    let (year, month, day) = (date.next()?.ok()?, date.next()?.ok()?, date.next()?.ok()?);
    let mut time = time.split(':').map(str::parse::<i64>);
    let (hours, minutes, seconds) = (time.next()?.ok()?, time.next()?.ok()?, time.next()?.ok()?);
    // Days from the civil date, Howard Hinnant's algorithm.
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let of_era = year - era * 400;
    let of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let of_cycle = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    let days = era * 146_097 + of_cycle - 719_468;
    Some(((days * 24 + hours) * 60 + minutes) * 60_000 + seconds * 1000)
}

async fn scheduled(runner: &Runner) -> Vec<(String, RuleSchedule)> {
    match rules::load_all(runner.database(), runner.cipher()).await {
        Ok(rules) => rules
            .into_iter()
            .filter(|(stored, _)| stored.rule.enabled && stored.rule.rule_type == "scheduled")
            .filter_map(|(stored, _)| {
                stored
                    .rule
                    .schedule
                    .map(|schedule| (stored.rule.id, schedule))
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Refreshes the next run of a rule shown in the list, `None` when it is disabled or real-time.
pub async fn refresh(runner: &Runner, id: &str) {
    let schedule = rules::load(runner.database(), runner.cipher(), id)
        .await
        .ok()
        .flatten()
        .map(|stored| stored.rule)
        .filter(|rule| rule.enabled && rule.rule_type == "scheduled")
        .and_then(|rule| rule.schedule);
    let next = match schedule {
        Some(schedule) => next_elapse(&calendar(&schedule)).await,
        None => None,
    };
    runner.set_next(id, next);
}

async fn upcoming(runner: &Runner) -> Vec<(i64, String)> {
    let rules = scheduled(runner).await;
    let mut upcoming = Vec::with_capacity(rules.len());
    for (id, schedule) in &rules {
        let next = match runner.next_run(id) {
            Some(next) => Some(next),
            None => next_elapse(&calendar(schedule)).await,
        };
        runner.set_next(id, next);
        if let Some(next) = next {
            upcoming.push((next, id.clone()));
        }
    }
    runner.keep_next(&rules.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>());
    upcoming
}

/// Runs a rule whose time passed while the NAS was off or asleep, and notifies the user.
async fn start_late(runner: &Arc<Runner>, id: &str) {
    match Runner::start(runner, id, None, false).await {
        Ok(_) => {
            let name = rules::load(runner.database(), runner.cipher(), id)
                .await
                .ok()
                .flatten()
                .map(|stored| stored.rule.name)
                .unwrap_or_else(|| id.to_owned());
            runner
                .launcher()
                .notify(
                    "Backup eseguito in ritardo",
                    &format!(
                        "Il backup {name} non è partito all'orario previsto perché il NAS era \
                         spento o in sospensione, ed è stato eseguito ora."
                    ),
                    "warning",
                )
                .await;
        }
        Err(error) => eprintln!("<4>the late run of {id} did not start: {}", error.message),
    }
}

/// At startup, runs once every scheduled rule whose first time after its last run, or its creation,
/// has passed.
async fn catch_up(runner: &Arc<Runner>, now: i64) {
    let Ok(rules) = rules::load_all(runner.database(), runner.cipher()).await else {
        return;
    };
    for (stored, last) in rules {
        let rule = &stored.rule;
        let Some(schedule) = rule
            .schedule
            .as_ref()
            .filter(|_| rule.enabled && rule.rule_type == "scheduled")
        else {
            continue;
        };
        let Some(since) = last
            .as_ref()
            .map(|last| last.at)
            .or_else(|| created_at_ms(&rule.created_at))
        else {
            continue;
        };
        if next_after(&calendar(schedule), since)
            .await
            .is_some_and(|due| due + LATE_MS < now)
        {
            start_late(runner, &rule.id).await;
        }
    }
}

async fn start_due(runner: &Arc<Runner>, upcoming: Vec<(i64, String)>, now: i64) {
    for (at, id) in upcoming {
        if at > now {
            continue;
        }
        // Recompute from now so the next time is strictly after this one.
        runner.set_next(&id, None);
        if now - at > LATE_MS {
            eprintln!("<4>the run of {id} at {at} was missed because the NAS was not awake, so it runs now");
            start_late(runner, &id).await;
            continue;
        }
        if let Err(error) = Runner::start(runner, &id, None, false).await {
            eprintln!(
                "<4>the scheduled run of {id} did not start: {}",
                error.message
            );
        }
    }
}

pub async fn run(runner: Arc<Runner>) {
    catch_up(&runner, crate::runs::runner::now_ms()).await;
    loop {
        let upcoming = upcoming(&runner).await;
        let now = crate::runs::runner::now_ms();
        let first = upcoming.iter().map(|(at, _)| *at).min();
        let rest = first.map_or(MAX_REST, |at| {
            Duration::from_millis(at.saturating_sub(now).max(0) as u64).min(MAX_REST)
        });
        let woken = tokio::select! {
            () = tokio::time::sleep(rest) => false,
            () = runner.schedule_changed() => true,
        };
        let now = crate::runs::runner::now_ms();
        start_due(&runner, upcoming, now).await;
        if !woken && first.is_none_or(|at| at > now) {
            // After a long sleep the clock or the time zone may have changed.
            runner.forget_next();
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/runs/scheduler.rs"]
mod tests;
