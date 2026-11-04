use crate::{
    files::storage::Storage,
    launcher::client::Launcher,
    runs::runner::{Disks, Runner},
    AppState, MIGRATOR,
};
use aes_gcm::{Aes256Gcm, KeyInit};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

pub fn unique_temp(name: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("a clock after 1970")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "nas-backup-{name}-{}-{nonce}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new(name: &str) -> Self {
        let path = unique_temp(name);
        std::fs::create_dir_all(&path).expect("scratch folder");
        Self(path.canonicalize().expect("scratch path"))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn write(&self, relative: &str, content: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("folders");
        std::fs::write(&path, content).expect("file");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub async fn database() -> SqlitePool {
    let options = "sqlite::memory:"
        .parse::<SqliteConnectOptions>()
        .expect("valid in-memory SQLite URL")
        .foreign_keys(true);
    let database = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("open in-memory database");
    MIGRATOR.run(&database).await.expect("apply migrations");
    database
}

pub fn cipher() -> Arc<Aes256Gcm> {
    Arc::new(Aes256Gcm::new_from_slice(&[0; 32]).expect("valid test encryption key"))
}

pub async fn state_with(root: &Path, socket: Option<PathBuf>) -> AppState {
    let database = database().await;
    let secret_cipher = cipher();
    let runner = Arc::new(Runner::new(
        database.clone(),
        Arc::clone(&secret_cipher),
        Launcher::new(socket),
        Disks::Fixed(Storage::Root(root.to_path_buf())),
        root.join(".work"),
    ));
    AppState {
        ui_dir: PathBuf::new(),
        database,
        secret_cipher,
        runner,
    }
}

pub async fn state() -> AppState {
    state_with(&std::env::temp_dir(), None).await
}
