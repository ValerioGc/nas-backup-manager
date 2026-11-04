//! Schedules of scheduled and cloud rules, daily, weekly on a weekday, monthly on a day or yearly
//! on a date, always at a NAS local time.

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

/// `weekday` goes from 1 for Monday to 7. A day past the end of a shorter month runs on its last
/// day.
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

/// Last day of each month, February 29th included.
const MONTH_DAYS: [u8; 12] = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/// Minute of the day of an `HH:MM` time, or `None` when it is invalid.
pub fn minute_of(time: &str) -> Option<u16> {
    let (hours, minutes) = time.split_once(':')?;
    if hours.len() != 2 || minutes.len() != 2 {
        return None;
    }
    let hours: u16 = hours.parse().ok().filter(|hours| *hours < 24)?;
    let minutes: u16 = minutes.parse().ok().filter(|minutes| *minutes < 60)?;
    Some(hours * 60 + minutes)
}

pub fn time_of(minute: u16) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

impl RuleSchedule {
    /// Checks that the schedule has a valid time and exactly the fields its frequency uses.
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
#[path = "../../tests/unit/rules/schedule.rs"]
mod tests;
