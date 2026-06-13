//! User-initiated bridge files for external MCP launchers.
//!
//! Shell launchers cannot decrypt AgentDeck's encrypted store. When the user
//! saves an xAI key in the app, we optionally mirror it to a mode-0600 env
//! file that only the grok-mcp launcher reads.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::Serialize;

use crate::secrets;
use crate::storage;

pub const GROK_MCP_BRIDGE_FILE: &str = "grok-mcp.env";
const XAI_SLOT_ID: &str = "xai";
const XAI_ENV_KEY: &str = "XAI_API_KEY";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokMcpBridgeStatus {
    pub path: String,
    pub exists: bool,
    pub has_key: bool,
    pub updated_at: Option<String>,
    pub detail: String,
}

pub fn bridge_path_for_database(database_path: &Path) -> Result<PathBuf, String> {
    database_path
        .parent()
        .map(|parent| parent.join(GROK_MCP_BRIDGE_FILE))
        .ok_or_else(|| "database path has no parent directory".to_owned())
}

pub fn bridge_status_at(database_path: &Path) -> Result<GrokMcpBridgeStatus, String> {
    let bridge_path = bridge_path_for_database(database_path)?;
    let exists = bridge_path.exists();
    let has_key = if exists {
        read_xai_key_from_bridge(&bridge_path)?.is_some()
    } else {
        false
    };
    let updated_at = if exists {
        fs::metadata(&bridge_path)
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .map(|modified| {
                let datetime: chrono::DateTime<Utc> = modified.into();
                datetime.to_rfc3339()
            })
    } else {
        None
    };
    let detail = if has_key {
        "Bridge file contains an xAI API key for the grok-mcp launcher.".to_owned()
    } else if exists {
        "Bridge file exists but does not contain XAI_API_KEY.".to_owned()
    } else {
        "Bridge file not written yet. Save an xAI key or run Sync bridge.".to_owned()
    };
    Ok(GrokMcpBridgeStatus {
        path: bridge_path.display().to_string(),
        exists,
        has_key,
        updated_at,
        detail,
    })
}

pub fn sync_grok_mcp_bridge(database_path: &Path) -> Result<GrokMcpBridgeStatus, String> {
    let bridge_path = bridge_path_for_database(database_path)?;
    if let Some(api_key) = read_xai_secret(database_path)? {
        write_bridge_file(&bridge_path, &api_key)?;
    } else {
        clear_bridge_file(&bridge_path)?;
    }
    bridge_status_at(database_path)
}

pub fn read_xai_secret_for_research(database_path: &Path) -> Result<String, String> {
    let api_key = read_xai_secret(database_path)?;
    api_key.ok_or_else(|| {
        "xAI API key is not configured. Save a key in AgentDeck Settings or sync the Grok MCP bridge."
            .to_owned()
    })
}

fn read_xai_secret(database_path: &Path) -> Result<Option<String>, String> {
    let Some(ciphertext) = storage::read_provider_secret(database_path, XAI_SLOT_ID)? else {
        return Ok(None);
    };
    let master = secrets::load_master_key(database_path)?;
    let secret = secrets::decrypt(&master, &ciphertext)?;
    Ok(Some(secret))
}

fn write_bridge_file(path: &Path, api_key: &str) -> Result<(), String> {
    validate_env_value(api_key)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create bridge directory: {error}"))?;
    }
    let contents = format!(
        "# Written by AgentDeck for the grok-mcp launcher.\n# Regenerated when xAI credentials change.\n{}\n",
        format_env_assignment(XAI_ENV_KEY, api_key)
    );
    write_restricted_file(path, contents.as_bytes())
}

fn clear_bridge_file(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path)
            .map_err(|error| format!("failed to remove bridge file: {error}"))?;
    }
    Ok(())
}

fn read_xai_key_from_bridge(path: &Path) -> Result<Option<String>, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read bridge file: {error}"))?;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(value) = parse_env_assignment(trimmed, XAI_ENV_KEY) {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

fn parse_env_assignment(line: &str, key: &str) -> Option<String> {
    let (name, value) = line.split_once('=')?;
    if name.trim() != key {
        return None;
    }
    parse_env_value(value.trim())
}

fn parse_env_value(raw: &str) -> Option<String> {
    if raw.is_empty() {
        return Some(String::new());
    }
    if raw.starts_with('\'') && raw.ends_with('\'') && raw.len() >= 2 {
        return Some(unescape_single_quoted(&raw[1..raw.len() - 1]));
    }
    if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
        return Some(unescape_double_quoted(&raw[1..raw.len() - 1]));
    }
    Some(raw.to_owned())
}

fn unescape_single_quoted(value: &str) -> String {
    value.replace("'\\''", "'")
}

fn unescape_double_quoted(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => output.push('\n'),
                Some('t') => output.push('\t'),
                Some('\\') => output.push('\\'),
                Some('"') => output.push('"'),
                Some(other) => {
                    output.push('\\');
                    output.push(other);
                }
                None => output.push('\\'),
            }
        } else {
            output.push(ch);
        }
    }
    output
}

fn validate_env_value(value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err("xAI API key is empty".to_owned());
    }
    if value.contains('\n') || value.contains('\0') {
        return Err("xAI API key contains unsupported characters".to_owned());
    }
    Ok(())
}

fn format_env_assignment(key: &str, value: &str) -> String {
    format!("{key}='{}'", value.replace('\'', "'\\''"))
}

#[cfg(unix)]
fn write_restricted_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| format!("failed to open bridge file: {error}"))?;
    file.write_all(bytes)
        .map_err(|error| format!("failed to write bridge file: {error}"))?;
    let _ = file.sync_all();
    Ok(())
}

#[cfg(not(unix))]
fn write_restricted_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|error| format!("failed to write bridge file: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_database_path(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "agentdeck-bridge-{label}-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("agentdeck.sqlite3")
    }

    #[test]
    fn sync_writes_and_reads_bridge_file() {
        let db_path = temp_database_path("write");
        let master = secrets::master_key(&db_path).unwrap();
        let ciphertext = secrets::encrypt(&master, "xai-test-key-12345678").unwrap();
        storage::store_provider_secret(&db_path, XAI_SLOT_ID, &ciphertext).unwrap();

        let status = sync_grok_mcp_bridge(&db_path).unwrap();
        assert!(status.exists);
        assert!(status.has_key);

        let bridge_path = bridge_path_for_database(&db_path).unwrap();
        let key = read_xai_key_from_bridge(&bridge_path).unwrap();
        assert_eq!(key.as_deref(), Some("xai-test-key-12345678"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::metadata(&bridge_path).unwrap().permissions();
            assert_eq!(perms.mode() & 0o777, 0o600);
        }

        let _ = std::fs::remove_dir_all(db_path.parent().unwrap());
    }

    #[test]
    fn sync_clears_bridge_when_secret_missing() {
        let db_path = temp_database_path("clear");
        let bridge_path = bridge_path_for_database(&db_path).unwrap();
        write_bridge_file(&bridge_path, "stale-key-12345678").unwrap();

        let status = sync_grok_mcp_bridge(&db_path).unwrap();
        assert!(!status.exists);
        assert!(!status.has_key);

        let _ = std::fs::remove_dir_all(db_path.parent().unwrap());
    }

    #[test]
    #[ignore = "live smoke: writes grok-mcp.env in home app support"]
    fn live_smoke_sync_home_grok_bridge() {
        let path = storage::home_database_path().expect("home database path");
        let status = sync_grok_mcp_bridge(&path).expect("sync home bridge");
        eprintln!("bridge status: {status:?}");
        if status.has_key {
            let bridge_path = bridge_path_for_database(&path).expect("bridge path");
            let key = read_xai_key_from_bridge(&bridge_path).expect("read bridge");
            assert!(key.as_ref().is_some_and(|value| value.len() >= 8));
        }
    }

    #[test]
    fn escapes_single_quotes_in_env_assignment() {
        assert_eq!(
            format_env_assignment("XAI_API_KEY", "abc'def"),
            "XAI_API_KEY='abc'\\''def'"
        );
        assert_eq!(
            parse_env_assignment("XAI_API_KEY='abc'\\''def'", "XAI_API_KEY").as_deref(),
            Some("abc'def")
        );
    }
}