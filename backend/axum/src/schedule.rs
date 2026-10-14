//! When a scheduled or cloud rule runs: every day, every week on a weekday, every month on a day
//! or every year on a date, always at a time of the NAS's local clock.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Frequency {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

impl Frequency {
    pub fn id(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
            Self::Yearly => "yearly",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        [Self::Daily, Self::Weekly, Self::Monthly, Self::Yearly]
            .into_iter()
            .find(|frequency| frequency.id() == id)
    }
}

/// The time is `HH:MM`; `weekday` (1 Monday to 7 Sunday) belongs to a weekly rule, `day` to a
/// monthly or yearly one and `month` (1 to 12) to a yearly one. A day past the end of a shorter
/// month, the 31st or February 29th, runs on its last day.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleSchedule {
    pub frequency: Frequency,
    pub time: String,
    #[serde(default)]
    pub weekday: Option<u8>,
    #[serde(default)]
    pub day: Option<u8>,
    #[serde(default)]
    pub month: Option<u8>,
}

/// The longest day of each month, February 29th included.
const MONTH_DAYS: [u8; 12] = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/// The minute of the day of an `HH:MM` time, or `None` when it is not one.
pub fn minute_of(time: &str) -> Option<u16> {
    let (hours, minutes) = time.split_once(':')?;
    if hours.len() != 2 || minutes.len() != 2 {
        return None;
    }
    let hours: u16 = hours.parse().ok().filter(|hours| *hours < 24)?;
    let minutes: u16 = minutes.parse().ok().filter(|minutes| *minutes < 60)?;
    Some(hours * 60 + minutes)
}

/// The `HH:MM` time of a minute of the day.
pub fn time_of(minute: u16) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

impl RuleSchedule {
    /// Checks that the schedule names a real time and exactly the fields its frequency uses.
    pub fn validate(&self) -> Result<(), &'static str> {
        minute_of(&self.time).ok_or("The time of the schedule is not HH:MM.")?;
        let weekday = self.weekday.map(|weekday| (1..=7).contains(&weekday));
        let month = self.month.map(|month| (1..=12).contains(&month));
        let day = self.day.map(|day| {
            let last = self
                .month
                .and_then(|month| MONTH_DAYS.get(usize::from(month).wrapping_sub(1)))
                .copied()
                .unwrap_or(31);
            (1..=last).contains(&day)
        });
        let valid = match self.frequency {
            Frequency::Daily => weekday.is_none() && day.is_none() && month.is_none(),
            Frequency::Weekly => weekday == Some(true) && day.is_none() && month.is_none(),
            Frequency::Monthly => weekday.is_none() && day == Some(true) && month.is_none(),
            Frequency::Yearly => weekday.is_none() && day == Some(true) && month == Some(true),
        };
        if valid {
            Ok(())
        } else {
            Err("The schedule does not match its frequency.")
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/axum/schedule.rs"]
mod tests;
