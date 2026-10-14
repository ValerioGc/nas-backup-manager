use super::*;
use std::env;

#[test]
fn credential_key_is_persistent_and_private() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("valid system time")
        .as_nanos();
    let directory = env::temp_dir().join(format!(
        "nas-backup-manager-key-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).expect("create test directory");
    let database_path = directory.join("rules.sqlite3");

    let cipher = secret_cipher(&database_path).expect("create encryption key");
    let encrypted = encrypt_secret("persisted-secret", &cipher).expect("encrypt secret");
    let restored_cipher = secret_cipher(&database_path).expect("load encryption key");
    assert_eq!(
        decrypt_secret(&encrypted, &restored_cipher).expect("decrypt secret"),
        "persisted-secret"
    );
    assert_eq!(
        fs::metadata(directory.join("rules.key"))
            .expect("inspect key permissions")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );

    fs::remove_file(directory.join("rules.key")).expect("remove test key");
    fs::remove_dir(directory).expect("remove test directory");
}

#[test]
fn refuses_a_password_it_did_not_encrypt() {
    let cipher = Aes256Gcm::new_from_slice(&[0; 32]).expect("valid test encryption key");

    assert!(decrypt_secret("plain", &cipher).is_err());
    assert!(decrypt_secret("v1:00", &cipher).is_err());
    assert!(decrypt_secret(&format!("v1:{}", "00".repeat(40)), &cipher).is_err());
}

#[test]
fn refuses_a_key_that_is_not_a_file_of_32_bytes() {
    let directory =
        env::temp_dir().join(format!("nas-backup-manager-bad-key-{}", std::process::id()));
    fs::create_dir_all(&directory).expect("create test directory");

    let short = directory.join("short.sqlite3");
    fs::write(directory.join("short.key"), [0; 16]).expect("write a short key");
    assert!(secret_cipher(&short).is_err());

    let folder = directory.join("folder.sqlite3");
    fs::create_dir_all(directory.join("folder.key")).expect("create a folder key");
    assert!(secret_cipher(&folder).is_err());

    fs::remove_dir_all(directory).expect("remove test directory");
}
