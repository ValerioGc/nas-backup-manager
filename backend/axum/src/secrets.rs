//! The passwords of the file operations are kept encrypted with AES-256-GCM, under a key stored
//! next to the database and readable only by the application.

use crate::ApiError;
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
};

/// Opens the key of the database at `database_path`, creating it (mode 0600) the first time.
pub fn secret_cipher(database_path: &Path) -> Result<Aes256Gcm, io::Error> {
    let key_path = database_path.with_extension("key");
    let mut options = OpenOptions::new();
    options.write(true).read(true).create_new(true).mode(0o600);

    let key = match options.open(&key_path) {
        Ok(mut file) => {
            let key = Aes256Gcm::generate_key(&mut OsRng);
            file.write_all(&key)?;
            file.sync_all()?;
            key.to_vec()
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(&key_path)?;
            if !metadata.file_type().is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Backup credential key is not a regular file.",
                ));
            }
            fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))?;
            let key = fs::read(&key_path)?;
            if key.len() != 32 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Backup credential key has an invalid length.",
                ));
            }
            key
        }
        Err(error) => return Err(error),
    };

    Aes256Gcm::new_from_slice(&key).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Unable to initialize backup credential encryption.",
        )
    })
}

pub fn encrypt_secret(secret: &str, cipher: &Aes256Gcm) -> Result<String, ApiError> {
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, secret.as_bytes())
        .map_err(|_| ApiError::internal("Unable to encrypt operation password."))?;
    let mut payload = nonce.to_vec();
    payload.extend(ciphertext);
    Ok(format!("v1:{}", hex::encode(payload)))
}

pub fn decrypt_secret(secret: &str, cipher: &Aes256Gcm) -> Result<String, ApiError> {
    let encoded = secret
        .strip_prefix("v1:")
        .ok_or_else(|| ApiError::internal("Unsupported encrypted password format."))?;
    let payload = hex::decode(encoded).map_err(ApiError::internal)?;
    if payload.len() <= 12 {
        return Err(ApiError::internal("Encrypted password data is incomplete."));
    }
    let (nonce_bytes, ciphertext) = payload.split_at(12);
    let nonce_bytes: [u8; 12] = nonce_bytes
        .try_into()
        .map_err(|_| ApiError::internal("Encrypted password nonce is invalid."))?;
    let nonce = Nonce::from(nonce_bytes);
    let plaintext = cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|_| ApiError::internal("Unable to decrypt operation password."))?;
    String::from_utf8(plaintext).map_err(ApiError::internal)
}

#[cfg(test)]
#[path = "../../tests/unit/axum/secrets.rs"]
mod tests;
