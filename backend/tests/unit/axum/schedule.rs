use super::*;

fn schedule(
    frequency: Frequency,
    time: &str,
    weekday: Option<u8>,
    day: Option<u8>,
    month: Option<u8>,
) -> RuleSchedule {
    RuleSchedule {
        frequency,
        time: time.to_owned(),
        weekday,
        day,
        month,
    }
}

#[test]
fn reads_and_writes_the_time_as_the_minute_of_the_day() {
    assert_eq!(minute_of("00:00"), Some(0));
    assert_eq!(minute_of("02:30"), Some(150));
    assert_eq!(minute_of("23:59"), Some(1439));
    for time in ["24:00", "12:60", "2:30", "02:3", "0230", "aa:bb", ""] {
        assert_eq!(minute_of(time), None, "{time}");
    }
    assert_eq!(time_of(150), "02:30");
    assert_eq!(time_of(1439), "23:59");
}

#[test]
fn names_each_frequency_by_its_id() {
    for frequency in [
        Frequency::Daily,
        Frequency::Weekly,
        Frequency::Monthly,
        Frequency::Yearly,
    ] {
        assert_eq!(Frequency::parse(frequency.id()), Some(frequency));
    }
    assert_eq!(Frequency::parse("hourly"), None);
}

#[test]
fn accepts_exactly_the_fields_each_frequency_uses() {
    use Frequency::*;
    assert!(schedule(Daily, "02:00", None, None, None)
        .validate()
        .is_ok());
    assert!(schedule(Weekly, "02:00", Some(7), None, None)
        .validate()
        .is_ok());
    assert!(schedule(Monthly, "02:00", None, Some(31), None)
        .validate()
        .is_ok());
    assert!(schedule(Yearly, "02:00", None, Some(29), Some(2))
        .validate()
        .is_ok());

    assert!(schedule(Daily, "02:00", Some(1), None, None)
        .validate()
        .is_err());
    assert!(schedule(Weekly, "02:00", None, None, None)
        .validate()
        .is_err());
    assert!(schedule(Weekly, "02:00", Some(8), None, None)
        .validate()
        .is_err());
    assert!(schedule(Monthly, "02:00", None, Some(0), None)
        .validate()
        .is_err());
    assert!(schedule(Monthly, "02:00", None, Some(10), Some(1))
        .validate()
        .is_err());
    assert!(schedule(Yearly, "02:00", None, Some(30), Some(2))
        .validate()
        .is_err());
    assert!(schedule(Yearly, "02:00", None, Some(31), Some(4))
        .validate()
        .is_err());
    assert!(schedule(Yearly, "02:00", None, Some(1), Some(13))
        .validate()
        .is_err());
    assert!(schedule(Yearly, "02:00", None, None, Some(1))
        .validate()
        .is_err());
    assert!(schedule(Daily, "25:00", None, None, None)
        .validate()
        .is_err());
}
