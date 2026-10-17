use super::*;

fn schedule(
    frequency: Frequency,
    weekday: Option<u8>,
    day: Option<u8>,
    month: Option<u8>,
) -> RuleSchedule {
    RuleSchedule {
        frequency,
        time: "03:30".to_owned(),
        weekday,
        day,
        month,
    }
}

#[test]
fn writes_each_schedule_as_systemd_calendar_expressions() {
    assert_eq!(
        calendar(&schedule(Frequency::Daily, None, None, None)),
        ["*-*-* 03:30:00"]
    );
    assert_eq!(
        calendar(&schedule(Frequency::Weekly, Some(7), None, None)),
        ["Sun *-*-* 03:30:00"]
    );
    assert_eq!(
        calendar(&schedule(Frequency::Monthly, None, Some(5), None)),
        ["*-*-05 03:30:00"]
    );
    assert_eq!(
        calendar(&schedule(Frequency::Monthly, None, Some(30), None)),
        ["*-*-30 03:30:00", "*-02~01 03:30:00"]
    );
    assert_eq!(
        calendar(&schedule(Frequency::Monthly, None, Some(31), None)),
        ["*-*~01 03:30:00"]
    );
    assert_eq!(
        calendar(&schedule(Frequency::Yearly, None, Some(29), Some(2))),
        ["*-02~01 03:30:00"]
    );
    assert_eq!(
        calendar(&schedule(Frequency::Yearly, None, Some(24), Some(12))),
        ["*-12-24 03:30:00"]
    );
}

#[test]
fn reads_the_earliest_moment_systemd_answers() {
    let answer = "  Original form: *-*-30 03:30:00\n    Next elapse: Fri 2026-10-30 03:30:00 CET\n       (in UTC): Fri 2026-10-30 02:30:00 UTC\n\n       (in UTC): Sun 2027-02-28 02:30:00 UTC\n";
    assert_eq!(
        parse_elapses(answer),
        utc_milliseconds("2026-10-30", "02:30:00")
    );
    let in_utc = "Normalized form: *-*-* 03:00:00\n    Next elapse: Tue 2026-10-13 03:00:00 UTC\n       From now: 15h left\n";
    assert_eq!(
        parse_elapses(in_utc),
        utc_milliseconds("2026-10-13", "03:00:00")
    );
    let local = "    Next elapse: Tue 2026-10-13 03:00:00 CEST\n";
    assert_eq!(parse_elapses(local), None);
    assert_eq!(parse_elapses("nothing"), None);
    assert_eq!(utc_milliseconds("1970-01-01", "00:00:00"), Some(0));
    assert_eq!(
        utc_milliseconds("2000-03-01", "00:00:01"),
        Some(951_868_801_000)
    );
    assert_eq!(utc_milliseconds("2026-02-28", "bad"), None);
}

#[tokio::test]
async fn asks_systemd_when_a_schedule_comes_next() {
    let Some(next) = next_elapse(&calendar(&schedule(Frequency::Daily, None, None, None))).await
    else {
        eprintln!("systemd-analyze is not available here");
        return;
    };
    let now = crate::runner::now_ms();
    assert!(next > now && next - now <= 24 * 3600 * 1000 + 3600 * 1000);
    assert_eq!(next_elapse(&["not a calendar".to_owned()]).await, None);
}

#[test]
fn reads_when_a_rule_was_made_from_the_text_of_the_page() {
    assert_eq!(
        created_at_ms("2026-10-17T10:00:00.000Z"),
        utc_milliseconds("2026-10-17", "10:00:00")
    );
    assert_eq!(
        created_at_ms("2026-10-17T10:00:00Z"),
        utc_milliseconds("2026-10-17", "10:00:00")
    );
    assert_eq!(created_at_ms("x"), None);
}

#[tokio::test]
async fn finds_the_first_time_after_a_moment() {
    let after = utc_milliseconds("2026-10-14", "12:00:00").unwrap();
    // Without systemd-analyze, as in some containers, there is no answer to check.
    if let Some(next) = next_after(&["*-*-* 00:00:00 UTC".to_owned()], after).await {
        assert_eq!(next, utc_milliseconds("2026-10-15", "00:00:00").unwrap());
    }
}
