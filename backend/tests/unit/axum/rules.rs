use super::*;
use crate::test_support::{state, state_with, Scratch};

fn daily_at(time: &str) -> Option<RuleSchedule> {
    Some(RuleSchedule {
        frequency: Frequency::Daily,
        time: time.to_owned(),
        weekday: None,
        day: None,
        month: None,
    })
}

/// A scheduled rule from `/Photos` to `/Backup/photos`, mirroring every day at 02:00.
pub fn scheduled_rule(id: &str) -> BackupRule {
    BackupRule {
        id: id.to_owned(),
        rule_type: "scheduled".to_owned(),
        name: "Photos".to_owned(),
        created_at: "2026-10-07T20:00:00.000Z".to_owned(),
        enabled: true,
        folders: RuleFolders {
            origin: Some("/Photos".to_owned()),
            destination: Some("/Backup/photos".to_owned()),
            cloud: None,
        },
        same_destination: false,
        sync_mode: SyncMode::Mirror,
        schedule: daily_at("02:00"),
        filters: Vec::new(),
        operations: Vec::new(),
        operation_configurations: FileOperationConfigurations::default(),
        duplicate_resolution: Some(DuplicateResolution::Overwrite),
        verify_archived_files: false,
    }
}

fn rtl_rule() -> BackupRule {
    BackupRule {
        rule_type: "rtl".to_owned(),
        schedule: None,
        ..scheduled_rule("live")
    }
}

fn archive() -> ArchiveConfiguration {
    ArchiveConfiguration {
        archive_name: "daily".to_owned(),
        password: "archive-secret".to_owned(),
        encrypt_file_info: true,
        verify_archive: true,
    }
}

fn rename() -> BatchRenameConfiguration {
    BatchRenameConfiguration {
        filename: "example.txt".to_owned(),
        separator: "_".to_owned(),
        prefix: "2026".to_owned(),
        position: "prefix".to_owned(),
    }
}

fn filter(id: i64, action: &str, file_type: &str) -> OriginFilter {
    OriginFilter {
        id,
        action: action.to_owned(),
        file_type: file_type.to_owned(),
        status: "updated".to_owned(),
        operator: Some("before".to_owned()),
        value: "today".to_owned(),
    }
}

async fn listed(state: &AppState) -> Vec<RuleView> {
    list_rules(State(state.clone())).await.expect("listed").0
}

#[tokio::test]
async fn saves_a_rule_with_encrypted_passwords_and_never_shows_them() {
    let state = state().await;
    let rule = BackupRule {
        name: "  Photos of the year ".to_owned(),
        schedule: Some(RuleSchedule {
            frequency: Frequency::Yearly,
            time: "02:30".to_owned(),
            weekday: None,
            day: Some(29),
            month: Some(2),
        }),
        filters: vec![filter(7, "exclude", ".PDF, .docx")],
        operations: vec![FileOperation::Archive, FileOperation::BatchRename],
        operation_configurations: FileOperationConfigurations {
            batch_rename: Some(rename()),
            archive: Some(archive()),
            encrypt: None,
        },
        duplicate_resolution: Some(DuplicateResolution::Copy),
        verify_archived_files: true,
        ..scheduled_rule("saved")
    };

    let (status, Json(answer)) = create_rule(State(state.clone()), Json(rule))
        .await
        .expect("saved");
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(answer.name, "Photos of the year");
    assert_eq!(
        answer.operation_configurations.archive.unwrap().password,
        ""
    );

    let (operation_data, minute): (String, i64) =
        sqlx::query_as("SELECT operation_data, schedule_minute FROM backup_rules")
            .fetch_one(&state.database)
            .await
            .expect("row");
    assert!(!operation_data.contains("archive-secret"));
    assert_eq!(minute, 150);

    let views = listed(&state).await;
    let view = &views[0];
    assert_eq!(view.rule.filters[0].file_type, ".pdf,.docx");
    assert_eq!(view.rule.filters[0].action, "exclude");
    assert_eq!(
        view.rule
            .operation_configurations
            .archive
            .as_ref()
            .unwrap()
            .password,
        ""
    );
    assert_eq!(
        view.rule.duplicate_resolution,
        Some(DuplicateResolution::Copy)
    );
    assert!(view.last_run.is_none());
    assert!(!view.running);
    let shown = serde_json::to_string(view).expect("json");
    assert!(!shown.contains("archive-secret"));
    assert!(shown.contains("\"nextRunAt\""));

    let stored = load(&state.database, &state.secret_cipher, "saved")
        .await
        .expect("loaded")
        .expect("there");
    assert_eq!(
        stored
            .rule
            .operation_configurations
            .archive
            .unwrap()
            .password,
        "archive-secret"
    );
    assert!(load(&state.database, &state.secret_cipher, "none")
        .await
        .expect("loaded")
        .is_none());

    let twice = create_rule(State(state.clone()), Json(scheduled_rule("saved")))
        .await
        .expect_err("taken");
    assert_eq!(twice.status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn edits_a_rule_keeping_its_password_and_forgetting_its_copies_when_its_folders_change() {
    let state = state().await;
    let rule = BackupRule {
        operations: vec![FileOperation::Archive],
        operation_configurations: FileOperationConfigurations {
            archive: Some(archive()),
            ..FileOperationConfigurations::default()
        },
        ..scheduled_rule("edited")
    };
    let _ = create_rule(State(state.clone()), Json(rule.clone()))
        .await
        .expect("created");
    let remember = || {
        sqlx::query(
            "INSERT INTO rule_files (rule_id, parent, name, destination, size, modified)
             VALUES ('edited', '', 'a', 'a', 1, 1)",
        )
        .execute(&state.database)
    };
    remember().await.expect("a copy");
    let copies = || async {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM rule_files")
            .fetch_one(&state.database)
            .await
            .expect("count")
    };

    let mut renamed = rule.clone();
    renamed.name = "New name".to_owned();
    renamed
        .operation_configurations
        .archive
        .as_mut()
        .unwrap()
        .password = String::new();
    let _ = update_rule(
        State(state.clone()),
        UrlPath("edited".to_owned()),
        Json(renamed.clone()),
    )
    .await
    .expect("edited");
    let stored = load(&state.database, &state.secret_cipher, "edited")
        .await
        .expect("loaded")
        .expect("there");
    assert_eq!(stored.rule.name, "New name");
    assert_eq!(
        stored
            .rule
            .operation_configurations
            .archive
            .unwrap()
            .password,
        "archive-secret"
    );
    assert_eq!(
        copies().await,
        1,
        "same folders: the copies are still its own"
    );

    let mut moved = renamed.clone();
    moved.folders.destination = Some("/Backup/elsewhere".to_owned());
    let _ = update_rule(
        State(state.clone()),
        UrlPath("edited".to_owned()),
        Json(moved),
    )
    .await
    .expect("moved");
    assert_eq!(copies().await, 0);

    let wrong = update_rule(
        State(state.clone()),
        UrlPath("other".to_owned()),
        Json(renamed.clone()),
    )
    .await
    .expect_err("address");
    assert_eq!(wrong.status, StatusCode::BAD_REQUEST);
    let mut ghost = renamed;
    ghost.id = "ghost".to_owned();
    let missing = update_rule(
        State(state.clone()),
        UrlPath("ghost".to_owned()),
        Json(ghost),
    )
    .await
    .expect_err("missing");
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_new_archive_needs_its_password() {
    let state = state().await;
    let mut rule = BackupRule {
        operations: vec![FileOperation::Archive],
        operation_configurations: FileOperationConfigurations {
            archive: Some(archive()),
            ..FileOperationConfigurations::default()
        },
        ..scheduled_rule("locked")
    };
    rule.operation_configurations
        .archive
        .as_mut()
        .unwrap()
        .password = String::new();
    let refused = create_rule(State(state), Json(rule))
        .await
        .expect_err("no password");
    assert_eq!(refused.message, "A password is required.");
}

#[tokio::test]
async fn switches_and_removes_a_rule() {
    let state = state().await;
    let _ = create_rule(State(state.clone()), Json(scheduled_rule("switched")))
        .await
        .expect("created");
    let status = switch_rule(
        State(state.clone()),
        UrlPath("switched".to_owned()),
        Json(Switch { enabled: false }),
    )
    .await
    .expect("switched");
    assert_eq!(status, StatusCode::NO_CONTENT);
    let views = listed(&state).await;
    assert!(!views[0].rule.enabled);
    assert_eq!(views[0].next_run_at, None, "a rule turned off does not run");
    switch_rule(
        State(state.clone()),
        UrlPath("switched".to_owned()),
        Json(Switch { enabled: true }),
    )
    .await
    .expect("switched on");
    if crate::scheduler::next_elapse(&["*-*-* 02:00:00".to_owned()])
        .await
        .is_some()
    {
        assert!(listed(&state).await[0].next_run_at.is_some());
    }

    assert_eq!(
        delete_rule(State(state.clone()), UrlPath("switched".to_owned()))
            .await
            .expect("removed"),
        StatusCode::NO_CONTENT
    );
    assert!(listed(&state).await.is_empty());
    for missing in [
        delete_rule(State(state.clone()), UrlPath("switched".to_owned()))
            .await
            .expect_err("gone"),
        switch_rule(
            State(state.clone()),
            UrlPath("switched".to_owned()),
            Json(Switch { enabled: true }),
        )
        .await
        .expect_err("gone"),
        stop_rule(State(state.clone()), UrlPath("switched".to_owned()))
            .await
            .expect_err("not running"),
        run_rule(State(state.clone()), UrlPath("switched".to_owned()))
            .await
            .expect_err("gone"),
    ] {
        assert_eq!(missing.status, StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn a_running_rule_is_neither_edited_nor_removed_until_it_stops() {
    let disks = Scratch::new("rules-running");
    disks.write("Photos/a.txt", "a");
    let slow = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/unit/axum/fixtures/slow-7z"
    );
    let mut state = state_with(disks.path(), None).await;
    let runner = crate::runner::Runner::new(
        state.database.clone(),
        std::sync::Arc::clone(&state.secret_cipher),
        crate::launcher::Launcher::new(None),
        crate::runner::Disks::Fixed(crate::storage::Storage::Root(disks.path().to_path_buf())),
        disks.path().join(".work"),
    )
    .with_seven_zip(slow);
    state.runner = std::sync::Arc::new(runner);
    let rule = BackupRule {
        operations: vec![FileOperation::Archive],
        operation_configurations: FileOperationConfigurations {
            archive: Some(archive()),
            ..FileOperationConfigurations::default()
        },
        ..scheduled_rule("busy")
    };
    let _ = create_rule(State(state.clone()), Json(rule.clone()))
        .await
        .expect("created");

    assert_eq!(
        run_rule(State(state.clone()), UrlPath("busy".to_owned()))
            .await
            .expect("started"),
        StatusCode::ACCEPTED
    );
    for refused in [
        run_rule(State(state.clone()), UrlPath("busy".to_owned()))
            .await
            .expect_err("twice"),
        update_rule(State(state.clone()), UrlPath("busy".to_owned()), Json(rule))
            .await
            .expect_err("busy"),
        delete_rule(State(state.clone()), UrlPath("busy".to_owned()))
            .await
            .expect_err("busy"),
    ] {
        assert_eq!(refused.status, StatusCode::CONFLICT);
    }
    assert!(listed(&state).await[0].running);
    assert_eq!(
        stop_rule(State(state.clone()), UrlPath("busy".to_owned()))
            .await
            .expect("stopped"),
        StatusCode::ACCEPTED
    );
    for _ in 0..50 {
        if !state.runner.is_running("busy") {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let last = listed(&state).await[0]
        .last_run
        .clone()
        .expect("a last run");
    assert_eq!(last.status, "interrupted");
}

#[test]
fn asks_a_schedule_of_scheduled_and_cloud_rules_only() {
    let unscheduled = BackupRule {
        schedule: None,
        ..scheduled_rule("rule")
    };
    assert!(validate_rule(&unscheduled).is_err());
    assert!(validate_rule(&BackupRule {
        schedule: daily_at("24:00"),
        ..scheduled_rule("rule")
    })
    .is_err());
    assert!(validate_rule(&rtl_rule()).is_ok());
    assert!(validate_rule(&BackupRule {
        schedule: daily_at("02:00"),
        ..rtl_rule()
    })
    .is_err());
    assert!(validate_schedule("cloud", daily_at("2:00").as_ref()).is_err());
}

#[test]
fn checks_the_name_the_filters_and_the_choice_on_duplicates() {
    for name in ["", "   ", &"x".repeat(81)] {
        assert!(validate_rule(&BackupRule {
            name: name.to_owned(),
            ..scheduled_rule("rule")
        })
        .is_err());
    }
    assert!(validate_rule(&BackupRule {
        duplicate_resolution: None,
        ..scheduled_rule("rule")
    })
    .is_err());
    let mut wrong = filter(1, "include", "*");
    wrong.status = "size".to_owned();
    assert!(validate_rule(&BackupRule {
        filters: vec![wrong],
        ..scheduled_rule("rule")
    })
    .is_err());
    assert!(validate_rule(&BackupRule {
        filters: vec![filter(1, "include", "*"), filter(2, "exclude", ".tmp")],
        ..scheduled_rule("rule")
    })
    .is_ok());
}

#[test]
fn keeps_operations_and_their_options_together() {
    let options_alone = BackupRule {
        operation_configurations: FileOperationConfigurations {
            archive: Some(archive()),
            ..FileOperationConfigurations::default()
        },
        ..scheduled_rule("rule")
    };
    assert!(validate_rule(&options_alone).is_err());
    let operation_alone = BackupRule {
        operations: vec![FileOperation::Archive],
        ..scheduled_rule("rule")
    };
    assert!(validate_rule(&operation_alone).is_err());
    let twice = BackupRule {
        operations: vec![FileOperation::Archive, FileOperation::Archive],
        ..options_alone.clone()
    };
    assert!(validate_rule(&twice).is_err());
    let encrypted = BackupRule {
        operations: vec![FileOperation::Encrypt],
        operation_configurations: FileOperationConfigurations {
            encrypt: Some(CryptConfiguration {
                mode: "encrypt".to_owned(),
                output: "single".to_owned(),
                password: "x".to_owned(),
            }),
            ..FileOperationConfigurations::default()
        },
        ..scheduled_rule("rule")
    };
    assert_eq!(
        validate_rule(&encrypted).expect_err("not yet").message,
        "Encryption is not available yet."
    );
    for (name, separator, text) in [("a", "+", "x"), ("a", "_", "a/b"), ("", "_", "x")] {
        assert!(validate_rule(&BackupRule {
            operations: vec![FileOperation::BatchRename],
            operation_configurations: FileOperationConfigurations {
                batch_rename: Some(BatchRenameConfiguration {
                    filename: name.to_owned(),
                    separator: separator.to_owned(),
                    prefix: text.to_owned(),
                    position: "prefix".to_owned(),
                }),
                ..FileOperationConfigurations::default()
            },
            ..scheduled_rule("rule")
        })
        .is_err());
    }
    for name in [".hidden", "a/b", ""] {
        let mut bad = archive();
        bad.archive_name = name.to_owned();
        assert!(validate_rule(&BackupRule {
            operations: vec![FileOperation::Archive],
            operation_configurations: FileOperationConfigurations {
                archive: Some(bad),
                ..FileOperationConfigurations::default()
            },
            ..scheduled_rule("rule")
        })
        .is_err());
    }
    let both = BackupRule {
        operations: vec![FileOperation::Archive, FileOperation::BatchRename],
        operation_configurations: FileOperationConfigurations {
            batch_rename: Some(rename()),
            archive: Some(archive()),
            encrypt: None,
        },
        ..scheduled_rule("rule")
    };
    assert!(validate_rule(&both).is_ok());
}

#[test]
fn a_real_time_rule_may_write_into_its_origin_only_with_an_operation() {
    let in_place = BackupRule {
        same_destination: true,
        folders: RuleFolders {
            origin: Some("/Photos".to_owned()),
            ..RuleFolders::default()
        },
        ..rtl_rule()
    };
    assert!(validate_rule(&in_place).is_err(), "nothing to do in place");
    let renaming = BackupRule {
        operations: vec![FileOperation::BatchRename],
        operation_configurations: FileOperationConfigurations {
            batch_rename: Some(rename()),
            ..FileOperationConfigurations::default()
        },
        ..in_place.clone()
    };
    assert!(validate_rule(&renaming).is_ok());
    assert_eq!(
        normalized(renaming).expect("normalized").sync_mode,
        SyncMode::Copy
    );
    assert!(
        validate_rule(&BackupRule {
            folders: rtl_rule().folders,
            ..in_place.clone()
        })
        .is_err(),
        "a destination too"
    );
    assert!(validate_rule(&BackupRule {
        rule_type: "scheduled".to_owned(),
        ..in_place
    })
    .is_err());
}

#[test]
fn requires_the_folders_of_each_backup_type_apart_from_each_other() {
    let folders =
        |origin: Option<&str>, destination: Option<&str>, cloud: Option<&str>| RuleFolders {
            origin: origin.map(str::to_owned),
            destination: destination.map(str::to_owned),
            cloud: cloud.map(str::to_owned),
        };
    let with = |rule_type: &str, folders: RuleFolders, mode: SyncMode| BackupRule {
        rule_type: rule_type.to_owned(),
        folders,
        sync_mode: mode,
        ..scheduled_rule("rule")
    };
    let ok = |rule_type, folders, mode| validate_folders(&with(rule_type, folders, mode)).is_ok();

    assert!(ok(
        "scheduled",
        folders(Some("/Photos"), Some("/Backup/p"), None),
        SyncMode::Mirror
    ));
    assert!(ok(
        "rtl",
        folders(Some("/Photos"), Some("/Backup/p"), None),
        SyncMode::Mirror
    ));
    assert!(ok(
        "cloud",
        folders(None, None, Some("/mnt/share")),
        SyncMode::Mirror
    ));
    assert!(ok(
        "scheduled",
        folders(Some("/Photos"), Some("/Backup"), None),
        SyncMode::Copy
    ));
    for (rule_type, folders, mode) in [
        (
            "scheduled",
            folders(Some("/Photos"), None, None),
            SyncMode::Mirror,
        ),
        ("cloud", folders(None, None, None), SyncMode::Mirror),
        (
            "rtl",
            folders(None, Some("/Backup/p"), None),
            SyncMode::Mirror,
        ),
        (
            "rtl",
            folders(Some("/Photos"), Some("Backup"), None),
            SyncMode::Mirror,
        ),
        (
            "rtl",
            folders(Some("/Photos"), Some("/Backup/../etc"), None),
            SyncMode::Mirror,
        ),
        (
            "scheduled",
            folders(Some("/Photos"), Some("/Photos/backup"), None),
            SyncMode::Copy,
        ),
        (
            "scheduled",
            folders(Some("/Backup/p/in"), Some("/Backup/p"), None),
            SyncMode::Copy,
        ),
        (
            "scheduled",
            folders(Some("/Photos"), Some("/Backup"), None),
            SyncMode::Mirror,
        ),
        (
            "cloud",
            folders(Some("/Photos"), None, Some("/mnt/share")),
            SyncMode::Mirror,
        ),
    ] {
        assert!(
            !ok(rule_type, folders.clone(), mode),
            "{rule_type} {folders:?} {mode:?}"
        );
    }
}

#[test]
fn validates_identity_and_type() {
    for rule in [
        BackupRule {
            id: " ".to_owned(),
            ..scheduled_rule("rule")
        },
        BackupRule {
            rule_type: "ftp".to_owned(),
            ..scheduled_rule("rule")
        },
        BackupRule {
            created_at: String::new(),
            ..scheduled_rule("rule")
        },
    ] {
        assert!(validate_rule(&rule).is_err());
    }
    assert_eq!(SyncMode::parse("copy"), SyncMode::Copy);
    assert_eq!(SyncMode::parse("anything"), SyncMode::Mirror);
    assert_eq!(
        DuplicateResolution::parse("ignore"),
        Some(DuplicateResolution::Ignore)
    );
    assert_eq!(DuplicateResolution::parse("x"), None);
}
