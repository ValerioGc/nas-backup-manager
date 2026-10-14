use super::*;

const DAY: i64 = 86_400;

fn clock() -> Clock {
    Clock {
        today: 100 * DAY,
        yesterday: 99 * DAY,
        last_week: 93 * DAY,
        last_month: 70 * DAY,
        this_month: 90 * DAY,
    }
}

fn filter(
    action: &str,
    file_type: &str,
    status: &str,
    operator: Option<&str>,
    value: &str,
) -> OriginFilter {
    OriginFilter {
        id: 1,
        action: action.to_owned(),
        file_type: file_type.to_owned(),
        status: status.to_owned(),
        operator: operator.map(str::to_owned),
        value: value.to_owned(),
    }
}

fn facts(size: u64, modified: i64, created: i64) -> Facts {
    Facts {
        size,
        modified,
        created,
    }
}

#[test]
fn keeps_the_extensions_in_one_form() {
    assert_eq!(normalize_extensions(" * ").as_deref(), Ok("*"));
    assert_eq!(
        normalize_extensions(".JPG, .png,.jpg").as_deref(),
        Ok(".jpg,.png")
    );
    let many = (0..33)
        .map(|index| format!(".e{index}"))
        .collect::<Vec<_>>()
        .join(",");
    for refused in ["", "jpg", ".", ".j-pg", ". pdf", ",", &many] {
        assert!(normalize_extensions(refused).is_err(), "{refused}");
    }
}

#[test]
fn checks_that_a_filter_holds_together() {
    for valid in [
        filter("include", "*", "updated", Some("before"), "today"),
        filter("exclude", ".pdf", "created", Some("after"), "thisMonth"),
        filter("include", "*", "size", Some("greater"), "1,5"),
        filter("include", "*", "name", None, "*.tmp"),
    ] {
        assert_eq!(validate(&valid), Ok(()), "{valid:?}");
    }
    for invalid in [
        filter("keep", "*", "name", None, "a"),
        filter("include", "pdf", "name", None, "a"),
        filter("include", "*", "updated", Some("greater"), "today"),
        filter("include", "*", "updated", Some("before"), "tomorrow"),
        filter("include", "*", "size", Some("less"), "much"),
        filter("include", "*", "size", Some("less"), "-1"),
        filter("include", "*", "name", Some("before"), "a"),
        filter("include", "*", "name", None, "  "),
        filter("include", "*", "color", None, "red"),
    ] {
        assert!(validate(&invalid).is_err(), "{invalid:?}");
    }
}

#[test]
fn takes_what_an_include_matches_and_no_exclude_does() {
    let clock = clock();
    let none = compile(&[], &clock).expect("compiled");
    assert!(none.admits("anything", &facts(1, 0, 0)));

    let filters = compile(
        &[
            filter(
                "include",
                ".jpg, .png",
                "updated",
                Some("after"),
                "lastWeek",
            ),
            filter("include", ".pdf", "size", Some("greater"), "1"),
            filter("exclude", "*", "name", None, "draft*"),
        ],
        &clock,
    )
    .expect("compiled");
    let recent = 95 * DAY;
    assert!(filters.admits("Holiday.JPG", &facts(10, recent, 0)));
    assert!(!filters.admits("holiday.jpg", &facts(10, 90 * DAY, 0)));
    assert!(!filters.admits("draft-holiday.jpg", &facts(10, recent, 0)));
    assert!(filters.admits("book.pdf", &facts(2 * 1024 * 1024, 0, 0)));
    assert!(!filters.admits("book.pdf", &facts(1024, recent, 0)));
    assert!(!filters.admits("notes.txt", &facts(10, recent, 0)));
}

#[test]
fn measures_every_condition_against_the_days_of_the_run() {
    let clock = clock();
    let admits = |status: &str, operator: Option<&str>, value: &str, facts: Facts| {
        compile(&[filter("include", "*", status, operator, value)], &clock)
            .expect("compiled")
            .admits("file.txt", &facts)
    };
    assert!(admits(
        "updated",
        Some("before"),
        "today",
        facts(0, 99 * DAY, 0)
    ));
    assert!(!admits(
        "updated",
        Some("before"),
        "yesterday",
        facts(0, 99 * DAY, 0)
    ));
    assert!(admits(
        "created",
        Some("after"),
        "thisMonth",
        facts(0, 0, 90 * DAY)
    ));
    assert!(admits(
        "created",
        Some("before"),
        "lastMonth",
        facts(0, 0, 60 * DAY)
    ));
    assert!(admits("size", Some("less"), "0.5", facts(1024, 0, 0)));
    assert!(!admits("size", Some("greater"), "0.5", facts(1024, 0, 0)));
    assert!(admits("name", None, "fi?e.*", facts(0, 0, 0)));
    assert!(compile(&[filter("include", "*", "size", Some("less"), "x")], &clock).is_err());
}

#[test]
fn matches_names_with_stars_and_question_marks() {
    for (pattern, name, matches) in [
        ("*", "", true),
        ("*.jpg", "photo.jpg", true),
        ("*.jpg", "photo.jpeg", false),
        ("a*b*c", "aXXbYYc", true),
        ("a*b*c", "aXXbYY", false),
        ("?ommit", "commit", true),
        ("caff?", "caffè", true),
        ("report", "report", true),
        ("report", "reports", false),
    ] {
        assert_eq!(glob(pattern, name), matches, "{pattern} {name}");
    }
}

#[tokio::test]
async fn reads_the_days_of_the_clock_of_the_machine() {
    let clock = Clock::now().await.expect("clock");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("after 1970")
        .as_secs() as i64;
    assert!(clock.today <= now && now - clock.today <= DAY + 3600);
    // A day of summer time may be an hour shorter or longer.
    assert!((clock.today - clock.yesterday - DAY).abs() <= 3600);
    assert!(clock.last_week < clock.yesterday);
    assert!(clock.last_month < clock.last_week);
    assert!(clock.this_month <= clock.today && clock.today - clock.this_month <= 31 * DAY);
}

#[test]
fn reads_the_facts_of_a_file() {
    let file = std::env::temp_dir().join(format!("nas-backup-facts-{}", std::process::id()));
    std::fs::write(&file, "four").expect("file");
    let facts = Facts::of(&std::fs::metadata(&file).expect("metadata"));
    std::fs::remove_file(&file).expect("clean up");
    assert_eq!(facts.size, 4);
    assert!(facts.modified > 0 && facts.created > 0);
}
