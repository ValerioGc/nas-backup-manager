use super::*;
use crate::{
    rules::{
        tests::scheduled_rule, ArchiveConfiguration, BackupRule, BatchRenameConfiguration,
        FileOperationConfigurations, RuleFolders,
    },
    test_support::{state_with, Scratch},
    AppState,
};
use axum::{extract::State, Json};
use std::sync::Mutex as StdMutex;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixListener,
};

async fn create(state: &AppState, rule: BackupRule) {
    let _ = rules::create_rule(State(state.clone()), Json(rule))
        .await
        .expect("created");
}

async fn last(state: &AppState, id: &str) -> rules::LastRun {
    rules::list_rules(State(state.clone()))
        .await
        .expect("listed")
        .0
        .into_iter()
        .find(|view| view.rule.id == id)
        .and_then(|view| view.last_run)
        .expect("a last run")
}

async fn run(state: &AppState, id: &str) {
    Runner::start(&state.runner, id, None)
        .await
        .expect("started")
        .await
        .expect("ended");
}

#[tokio::test]
async fn runs_a_rule_and_writes_how_it_went() {
    let disks = Scratch::new("runner-ok");
    disks.write("Photos/a.jpg", "a");
    disks.write("Photos/sub/b.jpg", "b");
    let state = state_with(disks.path(), None).await;
    let rename = BatchRenameConfiguration {
        filename: "x".to_owned(),
        separator: "-".to_owned(),
        prefix: "copy".to_owned(),
        position: "prefix".to_owned(),
    };
    create(
        &state,
        BackupRule {
            operations: vec![FileOperation::BatchRename],
            operation_configurations: FileOperationConfigurations {
                batch_rename: Some(rename),
                ..FileOperationConfigurations::default()
            },
            ..scheduled_rule("photos")
        },
    )
    .await;

    run(&state, "photos").await;
    let outcome = last(&state, "photos").await;
    assert_eq!((outcome.status.as_str(), outcome.message), ("ok", None));
    assert!(outcome.at > 0);
    assert_eq!(
        std::fs::read_to_string(disks.path().join("Backup/photos/sub/copy-b.jpg")).expect("copy"),
        "b"
    );
    assert!(!state.runner.is_running("photos"));
}

#[tokio::test]
async fn a_run_that_cannot_work_says_why() {
    let disks = Scratch::new("runner-failed");
    let state = state_with(disks.path(), None).await;
    create(&state, scheduled_rule("nowhere")).await;
    run(&state, "nowhere").await;
    let outcome = last(&state, "nowhere").await;
    assert_eq!(outcome.status, "failed");
    assert!(outcome.message.expect("why").contains("/Photos"));

    create(
        &state,
        BackupRule {
            rule_type: "cloud".to_owned(),
            folders: RuleFolders {
                cloud: Some("/mnt/share".to_owned()),
                ..RuleFolders::default()
            },
            ..scheduled_rule("cloudy")
        },
    )
    .await;
    let refused = Runner::start(&state.runner, "cloudy", None)
        .await
        .expect_err("not yet");
    assert_eq!(refused.status, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn archives_through_its_run_and_remembers_the_archive() {
    let has_seven_zip = std::process::Command::new("7z")
        .arg("i")
        .output()
        .is_ok_and(|output| output.status.success());
    if !has_seven_zip {
        return;
    }
    let disks = Scratch::new("runner-archive");
    disks.write("Photos/a.jpg", "a");
    let state = state_with(disks.path(), None).await;
    create(
        &state,
        BackupRule {
            operations: vec![FileOperation::Archive],
            operation_configurations: FileOperationConfigurations {
                archive: Some(ArchiveConfiguration {
                    archive_name: "photos".to_owned(),
                    password: "pw".to_owned(),
                    encrypt_file_info: false,
                    verify_archive: false,
                }),
                ..FileOperationConfigurations::default()
            },
            ..scheduled_rule("archived")
        },
    )
    .await;
    run(&state, "archived").await;
    assert_eq!(last(&state, "archived").await.status, "ok");
    assert!(disks.path().join("Backup/photos/photos.7z").is_file());
    let stored = rules::load(&state.database, &state.secret_cipher, "archived")
        .await
        .expect("loaded")
        .expect("there");
    assert_eq!(stored.archive_path.as_deref(), Some("photos.7z"));
    assert!(stored.archive_fingerprint.is_some());
    assert!(
        !disks.path().join(".work/links/archived").exists(),
        "links cleaned up"
    );
}

/// A launcher answering on a socket: what it was asked, and the state it reports for the tasks.
fn fake_launcher(socket: &std::path::Path, task_state: &'static str) -> Arc<StdMutex<Vec<String>>> {
    let listener = UnixListener::bind(socket).expect("socket");
    let seen = Arc::new(StdMutex::new(Vec::new()));
    let kept = Arc::clone(&seen);
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let mut buffer = vec![0; 16384];
            let read = stream.read(&mut buffer).await.unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
            let body = if request.starts_with("GET /api/tasks") {
                let id = kept
                    .lock()
                    .expect("seen")
                    .iter()
                    .find_map(|seen: &String| {
                        seen.split("\"id\":\"")
                            .nth(1)
                            .map(|rest| rest.split('"').next().unwrap_or("").to_owned())
                    })
                    .unwrap_or_default();
                format!(r#"[{{"id":"{id}","state":"{task_state}"}}]"#)
            } else {
                "{}".to_owned()
            };
            kept.lock().expect("seen").push(request);
            let answer = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(answer.as_bytes()).await;
        }
    });
    seen
}

#[tokio::test]
async fn shows_its_runs_to_the_launcher_and_says_when_it_works_in_background() {
    let disks = Scratch::new("runner-launcher");
    disks.write("Photos/a.jpg", "a");
    let socket = disks.path().join("launcher.sock");
    let seen = fake_launcher(&socket, "running");
    let state = state_with(disks.path(), Some(socket)).await;

    create(&state, scheduled_rule("shown")).await;
    run(&state, "shown").await;
    rules::switch_rule(
        State(state.clone()),
        axum::extract::Path("shown".to_owned()),
        Json(rules::Switch { enabled: false }),
    )
    .await
    .expect("off");

    let seen = seen.lock().expect("seen").clone();
    let backgrounds: Vec<&String> = seen
        .iter()
        .filter(|request| request.starts_with("PUT /api/apps/self/background"))
        .collect();
    assert!(backgrounds[0].ends_with(r#"{"needed":true}"#));
    assert!(backgrounds
        .last()
        .expect("off")
        .ends_with(r#"{"needed":false}"#));
    assert!(seen.iter().any(
        |request| request.starts_with("POST /api/tasks") && request.contains("Backup: Photos")
    ));
    assert!(seen
        .iter()
        .any(|request| request.starts_with("PATCH /api/tasks/")
            && request.contains(r#""state":"done""#)));
}

#[tokio::test]
async fn a_run_stopped_from_the_launcher_is_interrupted() {
    let disks = Scratch::new("runner-cancelled");
    for index in 0..40 {
        disks.write(&format!("Photos/{index}.bin"), &"x".repeat(1 << 16));
    }
    let socket = disks.path().join("launcher.sock");
    fake_launcher(&socket, "cancelled");
    let slow = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/unit/axum/fixtures/slow-7z"
    );
    let mut state = state_with(disks.path(), Some(socket.clone())).await;
    state.runner = Arc::new(
        Runner::new(
            state.database.clone(),
            Arc::clone(&state.secret_cipher),
            Launcher::new(Some(socket)),
            Disks::Fixed(Storage::Root(disks.path().to_path_buf())),
            disks.path().join(".work"),
        )
        .with_seven_zip(slow),
    );
    create(
        &state,
        BackupRule {
            operations: vec![FileOperation::Archive],
            operation_configurations: FileOperationConfigurations {
                archive: Some(ArchiveConfiguration {
                    archive_name: "slow".to_owned(),
                    password: "pw".to_owned(),
                    encrypt_file_info: false,
                    verify_archive: false,
                }),
                ..FileOperationConfigurations::default()
            },
            ..scheduled_rule("stopped")
        },
    )
    .await;
    run(&state, "stopped").await;
    assert_eq!(last(&state, "stopped").await.status, "interrupted");
}

#[tokio::test]
async fn stops_every_run_when_the_application_stops() {
    let disks = Scratch::new("runner-shutdown");
    disks.write("Photos/a.jpg", "a");
    let slow = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/unit/axum/fixtures/slow-7z"
    );
    let mut state = state_with(disks.path(), None).await;
    state.runner = Arc::new(
        Runner::new(
            state.database.clone(),
            Arc::clone(&state.secret_cipher),
            Launcher::new(None),
            Disks::Fixed(Storage::Root(disks.path().to_path_buf())),
            disks.path().join(".work"),
        )
        .with_seven_zip(slow),
    );
    create(
        &state,
        BackupRule {
            operations: vec![FileOperation::Archive],
            operation_configurations: FileOperationConfigurations {
                archive: Some(ArchiveConfiguration {
                    archive_name: "slow".to_owned(),
                    password: "pw".to_owned(),
                    encrypt_file_info: false,
                    verify_archive: false,
                }),
                ..FileOperationConfigurations::default()
            },
            ..scheduled_rule("long")
        },
    )
    .await;
    let handle = Runner::start(&state.runner, "long", None)
        .await
        .expect("started");
    tokio::time::sleep(Duration::from_millis(200)).await;
    state.runner.shutdown().await;
    handle.await.expect("ended");
    assert_eq!(last(&state, "long").await.status, "interrupted");
    assert!(!state.runner.stop("long"));
}

#[tokio::test]
async fn a_real_time_rule_copies_what_changes_in_its_origin() {
    let disks = Scratch::new("runner-watch");
    disks.write("Photos/before.txt", "before");
    let state = state_with(disks.path(), None).await;
    create(
        &state,
        BackupRule {
            rule_type: "rtl".to_owned(),
            schedule: None,
            ..scheduled_rule("live")
        },
    )
    .await;
    let watcher = tokio::spawn(crate::watcher::run(Arc::clone(&state.runner)));
    let copy = |name: &str| std::fs::read_to_string(disks.path().join("Backup/photos").join(name));
    let wait = |name: &'static str, expected: Option<&'static str>| {
        let copy = &copy;
        async move {
            for _ in 0..100 {
                if copy(name).ok().as_deref() == expected {
                    return true;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            false
        }
    };
    assert!(
        wait("before.txt", Some("before")).await,
        "the first run reads the whole origin"
    );
    disks.write("Photos/after.txt", "after");
    assert!(
        wait("after.txt", Some("after")).await,
        "a new file is copied"
    );
    std::fs::remove_file(disks.path().join("Photos/before.txt")).expect("gone");
    assert!(wait("before.txt", None).await, "a mirror follows a removal");

    // Turned off, the rule no longer follows its origin.
    rules::switch_rule(
        State(state.clone()),
        axum::extract::Path("live".to_owned()),
        Json(rules::Switch { enabled: false }),
    )
    .await
    .expect("off");
    tokio::time::sleep(Duration::from_millis(300)).await;
    disks.write("Photos/late.txt", "late");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(copy("late.txt").is_err());
    watcher.abort();
}

#[test]
fn describes_where_a_run_is() {
    let progress = Progress::default();
    progress.copied.store(3, Ordering::Relaxed);
    assert_eq!(detail(&progress), "3 file");
    progress.removed.store(2, Ordering::Relaxed);
    assert_eq!(detail(&progress), "3 file, 2 rimossi");
}

#[tokio::test]
async fn names_an_archive_after_the_local_time() {
    let stamp = stamp().await;
    assert_eq!(stamp.len(), 13);
    assert_eq!(&stamp[8..9], "-");
}

#[tokio::test]
async fn a_network_origin_gone_empty_never_empties_its_mirror() {
    let share = Scratch::new("runner-share");
    let backup = Scratch::new("runner-backup");
    let state = state_with(share.path(), None).await;
    let folder = |path: &std::path::Path, network: bool| storage::Folder {
        path: path.to_path_buf(),
        volume: path.to_path_buf(),
        network,
    };
    let (origin, destination) = (folder(share.path(), true), folder(backup.path(), false));
    create(&state, scheduled_rule("rule")).await;

    // Nothing copied yet: an empty share has nothing to protect.
    assert!(state
        .runner
        .network_ready("rule", &origin, &destination, true)
        .await
        .is_ok());
    sqlx::query(
        "INSERT INTO rule_files (rule_id, parent, name, destination, size, modified)
         VALUES ('rule', '', 'a', 'a', 1, 1)",
    )
    .execute(&state.database)
    .await
    .expect("a copy");
    let stopped = state
        .runner
        .network_ready("rule", &origin, &destination, true)
        .await;
    assert!(matches!(stopped, Err(Stop::Failed(reason)) if reason.contains("nothing was removed")));
    // A copy without a mirror removes nothing, and a share with files goes on.
    assert!(state
        .runner
        .network_ready("rule", &origin, &destination, false)
        .await
        .is_ok());
    share.write("b.txt", "b");
    assert!(state
        .runner
        .network_ready("rule", &origin, &destination, true)
        .await
        .is_ok());
    let gone = folder(&share.path().join("missing"), true);
    assert!(state
        .runner
        .network_ready("rule", &origin, &gone, true)
        .await
        .is_err());
}
