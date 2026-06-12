use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::models::{
    AppSettings, AuditEventRecord, AuditEventsPage, ChatMessage, ChatPreferences, HandoffRun,
    LocalDeleteResult, LocalExportResult, SkillExecutionRecord,
};

const DEFAULT_REDACT_SENSITIVE_EXPORTS: bool = true;
const DEFAULT_CRASH_SAFE_LOGGING: bool = true;
const DEFAULT_GROK_SUBSCRIPTION_ACTIVE: bool = true;
const DEFAULT_ONBOARDING_COMPLETE: bool = false;
const DEFAULT_CHAT_PROVIDER_ID: &str = "lm-studio";

pub fn database_path(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("failed to resolve app data directory: {error}"))?;
    fs::create_dir_all(&directory)
        .map_err(|error| format!("failed to create app data directory: {error}"))?;
    Ok(directory.join("agentdeck.sqlite3"))
}

pub fn home_database_path() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or_else(|| "HOME is not set".to_owned())?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("com.agentdeck.desktop")
        .join("agentdeck.sqlite3"))
}

/// Single resolver for the local SQLite database used by all subsystems.
pub fn resolve_database_path(app: Option<&AppHandle>) -> Result<PathBuf, String> {
    if let Some(app) = app {
        return database_path(app);
    }
    let path = home_database_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create app data directory: {error}"))?;
    }
    Ok(path)
}

pub fn open_database(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create app data directory: {error}"))?;
    }
    let connection = Connection::open(path)
        .map_err(|error| format!("failed to open local database: {error}"))?;
    migrate_database(&connection)?;
    Ok(connection)
}

pub(crate) const MAX_IDENTIFIER_CHARS: usize = 128;

pub(crate) fn validate_identifier(label: &str, value: &str) -> Result<(), String> {
    let length = value.chars().count();
    if length == 0 || length > MAX_IDENTIFIER_CHARS {
        return Err(format!(
            "{label} must contain between 1 and {MAX_IDENTIFIER_CHARS} characters"
        ));
    }
    Ok(())
}

pub(crate) fn stable_hash(value: &str) -> u64 {
    value
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        })
}

pub fn load_handoff_run(connection: &Connection, run_id: &str) -> Result<HandoffRun, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, thread_id, source_agent_id, source_agent_name, target_provider_id,
                    target_provider_name, target_model_id, title, task, context, status,
                    output, error, approvals, audit_ref, created_at, updated_at
             FROM handoff_runs
             WHERE id = ?1",
        )
        .map_err(|error| format!("failed to prepare handoff run lookup: {error}"))?;
    let mut rows = statement
        .query([run_id])
        .map_err(|error| format!("failed to load handoff run: {error}"))?;
    let Some(row) = rows
        .next()
        .map_err(|error| format!("failed to iterate handoff run: {error}"))?
    else {
        return Err("handoff run was not found".to_owned());
    };
    let approvals: String = row
        .get(13)
        .map_err(|error| format!("failed to decode approvals: {error}"))?;
    Ok(HandoffRun {
        id: row
            .get(0)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        thread_id: row
            .get(1)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        source_agent_id: row
            .get(2)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        source_agent_name: row
            .get(3)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        target_provider_id: row
            .get(4)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        target_provider_name: row
            .get(5)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        target_model_id: row
            .get(6)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        title: row
            .get(7)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        task: row
            .get(8)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        context: row
            .get(9)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        status: row
            .get(10)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        output: row
            .get(11)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        error: row
            .get(12)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        approvals: serde_json::from_str::<Vec<String>>(&approvals).unwrap_or_default(),
        audit_ref: row
            .get(14)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        created_at: row
            .get(15)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        updated_at: row
            .get(16)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
    })
}

pub fn load_app_settings(path: &Path) -> Result<AppSettings, String> {
    let connection = open_database(path)?;
    Ok(AppSettings {
        redact_sensitive_exports: read_bool_setting(
            &connection,
            "redact_sensitive_exports",
            DEFAULT_REDACT_SENSITIVE_EXPORTS,
        )?,
        crash_safe_logging: read_bool_setting(
            &connection,
            "crash_safe_logging",
            DEFAULT_CRASH_SAFE_LOGGING,
        )?,
        grok_subscription_active: read_bool_setting(
            &connection,
            "grok_subscription_active",
            DEFAULT_GROK_SUBSCRIPTION_ACTIVE,
        )?,
        onboarding_complete: read_bool_setting(
            &connection,
            "onboarding_complete",
            DEFAULT_ONBOARDING_COMPLETE,
        )?,
    })
}

pub fn load_chat_preferences(path: &Path) -> Result<ChatPreferences, String> {
    let connection = open_database(path)?;
    Ok(ChatPreferences {
        last_provider_id: read_string_setting(
            &connection,
            "chat_last_provider_id",
            DEFAULT_CHAT_PROVIDER_ID,
        )?,
        last_model_id: read_string_setting(&connection, "chat_last_model_id", "")?,
    })
}

pub fn save_chat_preferences(path: &Path, preferences: &ChatPreferences) -> Result<(), String> {
    let connection = open_database(path)?;
    set_string_setting(
        &connection,
        "chat_last_provider_id",
        &preferences.last_provider_id,
    )?;
    set_string_setting(&connection, "chat_last_model_id", &preferences.last_model_id)?;
    Ok(())
}

fn provider_credential_setting_key(provider_id: &str) -> String {
    format!("provider_credential_stored:{provider_id}")
}

pub fn is_provider_credential_stored(path: &Path, provider_id: &str) -> bool {
    open_database(path)
        .ok()
        .and_then(|connection| {
            read_bool_setting(
                &connection,
                &provider_credential_setting_key(provider_id),
                false,
            )
            .ok()
        })
        .unwrap_or(false)
}

pub fn set_provider_credential_stored(
    path: &Path,
    provider_id: &str,
    stored: bool,
) -> Result<(), String> {
    let connection = open_database(path)?;
    set_bool_setting(
        &connection,
        &provider_credential_setting_key(provider_id),
        stored,
    )
}

fn provider_import_failure_key(slot_id: &str) -> String {
    format!("provider_import_failure:{slot_id}")
}

pub fn get_provider_import_failure(path: &Path, slot_id: &str) -> Option<String> {
    let connection = open_database(path).ok()?;
    let key = provider_import_failure_key(slot_id);
    let mut statement = connection
        .prepare("SELECT value FROM app_settings WHERE key = ?1")
        .ok()?;
    match statement.query_row([&key], |row| row.get::<_, String>(0)) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => None,
    }
}

pub fn set_provider_import_failure(
    path: &Path,
    slot_id: &str,
    detail: Option<&str>,
) -> Result<(), String> {
    let connection = open_database(path)?;
    let key = provider_import_failure_key(slot_id);
    match detail.filter(|value| !value.trim().is_empty()) {
        Some(value) => set_string_setting(&connection, &key, value),
        None => connection
            .execute("DELETE FROM app_settings WHERE key = ?1", params![key])
            .map_err(|error| format!("failed to clear provider import failure: {error}"))
            .map(|_| ()),
    }
}

pub fn update_app_settings(path: &Path, settings: &AppSettings) -> Result<AppSettings, String> {
    let connection = open_database(path)?;
    set_bool_setting(
        &connection,
        "redact_sensitive_exports",
        settings.redact_sensitive_exports,
    )?;
    set_bool_setting(
        &connection,
        "crash_safe_logging",
        settings.crash_safe_logging,
    )?;
    set_bool_setting(
        &connection,
        "grok_subscription_active",
        settings.grok_subscription_active,
    )?;
    set_bool_setting(
        &connection,
        "onboarding_complete",
        settings.onboarding_complete,
    )?;
    append_log_event(
        path,
        "settings.update",
        json!({
            "redactSensitiveExports": settings.redact_sensitive_exports,
            "crashSafeLogging": settings.crash_safe_logging,
            "grokSubscriptionActive": settings.grok_subscription_active,
            "onboardingComplete": settings.onboarding_complete,
        }),
    );
    Ok(settings.clone())
}

pub fn export_local_data(path: &Path) -> Result<LocalExportResult, String> {
    let connection = open_database(path)?;
    let settings = load_app_settings(path)?;
    let exported_at = Utc::now().to_rfc3339();
    let export_dir = ensure_export_dir(path)?;
    let export_path = export_dir.join(format!(
        "agentdeck-export-{}.json",
        exported_at.replace([':', '.'], "-")
    ));

    let snapshot = LocalDataExport {
        exported_at: exported_at.clone(),
        database_path: path.to_string_lossy().into_owned(),
        redact_sensitive_exports: settings.redact_sensitive_exports,
        chat_messages: load_chat_messages(&connection)?,
        handoff_runs: load_handoff_runs(&connection)?,
        audit_events: load_audit_events(&connection)?,
        plugin_settings: load_plugin_settings(&connection)?,
        skill_execution_runs: load_skill_executions(&connection)?,
        app_settings: settings.clone(),
    };

    let snapshot = if settings.redact_sensitive_exports {
        redact_snapshot(snapshot)
    } else {
        snapshot
    };

    let payload = serde_json::to_vec_pretty(&snapshot)
        .map_err(|error| format!("failed to encode export: {error}"))?;
    fs::write(&export_path, &payload)
        .map_err(|error| format!("failed to write export: {error}"))?;
    let bytes_written = payload.len() as u64;
    append_log_event(
        path,
        "data.export",
        json!({
            "path": export_path.to_string_lossy(),
            "bytesWritten": bytes_written,
            "redacted": settings.redact_sensitive_exports,
        }),
    );

    Ok(LocalExportResult {
        exported_at,
        path: export_path.to_string_lossy().into_owned(),
        redacted: settings.redact_sensitive_exports,
        bytes_written,
    })
}

pub fn delete_local_data(path: &Path) -> Result<LocalDeleteResult, String> {
    let exported_at = Utc::now().to_rfc3339();
    let mut removed_files = Vec::new();

    for candidate in database_family(path) {
        if candidate.exists() {
            fs::remove_file(&candidate)
                .map_err(|error| format!("failed to remove {}: {error}", candidate.display()))?;
            removed_files.push(candidate.to_string_lossy().into_owned());
        }
    }

    let export_dir = path
        .parent()
        .map(|parent| parent.join("exports"))
        .unwrap_or_else(|| PathBuf::from("exports"));
    if export_dir.exists() {
        fs::remove_dir_all(&export_dir)
            .map_err(|error| format!("failed to remove {}: {error}", export_dir.display()))?;
        removed_files.push(export_dir.to_string_lossy().into_owned());
    }

    let log_path = log_path(path);
    if log_path.exists() {
        fs::remove_file(&log_path)
            .map_err(|error| format!("failed to remove {}: {error}", log_path.display()))?;
        removed_files.push(log_path.to_string_lossy().into_owned());
    }

    Ok(LocalDeleteResult {
        deleted_at: exported_at,
        path: path.to_string_lossy().into_owned(),
        removed_files,
    })
}

pub fn append_log_event(path: &Path, action: &str, payload: Value) {
    if !load_app_settings(path)
        .map(|settings| settings.crash_safe_logging)
        .unwrap_or(DEFAULT_CRASH_SAFE_LOGGING)
    {
        return;
    }
    let record = json!({
        "timestamp": Utc::now().to_rfc3339(),
        "action": action,
        "payload": payload,
    });
    let Ok(serialized) = serde_json::to_string(&record) else {
        return;
    };
    let log_path = log_path(path);
    if let Some(parent) = log_path.parent() {
        if fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) else {
        return;
    };
    if writeln!(file, "{serialized}").is_err() {
        return;
    }
    let _ = file.sync_data();
}

#[derive(Debug, Serialize)]
struct LocalDataExport {
    exported_at: String,
    database_path: String,
    redact_sensitive_exports: bool,
    chat_messages: Vec<ChatMessage>,
    handoff_runs: Vec<HandoffRun>,
    audit_events: Vec<AuditEventRecord>,
    plugin_settings: Vec<PluginSettingRecord>,
    skill_execution_runs: Vec<SkillExecutionRecord>,
    app_settings: AppSettings,
}

#[derive(Debug, Serialize)]
struct PluginSettingRecord {
    plugin_id: String,
    enabled: bool,
    updated_at: String,
}

fn migrate_database(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "
            PRAGMA journal_mode = WAL;
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS chat_messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                model TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_chat_conversation
                ON chat_messages(conversation_id, created_at);
            CREATE TABLE IF NOT EXISTS audit_events (
                id TEXT PRIMARY KEY,
                action TEXT NOT NULL,
                status TEXT NOT NULL,
                model TEXT NOT NULL,
                conversation_id TEXT NOT NULL,
                duration_ms INTEGER NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_audit_created
                ON audit_events(created_at DESC);
            CREATE TABLE IF NOT EXISTS handoff_runs (
                id TEXT PRIMARY KEY,
                thread_id TEXT NOT NULL,
                source_agent_id TEXT NOT NULL,
                source_agent_name TEXT NOT NULL,
                target_provider_id TEXT NOT NULL,
                target_provider_name TEXT NOT NULL,
                target_model_id TEXT NOT NULL,
                title TEXT NOT NULL,
                task TEXT NOT NULL,
                context TEXT NOT NULL,
                status TEXT NOT NULL,
                output TEXT NOT NULL,
                error TEXT,
                approvals TEXT NOT NULL,
                audit_ref TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_handoff_thread_created
                ON handoff_runs(thread_id, created_at);
            CREATE TABLE IF NOT EXISTS plugin_settings (
                plugin_id TEXT PRIMARY KEY,
                enabled INTEGER NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS skill_execution_runs (
                id TEXT PRIMARY KEY,
                skill_id TEXT NOT NULL,
                skill_name TEXT NOT NULL,
                status TEXT NOT NULL,
                audit_ref TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS router_rules (
                id TEXT PRIMARY KEY,
                priority INTEGER NOT NULL,
                rule_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_router_priority
                ON router_rules(priority ASC);
            CREATE TABLE IF NOT EXISTS agent_permissions (
                agent_id TEXT NOT NULL,
                action TEXT NOT NULL,
                allow INTEGER NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (agent_id, action)
            );
            CREATE TABLE IF NOT EXISTS provider_secrets (
                slot_id TEXT PRIMARY KEY,
                ciphertext TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            ",
        )
        .map_err(|error| format!("failed to initialize local database: {error}"))?;
    if !migration_applied(connection, 1)? {
        connection
            .execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![1_i64, Utc::now().to_rfc3339()],
            )
            .map_err(|error| format!("failed to record schema migration: {error}"))?;
    }
    if !migration_applied(connection, 2)? {
        connection
            .execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![2_i64, Utc::now().to_rfc3339()],
            )
            .map_err(|error| format!("failed to record schema migration: {error}"))?;
    }
    if !migration_applied(connection, 3)? {
        crate::permissions::load_agent_permissions(connection)?;
        connection
            .execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![3_i64, Utc::now().to_rfc3339()],
            )
            .map_err(|error| format!("failed to record schema migration: {error}"))?;
    }
    if !migration_applied(connection, 4)? {
        connection
            .execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![4_i64, Utc::now().to_rfc3339()],
            )
            .map_err(|error| format!("failed to record schema migration: {error}"))?;
    }
    Ok(())
}

pub fn store_provider_secret(path: &Path, slot_id: &str, ciphertext: &str) -> Result<(), String> {
    let connection = open_database(path)?;
    connection
        .execute(
            "INSERT INTO provider_secrets (slot_id, ciphertext, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(slot_id) DO UPDATE SET
                ciphertext = excluded.ciphertext,
                updated_at = excluded.updated_at",
            params![slot_id, ciphertext, Utc::now().to_rfc3339()],
        )
        .map_err(|error| format!("failed to store provider secret: {error}"))?;
    Ok(())
}

pub fn read_provider_secret(path: &Path, slot_id: &str) -> Result<Option<String>, String> {
    let connection = open_database(path)?;
    let mut statement = connection
        .prepare("SELECT ciphertext FROM provider_secrets WHERE slot_id = ?1")
        .map_err(|error| format!("failed to prepare provider secret query: {error}"))?;
    match statement.query_row([slot_id], |row| row.get::<_, String>(0)) {
        Ok(ciphertext) => Ok(Some(ciphertext)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(format!("failed to read provider secret: {error}")),
    }
}

pub fn delete_provider_secret(path: &Path, slot_id: &str) -> Result<(), String> {
    let connection = open_database(path)?;
    connection
        .execute(
            "DELETE FROM provider_secrets WHERE slot_id = ?1",
            params![slot_id],
        )
        .map_err(|error| format!("failed to delete provider secret: {error}"))?;
    Ok(())
}

fn read_bool_setting(
    connection: &Connection,
    key: &str,
    default_value: bool,
) -> Result<bool, String> {
    let mut statement = connection
        .prepare("SELECT value FROM app_settings WHERE key = ?1")
        .map_err(|error| format!("failed to prepare settings query: {error}"))?;
    match statement.query_row([key], |row| row.get::<_, String>(0)) {
        Ok(value) => Ok(matches!(value.as_str(), "true" | "1" | "yes")),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(default_value),
        Err(error) => Err(format!("failed to decode settings value: {error}")),
    }
}

fn read_string_setting(
    connection: &Connection,
    key: &str,
    default_value: &str,
) -> Result<String, String> {
    let mut statement = connection
        .prepare("SELECT value FROM app_settings WHERE key = ?1")
        .map_err(|error| format!("failed to prepare settings query: {error}"))?;
    match statement.query_row([key], |row| row.get::<_, String>(0)) {
        Ok(value) => Ok(value),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(default_value.to_owned()),
        Err(error) => Err(format!("failed to decode settings value: {error}")),
    }
}

fn set_string_setting(connection: &Connection, key: &str, value: &str) -> Result<(), String> {
    let updated_at = Utc::now().to_rfc3339();
    connection
        .execute(
            "INSERT INTO app_settings (key, value, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at",
            params![key, value, updated_at],
        )
        .map_err(|error| format!("failed to store settings value: {error}"))?;
    Ok(())
}

fn set_bool_setting(connection: &Connection, key: &str, value: bool) -> Result<(), String> {
    let updated_at = Utc::now().to_rfc3339();
    connection
        .execute(
            "INSERT INTO app_settings (key, value, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at",
            params![key, if value { "true" } else { "false" }, updated_at],
        )
        .map_err(|error| format!("failed to store settings value: {error}"))?;
    Ok(())
}

fn load_chat_messages(connection: &Connection) -> Result<Vec<ChatMessage>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, conversation_id, role, content, model, created_at
             FROM chat_messages
             ORDER BY created_at ASC",
        )
        .map_err(|error| format!("failed to prepare chat export: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok(ChatMessage {
                id: Some(row.get(0)?),
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                model: row.get(4)?,
                created_at: Some(row.get(5)?),
            })
        })
        .map_err(|error| format!("failed to load chat export: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to decode chat export: {error}"))
}

fn load_handoff_runs(connection: &Connection) -> Result<Vec<HandoffRun>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, thread_id, source_agent_id, source_agent_name, target_provider_id,
                    target_provider_name, target_model_id, title, task, context, status,
                    output, error, approvals, audit_ref, created_at, updated_at
             FROM handoff_runs
             ORDER BY created_at ASC",
        )
        .map_err(|error| format!("failed to prepare handoff export: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            let approvals: String = row.get(13)?;
            Ok(HandoffRun {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                source_agent_id: row.get(2)?,
                source_agent_name: row.get(3)?,
                target_provider_id: row.get(4)?,
                target_provider_name: row.get(5)?,
                target_model_id: row.get(6)?,
                title: row.get(7)?,
                task: row.get(8)?,
                context: row.get(9)?,
                status: row.get(10)?,
                output: row.get(11)?,
                error: row.get(12)?,
                approvals: serde_json::from_str::<Vec<String>>(&approvals).unwrap_or_default(),
                audit_ref: row.get(14)?,
                created_at: row.get(15)?,
                updated_at: row.get(16)?,
            })
        })
        .map_err(|error| format!("failed to load handoff export: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to decode handoff export: {error}"))
}

pub fn query_audit_events(
    connection: &Connection,
    limit: u32,
    offset: u32,
    filter: Option<&str>,
) -> Result<AuditEventsPage, String> {
    let filter = filter
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let total = count_audit_events(connection, filter)?;
    let events = load_audit_events_page(connection, limit, offset, filter)?;

    Ok(AuditEventsPage {
        events,
        total,
        limit,
        offset,
    })
}

fn count_audit_events(
    connection: &Connection,
    filter: Option<&str>,
) -> Result<u32, String> {
    let count: i64 = if let Some(filter) = filter {
        let pattern = format!("%{filter}%");
        connection
            .query_row(
                "SELECT COUNT(*)
                 FROM audit_events
                 WHERE action LIKE ?1 COLLATE NOCASE
                    OR model LIKE ?1 COLLATE NOCASE",
                params![pattern],
                |row| row.get(0),
            )
            .map_err(|error| format!("failed to count audit events: {error}"))?
    } else {
        connection
            .query_row("SELECT COUNT(*) FROM audit_events", [], |row| row.get(0))
            .map_err(|error| format!("failed to count audit events: {error}"))?
    };

    Ok(count.max(0) as u32)
}

fn load_audit_events_page(
    connection: &Connection,
    limit: u32,
    offset: u32,
    filter: Option<&str>,
) -> Result<Vec<AuditEventRecord>, String> {
    let mut statement = if filter.is_some() {
        connection
            .prepare(
                "SELECT id, action, status, model, conversation_id, duration_ms, created_at
                 FROM audit_events
                 WHERE action LIKE ?1 COLLATE NOCASE
                    OR model LIKE ?1 COLLATE NOCASE
                 ORDER BY created_at DESC
                 LIMIT ?2 OFFSET ?3",
            )
            .map_err(|error| format!("failed to prepare audit query: {error}"))?
    } else {
        connection
            .prepare(
                "SELECT id, action, status, model, conversation_id, duration_ms, created_at
                 FROM audit_events
                 ORDER BY created_at DESC
                 LIMIT ?1 OFFSET ?2",
            )
            .map_err(|error| format!("failed to prepare audit query: {error}"))?
    };

    let rows = if let Some(filter) = filter {
        let pattern = format!("%{filter}%");
        statement
            .query_map(params![pattern, limit, offset], map_audit_row)
            .map_err(|error| format!("failed to load audit events: {error}"))?
    } else {
        statement
            .query_map(params![limit, offset], map_audit_row)
            .map_err(|error| format!("failed to load audit events: {error}"))?
    };

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to decode audit events: {error}"))
}

pub(crate) fn map_audit_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AuditEventRecord> {
    Ok(AuditEventRecord {
        id: row.get(0)?,
        action: row.get(1)?,
        status: row.get(2)?,
        model: row.get(3)?,
        conversation_id: row.get(4)?,
        duration_ms: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn load_audit_events(connection: &Connection) -> Result<Vec<AuditEventRecord>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, action, status, model, conversation_id, duration_ms, created_at
             FROM audit_events
             ORDER BY created_at ASC",
        )
        .map_err(|error| format!("failed to prepare audit export: {error}"))?;
    let rows = statement
        .query_map([], map_audit_row)
        .map_err(|error| format!("failed to load audit export: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to decode audit export: {error}"))
}

fn load_plugin_settings(connection: &Connection) -> Result<Vec<PluginSettingRecord>, String> {
    let mut statement = connection
        .prepare(
            "SELECT plugin_id, enabled, updated_at FROM plugin_settings ORDER BY plugin_id ASC",
        )
        .map_err(|error| format!("failed to prepare plugin settings export: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok(PluginSettingRecord {
                plugin_id: row.get(0)?,
                enabled: row.get::<_, i64>(1)? != 0,
                updated_at: row.get(2)?,
            })
        })
        .map_err(|error| format!("failed to load plugin settings export: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to decode plugin settings export: {error}"))
}

fn load_skill_executions(connection: &Connection) -> Result<Vec<SkillExecutionRecord>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, skill_id, skill_name, status, audit_ref, created_at
             FROM skill_execution_runs
             ORDER BY created_at ASC",
        )
        .map_err(|error| format!("failed to prepare skill execution export: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok(SkillExecutionRecord {
                id: row.get(0)?,
                skill_id: row.get(1)?,
                skill_name: row.get(2)?,
                status: row.get(3)?,
                audit_ref: row.get(4)?,
                created_at: row.get(5)?,
                output: String::new(),
            })
        })
        .map_err(|error| format!("failed to load skill execution export: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to decode skill execution export: {error}"))
}

fn ensure_export_dir(path: &Path) -> Result<PathBuf, String> {
    let export_dir = path
        .parent()
        .map(|parent| parent.join("exports"))
        .unwrap_or_else(|| PathBuf::from("exports"));
    fs::create_dir_all(&export_dir)
        .map_err(|error| format!("failed to create export directory: {error}"))?;
    Ok(export_dir)
}

fn database_family(path: &Path) -> Vec<PathBuf> {
    vec![
        path.to_path_buf(),
        PathBuf::from(format!("{}-wal", path.display())),
        PathBuf::from(format!("{}-shm", path.display())),
    ]
}

fn log_path(path: &Path) -> PathBuf {
    path.parent()
        .map(|parent| parent.join("agentdeck.log"))
        .unwrap_or_else(|| PathBuf::from("agentdeck.log"))
}

fn redact_snapshot(mut snapshot: LocalDataExport) -> LocalDataExport {
    snapshot.chat_messages = snapshot
        .chat_messages
        .into_iter()
        .map(|mut message| {
            message.content = redact_text(&message.content);
            message
        })
        .collect();
    snapshot.handoff_runs = snapshot
        .handoff_runs
        .into_iter()
        .map(|mut run| {
            run.title = redact_text(&run.title);
            run.task = redact_text(&run.task);
            run.context = redact_text(&run.context);
            run.output = redact_text(&run.output);
            run.error = run.error.map(|value| redact_text(&value));
            run
        })
        .collect();
    snapshot
}

fn redact_text(value: &str) -> String {
    let lowered = value.to_lowercase();
    if lowered.contains("bearer ")
        || lowered.contains("api key")
        || lowered.contains("api_key")
        || lowered.contains("password")
        || lowered.contains("secret")
        || lowered.contains("private key")
        || lowered.contains("session cookie")
    {
        return "[redacted]".to_owned();
    }
    value.to_owned()
}

#[cfg(test)]
mod storage_tests {
    use super::*;

    #[test]
    fn validate_identifier_rejects_overlong_values() {
        let value = "a".repeat(MAX_IDENTIFIER_CHARS + 1);
        assert!(validate_identifier("test ID", &value).is_err());
        assert!(validate_identifier("test ID", &"a".repeat(MAX_IDENTIFIER_CHARS)).is_ok());
    }
}

fn migration_applied(connection: &Connection, version: i64) -> Result<bool, String> {
    let mut statement = connection
        .prepare("SELECT 1 FROM schema_migrations WHERE version = ?1 LIMIT 1")
        .map_err(|error| format!("failed to prepare migration query: {error}"))?;
    let result = statement
        .query_row([version], |row| row.get::<_, i64>(0))
        .map(|_| true);
    match result {
        Ok(value) => Ok(value),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
        Err(error) => Err(format!("failed to query schema migration state: {error}")),
    }
}
