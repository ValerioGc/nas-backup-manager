use super::*;
use axum::body::to_bytes;
use std::{
    fs,
    os::unix::net::UnixListener as StdUnixListener,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = env::temp_dir().join(format!(
        "nas-backup-manager-main-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

async fn state(ui_dir: PathBuf) -> AppState {
    let (database, secret_cipher) = open_database(&ui_dir.join("data/rules.sqlite3"))
        .await
        .unwrap();
    let mut state = crate::test_support::state().await;
    state.ui_dir = ui_dir;
    state.database = database;
    state.secret_cipher = secret_cipher;
    state
}

#[tokio::test]
async fn health_answers_with_the_version() {
    let response = health().await.into_response();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["status"], "ok");
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn api_errors_answer_with_their_status_and_message() {
    let refused = ApiError::bad_request("no").into_response();
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    let failed = ApiError::internal("disk").into_response();
    assert_eq!(failed.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn serves_the_interface_and_nothing_outside_it() {
    let dir = temp_dir();
    fs::write(dir.join("index.html"), "home").unwrap();
    fs::write(dir.join("app.js"), "script").unwrap();
    fs::write(dir.join("data.bin"), "data").unwrap();
    let state = State(state(dir.clone()).await);

    for (uri, status) in [
        ("/api/rules", StatusCode::NOT_FOUND),
        ("/api", StatusCode::NOT_FOUND),
        ("/../secret", StatusCode::BAD_REQUEST),
        ("/missing.css", StatusCode::NOT_FOUND),
    ] {
        let response = static_file(state.clone(), uri.parse().unwrap()).await;
        assert_eq!(response.status(), status, "{uri}");
    }
    for (uri, mime) in [
        ("/", "text/html; charset=utf-8"),
        ("/app.js", "text/javascript; charset=utf-8"),
        ("/data.bin", "application/octet-stream"),
    ] {
        let response = static_file(state.clone(), uri.parse().unwrap()).await;
        assert_eq!(response.headers()[header::CONTENT_TYPE], mime, "{uri}");
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "no-cache",
            "{uri}"
        );
    }
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::write(dir.join("assets/index-abc.js"), "script").unwrap();
    let asset = static_file(state.clone(), "/assets/index-abc.js".parse().unwrap()).await;
    assert_eq!(
        asset.headers()[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn prepares_the_socket_only_over_a_stale_one() {
    let dir = temp_dir();
    let file = dir.join("occupied");
    fs::write(&file, "file").unwrap();
    assert!(prepare_socket(&file)
        .await
        .unwrap_err()
        .contains("occupied"));

    let live = dir.join("live.sock");
    let listener = UnixListener::bind(&live).unwrap();
    assert!(prepare_socket(&live)
        .await
        .unwrap_err()
        .contains("already listening"));
    drop(listener);

    let stale = dir.join("stale.sock");
    drop(StdUnixListener::bind(&stale).unwrap());
    assert!(prepare_socket(&stale).await.is_ok());
    assert!(!stale.exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn finds_the_database_from_the_configuration_or_the_state_directory() {
    assert_eq!(
        database_path(Some("/data/rules.db".into()), None).unwrap(),
        PathBuf::from("/data/rules.db")
    );
    assert_eq!(
        database_path(
            Some("".into()),
            Some("/var/lib/nas-app/backup:/other".into())
        )
        .unwrap(),
        PathBuf::from("/var/lib/nas-app/backup/backup-manager.sqlite3")
    );
    assert!(database_path(None, None)
        .unwrap()
        .ends_with("data/backup-manager.sqlite3"));
}

#[test]
fn finds_the_interface_configured_packaged_or_built() {
    let configured = PathBuf::from("/custom/ui");
    assert_eq!(
        ui_dir_for(Some(configured.clone()), Ok(PathBuf::from("/bin/app"))).unwrap(),
        configured
    );

    let dir = temp_dir();
    let packaged = dir.join("package/dist");
    fs::create_dir_all(&packaged).unwrap();
    fs::write(packaged.join("index.html"), "packaged").unwrap();
    assert_eq!(
        ui_dir_for(None, Ok(dir.join("package/bin/app"))).unwrap(),
        packaged
    );
    assert!(ui_dir_for(None, Ok(dir.join("bin/app")))
        .unwrap()
        .ends_with("dist/axum"));
    assert!(ui_dir_for(None, Err("no executable".to_owned())).is_err());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn reads_the_socket_from_the_arguments_or_the_environment() {
    for (args, error) in [
        (vec!["--listen".into()], "requires a socket path"),
        (vec!["--other".into()], "Unrecognized argument"),
        (vec![], "Provide"),
        (vec!["--listen".into(), "".into()], "Empty socket path"),
    ] {
        assert!(
            socket_path(args, None).unwrap_err().contains(error),
            "{error}"
        );
    }
    assert_eq!(
        socket_path(vec![], Some("/run/app.sock".into())).unwrap(),
        PathBuf::from("/run/app.sock")
    );
}

#[tokio::test]
async fn serves_on_its_socket_and_removes_it_when_stopped() {
    let dir = temp_dir();
    let ui = dir.join("ui");
    fs::create_dir_all(&ui).unwrap();
    fs::write(ui.join("index.html"), "home").unwrap();
    let socket = dir.join("app.sock");
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(run(
        vec![],
        Some(socket.clone().into_os_string()),
        Some(ui),
        dir.join("rules.sqlite3"),
        async {
            let _ = stopped.await;
        },
    ));
    for _ in 0..200 {
        if socket.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(socket.exists());
    stop.send(()).unwrap();
    server.await.unwrap().unwrap();
    assert!(!socket.exists());
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn refuses_to_start_without_an_interface_or_with_its_socket_taken() {
    let dir = temp_dir();
    let missing = run(
        vec![],
        Some(dir.join("app.sock").into_os_string()),
        Some(dir.join("ui")),
        dir.join("rules.sqlite3"),
        std::future::pending(),
    )
    .await;
    assert!(missing
        .unwrap_err()
        .to_string()
        .contains("Frontend not found"));

    let ui = dir.join("ui");
    fs::create_dir_all(&ui).unwrap();
    fs::write(ui.join("index.html"), "home").unwrap();
    let occupied = dir.join("occupied.sock");
    fs::write(&occupied, "file").unwrap();
    let taken = run(
        vec![],
        Some(occupied.into_os_string()),
        Some(ui),
        dir.join("rules.sqlite3"),
        std::future::pending(),
    )
    .await;
    assert!(taken.unwrap_err().to_string().contains("occupied"));
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn names_the_disks_it_reaches_and_none_without_a_registry() {
    let disks = crate::test_support::Scratch::new("main-volumes");
    let state = crate::test_support::state_with(disks.path(), None).await;
    let Json(names) = volumes(State(state)).await;
    assert!(names.is_empty());
}
