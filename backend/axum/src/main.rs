mod archive;
mod filters;
mod launcher;
mod rename;
mod rules;
mod runner;
mod schedule;
mod scheduler;
mod secrets;
mod storage;
mod sync;
mod watcher;

use aes_gcm::Aes256Gcm;
use axum::{
    extract::State,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use serde_json::json;
use sqlx::{
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions},
};
use std::{
    env,
    ffi::OsString,
    fmt,
    os::unix::fs::FileTypeExt,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::net::{UnixListener, UnixStream};

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
struct AppState {
    ui_dir: PathBuf,
    database: SqlitePool,
    secret_cipher: Arc<Aes256Gcm>,
    runner: Arc<runner::Runner>,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }

    fn internal(error: impl fmt::Display) -> Self {
        eprintln!("Backup rules database error: {error}");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Unable to access backup rules".to_owned(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

/// The names of the disks the folders of the rules are on: an empty list when they cannot be read.
async fn volumes(State(state): State<AppState>) -> Json<Vec<storage::VolumeName>> {
    Json(
        state
            .runner
            .storage()
            .map(|storage| storage.names())
            .unwrap_or_default(),
    )
}

async fn health() -> impl IntoResponse {
    Json(json!({"status":"ok","version":env!("CARGO_PKG_VERSION")}))
}

type Failure = Box<dyn std::error::Error + Send + Sync>;

/// `NAS_DB_PATH` when set, otherwise the database in the first state directory of the unit.
fn database_path(
    configured: Option<OsString>,
    state_directory: Option<OsString>,
) -> Result<PathBuf, String> {
    if let Some(path) = configured.filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    if let Some(directory) = state_directory
        .and_then(|value| value.into_string().ok())
        .and_then(|value| value.split(':').next().map(str::to_owned))
        .filter(|value| !value.is_empty())
    {
        return Ok(PathBuf::from(directory).join("backup-manager.sqlite3"));
    }
    if cfg!(debug_assertions) {
        return Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/backup-manager.sqlite3"));
    }
    Err("Set NAS_DB_PATH or run under systemd with STATE_DIRECTORY.".to_owned())
}

async fn open_database(path: &Path) -> Result<(SqlitePool, Arc<Aes256Gcm>), Failure> {
    if let Some(directory) = path
        .parent()
        .filter(|directory| !directory.as_os_str().is_empty())
    {
        tokio::fs::create_dir_all(directory).await?;
    }
    let cipher = Arc::new(secrets::secret_cipher(path)?);
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    MIGRATOR.run(&pool).await?;
    Ok((pool, cipher))
}

/// `NAS_UI_DIR` when set, the `dist` of the package beside the binary, or the build of the repository.
fn ui_dir_for(
    configured: Option<PathBuf>,
    executable: Result<PathBuf, String>,
) -> Result<PathBuf, String> {
    if let Some(path) = configured {
        return Ok(path);
    }
    let executable = executable?;
    let binary_dir = executable.parent().ok_or("Invalid executable path")?;
    let packaged = binary_dir
        .parent()
        .ok_or("Invalid executable path")?
        .join("dist");
    if packaged.join("index.html").is_file() {
        Ok(packaged)
    } else {
        Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist/axum"))
    }
}

async fn static_file(State(state): State<AppState>, uri: Uri) -> Response {
    let path = uri.path();
    if path.starts_with("/api/") || path == "/api" {
        return StatusCode::NOT_FOUND.into_response();
    }
    let requested = if path == "/" {
        "index.html"
    } else {
        &path[1..]
    };
    let relative = Path::new(requested);
    if !relative
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let file = state.ui_dir.join(relative);
    let mime = match file
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
    {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    };
    // The files of `assets/` carry a hash of their content in the name: the browser keeps them for
    // good. The page that names them is asked for again every time.
    let cache = if requested.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    match tokio::fs::read(file).await {
        Ok(bytes) => (
            [(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, cache)],
            bytes,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn prepare_socket(path: &Path) -> Result<(), String> {
    if let Ok(metadata) = tokio::fs::symlink_metadata(path).await {
        if !metadata.file_type().is_socket() {
            return Err(format!("Socket path is occupied: {}", path.display()));
        }
        match UnixStream::connect(path).await {
            Ok(_) => return Err(format!("Socket is already listening: {}", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {}
            Err(error) => return Err(format!("Unable to check socket: {error}")),
        }
        tokio::fs::remove_file(path)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// The socket of `--listen`, or the one the launcher configured.
fn socket_path(args: Vec<OsString>, configured: Option<OsString>) -> Result<PathBuf, String> {
    let mut listen = configured;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--listen" {
            listen = Some(args.next().ok_or("--listen requires a socket path")?);
        } else {
            return Err(format!("Unrecognized argument: {}", arg.to_string_lossy()));
        }
    }
    let path = PathBuf::from(listen.ok_or("Provide --listen or NAS_APP_SOCKET")?);
    if path.as_os_str().is_empty() {
        return Err("Empty socket path".into());
    }
    Ok(path)
}

/// Serves the rules and the interface on the socket until `shutdown_signal` completes.
async fn run(
    args: Vec<OsString>,
    configured_socket: Option<OsString>,
    configured_ui: Option<PathBuf>,
    database: PathBuf,
    shutdown_signal: impl std::future::Future<Output = ()> + Send + 'static,
) -> Result<(), Failure> {
    let path = socket_path(args, configured_socket)?;
    let ui_dir = ui_dir_for(
        configured_ui,
        env::current_exe().map_err(|error| error.to_string()),
    )?;
    if !ui_dir.join("index.html").is_file() {
        return Err(format!("Frontend not found: {}", ui_dir.display()).into());
    }
    let work = database
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let (database, secret_cipher) = open_database(&database).await?;
    let runner = Arc::new(runner::Runner::new(
        database.clone(),
        Arc::clone(&secret_cipher),
        launcher::Launcher::from_env(),
        runner::Disks::Granted,
        work,
    ));
    let state = AppState {
        ui_dir,
        database,
        secret_cipher,
        runner: Arc::clone(&runner),
    };

    prepare_socket(&path).await?;
    let listener = UnixListener::bind(&path)?;
    let app = router(state);
    println!("nas-backup-manager listening on {}", path.display());
    // What runs without the window: the scheduled rules, the real-time ones, and the launcher
    // told whether there are any.
    let scheduler = tokio::spawn(scheduler::run(Arc::clone(&runner)));
    let watcher = tokio::spawn(watcher::run(Arc::clone(&runner)));
    tokio::spawn({
        let runner = Arc::clone(&runner);
        async move { runner.declare_background().await }
    });
    let result = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await;
    scheduler.abort();
    watcher.abort();
    runner.shutdown().await;
    if let Err(error) = tokio::fs::remove_file(&path).await {
        eprintln!("Unable to remove socket: {error}");
    }
    result?;
    Ok(())
}

/// The routes of the rules and the interface.
fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route(
            "/api/rules",
            get(rules::list_rules).post(rules::create_rule),
        )
        .route(
            "/api/rules/{id}",
            put(rules::update_rule)
                .delete(rules::delete_rule)
                .patch(rules::switch_rule),
        )
        .route("/api/rules/{id}/run", post(rules::run_rule))
        .route("/api/rules/{id}/stop", post(rules::stop_rule))
        .route("/api/volumes", get(volumes))
        .fallback(get(static_file))
        .with_state(state)
}

#[tokio::main]
async fn main() -> Result<(), Failure> {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    run(
        env::args_os().skip(1).collect(),
        env::var_os("NAS_APP_SOCKET"),
        env::var_os("NAS_UI_DIR").map(PathBuf::from),
        database_path(env::var_os("NAS_DB_PATH"), env::var_os("STATE_DIRECTORY"))?,
        async move {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = terminate.recv() => {},
            }
        },
    )
    .await
}

#[cfg(test)]
#[path = "../../tests/unit/axum/main.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/unit/axum/support.rs"]
mod test_support;
