//! App-managed, encrypted-at-rest secret store.
//!
//! Provider API keys are encrypted with AES-256-GCM under a device-local master
//! key stored alongside the SQLite database (`secret.key`, mode 0600 on Unix).
//! This deliberately avoids the macOS login Keychain: the Keychain binds each
//! item's ACL to the requesting binary's code signature, so unsigned/ad-hoc dev
//! builds re-prompt on every rebuild. App-managed encryption is stable across
//! rebuilds and never triggers an OS prompt.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use base64::Engine;
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM, NONCE_LEN};
use ring::rand::{SecureRandom, SystemRandom};

const MASTER_KEY_LEN: usize = 32;
const GCM_TAG_LEN: usize = 16;
const SECRET_KEY_FILE: &str = "secret.key";

fn master_key_path(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .map(|parent| parent.join(SECRET_KEY_FILE))
        .unwrap_or_else(|| PathBuf::from(SECRET_KEY_FILE))
}

/// Load the device master key from disk, creating it on first use.
pub fn master_key(db_path: &Path) -> Result<[u8; MASTER_KEY_LEN], String> {
    let path = master_key_path(db_path);
    if path.exists() {
        return load_master_key(db_path);
    }

    let rng = SystemRandom::new();
    let mut key = [0u8; MASTER_KEY_LEN];
    rng.fill(&mut key)
        .map_err(|_| "failed to generate device master key".to_owned())?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create secret directory: {error}"))?;
    }
    write_key_file(&path, &key)?;
    Ok(key)
}

/// Load an existing device master key without creating one.
pub fn load_master_key(db_path: &Path) -> Result<[u8; MASTER_KEY_LEN], String> {
    let path = master_key_path(db_path);
    let bytes = fs::read(&path)
        .map_err(|error| format!("failed to read device master key: {error}"))?;
    if bytes.len() != MASTER_KEY_LEN {
        return Err(format!(
            "device master key has invalid length: expected {MASTER_KEY_LEN} bytes, found {}",
            bytes.len()
        ));
    }
    let mut key = [0u8; MASTER_KEY_LEN];
    key.copy_from_slice(&bytes);
    Ok(key)
}

#[cfg(unix)]
fn write_key_file(path: &Path, key: &[u8]) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| format!("failed to open secret key file: {error}"))?;
    file.write_all(key)
        .map_err(|error| format!("failed to write secret key: {error}"))?;
    let _ = file.sync_all();
    Ok(())
}

#[cfg(not(unix))]
fn write_key_file(path: &Path, key: &[u8]) -> Result<(), String> {
    let mut file = fs::File::create(path)
        .map_err(|error| format!("failed to open secret key file: {error}"))?;
    file.write_all(key)
        .map_err(|error| format!("failed to write secret key: {error}"))?;
    let _ = file.sync_all();
    Ok(())
}

/// Encrypt a plaintext secret. Output is `base64(nonce ‖ ciphertext ‖ tag)`.
pub fn encrypt(master: &[u8; MASTER_KEY_LEN], plaintext: &str) -> Result<String, String> {
    let unbound = UnboundKey::new(&AES_256_GCM, master)
        .map_err(|_| "failed to initialize cipher".to_owned())?;
    let key = LessSafeKey::new(unbound);

    let rng = SystemRandom::new();
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rng.fill(&mut nonce_bytes)
        .map_err(|_| "failed to generate nonce".to_owned())?;
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);

    let mut in_out = plaintext.as_bytes().to_vec();
    key.seal_in_place_append_tag(nonce, Aad::empty(), &mut in_out)
        .map_err(|_| "failed to encrypt secret".to_owned())?;

    let mut combined = Vec::with_capacity(NONCE_LEN + in_out.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&in_out);
    Ok(base64::engine::general_purpose::STANDARD.encode(combined))
}

/// Decrypt a secret produced by [`encrypt`]. Fails on tamper or wrong key.
pub fn decrypt(master: &[u8; MASTER_KEY_LEN], encoded: &str) -> Result<String, String> {
    let combined = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| format!("invalid stored secret encoding: {error}"))?;
    if combined.len() < NONCE_LEN + GCM_TAG_LEN {
        return Err("stored secret is truncated".to_owned());
    }

    let (nonce_bytes, ciphertext) = combined.split_at(NONCE_LEN);
    let mut nonce_arr = [0u8; NONCE_LEN];
    nonce_arr.copy_from_slice(nonce_bytes);
    let nonce = Nonce::assume_unique_for_key(nonce_arr);

    let unbound = UnboundKey::new(&AES_256_GCM, master)
        .map_err(|_| "failed to initialize cipher".to_owned())?;
    let key = LessSafeKey::new(unbound);

    let mut buffer = ciphertext.to_vec();
    let plaintext = key
        .open_in_place(nonce, Aad::empty(), &mut buffer)
        .map_err(|_| "failed to decrypt stored secret".to_owned())?;
    String::from_utf8(plaintext.to_vec())
        .map_err(|_| "decrypted secret is not valid UTF-8".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; MASTER_KEY_LEN] {
        let mut key = [0u8; MASTER_KEY_LEN];
        for (index, byte) in key.iter_mut().enumerate() {
            *byte = index as u8;
        }
        key
    }

    #[test]
    fn round_trips_secret() {
        let key = test_key();
        let encoded = encrypt(&key, "sk-test-1234567890").unwrap();
        assert_ne!(encoded, "sk-test-1234567890");
        assert_eq!(decrypt(&key, &encoded).unwrap(), "sk-test-1234567890");
    }

    #[test]
    fn distinct_nonces_produce_distinct_ciphertext() {
        let key = test_key();
        let first = encrypt(&key, "same-secret").unwrap();
        let second = encrypt(&key, "same-secret").unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn rejects_wrong_key() {
        let encoded = encrypt(&test_key(), "secret").unwrap();
        let mut other = test_key();
        other[0] ^= 0xff;
        assert!(decrypt(&other, &encoded).is_err());
    }

    #[test]
    fn rejects_tampered_ciphertext() {
        let key = test_key();
        let encoded = encrypt(&key, "secret").unwrap();
        let mut raw = base64::engine::general_purpose::STANDARD
            .decode(&encoded)
            .unwrap();
        let last = raw.len() - 1;
        raw[last] ^= 0xff;
        let tampered = base64::engine::general_purpose::STANDARD.encode(raw);
        assert!(decrypt(&key, &tampered).is_err());
    }

    #[test]
    fn creates_master_key_with_restrictive_permissions() {
        let dir = std::env::temp_dir().join(format!(
            "agentdeck-secrets-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("agentdeck.sqlite3");
        let key = master_key(&db_path).unwrap();
        // Stable across calls
        assert_eq!(master_key(&db_path).unwrap(), key);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::metadata(dir.join(SECRET_KEY_FILE))
                .unwrap()
                .permissions();
            assert_eq!(perms.mode() & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn loading_missing_master_key_does_not_create_one() {
        let dir = std::env::temp_dir().join(format!(
            "agentdeck-missing-secrets-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("agentdeck.sqlite3");

        assert!(load_master_key(&db_path).is_err());
        assert!(!dir.join(SECRET_KEY_FILE).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_corrupt_master_key_file() {
        let dir = std::env::temp_dir().join(format!(
            "agentdeck-corrupt-secrets-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("agentdeck.sqlite3");
        std::fs::write(dir.join(SECRET_KEY_FILE), b"short").unwrap();

        assert!(load_master_key(&db_path)
            .unwrap_err()
            .contains("invalid length"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
