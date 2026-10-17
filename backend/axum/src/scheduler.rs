//! Runs the scheduled rules at their time. One loop for all of them: it asks systemd when each
//! rule comes next (`systemd-analyze calendar`, which knows the time zone and summer time of the
//! NAS), sleeps until the first one, at most a quarter of an hour, and wakes at once when a rule
//! changes. A time missed while the NAS was off or asleep is caught up once: at the start, a rule
//! whose first time after its last run (or its creation) has passed runs now, and the person is
//! told; a time that comes while the rule is still running is skipped.

use crate::{
    rules,
    runner::Runner,
    schedule::{Frequency, RuleSchedule},
};
use std::{sync::Arc, time::Duration};
use tokio::process::Command;

/// The longest the loop sleeps before it reads the calendar again: a clock set forward or back,
/// or a new time zone, is caught within this.
const MAX_REST: Duration = Duration::from_secs(15 * 60);
/// How late a run may start: past this the time counts as missed.
const LATE_MS: i64 = 5 * 60 * 1000;

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const SHORTEST_MONTH: u8 = 28;

/// The calendar expressions of a schedule: a day the month does not have (the 31st, the 30th of
/// February) runs on its last day.
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
                // The 29th and 30th where the month has them, the last day of February otherwise.
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

/// When the first of the expressions comes next, in Unix milliseconds.
pub async fn next_elapse(expressions: &[String]) -> Option<i64> {
    let output = Command::new("systemd-analyze")
        .args(["calendar", "--iterations=1"])
        .args(expressions)
        .output()
        .await
        .ok()?;
    parse_elapses(&String::from_utf8_lossy(&output.stdout))
}

/// The first time of a schedule after the moment `after` (Unix milliseconds).
pub async fn next_after(expressions: &[String], after: i64) -> Option<i64> {
    let output = Command::new("systemd-analyze")
        .args(["calendar", "--iterations=1"])
        .arg(format!("--base-time=@{}", after.div_euclid(1000)))
        .args(expressions)
        .output()
        .await
        .ok()?;
    parse_elapses(&String::from_utf8_lossy(&output.stdout))
}

/// When a rule was made, from the text the page writes (`2026-10-17T10:00:00.000Z`).
pub fn created_at_ms(created_at: &str) -> Option<i64> {
    let (date, time) = created_at.split_once('T')?;
    let time = time.trim_end_matches('Z').split('.').next()?;
    utc_milliseconds(date, time)
}

/// The earliest `(in UTC): Wed 2026-10-14 01:00:00 UTC` of an answer of systemd-analyze. On a
/// machine in UTC there is no such line: `Next elapse:` is already in UTC.
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

/// `2026-10-14` and `01:00:00`, in UTC, as Unix milliseconds.
pub fn utc_milliseconds(date: &str, time: &str) -> Option<i64> {
    let mut date = date.split('-').map(str::parse::<i64>);
    let (year, month, day) = (date.next()?.ok()?, date.next()?.ok()?, date.next()?.ok()?);
    let mut time = time.split(':').map(str::parse::<i64>);
    let (hours, minutes, seconds) = (time.next()?.ok()?, time.next()?.ok()?, time.next()?.ok()?);
    // Days from the civil date (Howard Hinnant's algorithm).
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let of_era = year - era * 400;
    let of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let of_cycle = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    let days = era * 146_097 + of_cycle - 719_468;
    Some(((days * 24 + hours) * 60 + minutes) * 60_000 + seconds * 1000)
}

/// The scheduled rules that are on, with their schedule.
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

/// When a rule comes next, kept by the runner for the list: none when it is off or real-time.
pub async fn refresh(runner: &Runner, id: &str) {
    let rule = scheduled(runner)
        .await
        .into_iter()
        .find(|(rule, _)| rule == id);
    let next = match rule {
        Some((_, schedule)) => next_elapse(&calendar(&schedule)).await,
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

/// Runs a rule whose time passed while the NAS was off or asleep, and tells the person.
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
                         spento o in sospensione: è stato eseguito ora."
                    ),
                    "warning",
                )
                .await;
        }
        Err(error) => eprintln!("<4>the late run of {id} did not start: {}", error.message),
    }
}

/// At the start: the scheduled rules whose first time after their last run (or their creation)
/// has passed run now, once.
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
        // Read again from now on: the next time is strictly after this one.
        runner.set_next(&id, None);
        if now - at > LATE_MS {
            eprintln!("<4>the run of {id} at {at} was missed: the NAS was not awake; it runs now");
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
    catch_up(&runner, crate::runner::now_ms()).await;
    loop {
        let upcoming = upcoming(&runner).await;
        let now = crate::runner::now_ms();
        let first = upcoming.iter().map(|(at, _)| *at).min();
        let rest = first.map_or(MAX_REST, |at| {
            Duration::from_millis(at.saturating_sub(now).max(0) as u64).min(MAX_REST)
        });
        let woken = tokio::select! {
            () = tokio::time::sleep(rest) => false,
            () = runner.schedule_changed() => true,
        };
        let now = crate::runner::now_ms();
        start_due(&runner, upcoming, now).await;
        if !woken && first.is_none_or(|at| at > now) {
            // A long rest: the clock or the time zone may have changed since.
            runner.forget_next();
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/axum/scheduler.rs"]
mod tests;
