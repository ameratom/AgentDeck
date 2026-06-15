use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::models::{
    AppSettings, AuditEventRecord, AuditEventsPage, ChatMessage, ChatPreferences, HandoffRun,
    LocalDeleteResult, LocalExportResult, ProjectConnectorSettings, ProjectWorkspace, RouterRule,
    SkillExecutionRecord, WebhookEndpoint,
};

const DEFAULT_REDACT_SENSITIVE_EXPORTS: bool = true;
const DEFAULT_CRASH_SAFE_LOGGING: bool = true;
const DEFAULT_GROK_SUBSCRIPTION_ACTIVE: bool = true;
const DEFAULT_ONBOARDING_COMPLETE: bool = false;
const DEFAULT_ROUTER_AUTO_APPLY: bool = true;
const DEFAULT_MENU_BAR_SERVICE_MODE: bool = true;
const DEFAULT_START_HIDDEN: bool = true;
const DEFAULT_CLOSE_HIDES_TO_MENU_BAR: bool = true;
const DEFAULT_LAUNCH_AT_LOGIN: bool = false;
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

pub fn lookup_handoff_run_id_by_audit_ref(
    connection: &Connection,
    audit_ref: &str,
) -> Result<Option<String>, String> {
    connection
        .query_row(
            "SELECT id FROM handoff_runs
             WHERE audit_ref = ?1
             ORDER BY created_at DESC
             LIMIT 1",
            [audit_ref],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("failed to resolve handoff run by audit ref: {error}"))
}

pub fn lookup_handoff_run_id_by_thread_id(
    connection: &Connection,
    thread_id: &str,
) -> Result<Option<String>, String> {
    connection
        .query_row(
            "SELECT id FROM handoff_runs
             WHERE thread_id = ?1
             ORDER BY created_at DESC
             LIMIT 1",
            [thread_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("failed to resolve handoff run by thread id: {error}"))
}

pub fn resolve_handoff_run_id(
    connection: &Connection,
    run_id: Option<&str>,
    audit_id: Option<&str>,
    conversation_id: Option<&str>,
) -> Result<String, String> {
    if let Some(run_id) = run_id.filter(|value| !value.trim().is_empty()) {
        validate_identifier("run ID", run_id)?;
        return Ok(run_id.to_owned());
    }
    if let Some(audit_id) = audit_id.filter(|value| !value.trim().is_empty()) {
        validate_identifier("audit ID", audit_id)?;
        if let Some(run_id) = lookup_handoff_run_id_by_audit_ref(connection, audit_id)? {
            return Ok(run_id);
        }
        return Err(format!("no handoff run found for audit ID {audit_id}"));
    }
    if let Some(conversation_id) = conversation_id.filter(|value| !value.trim().is_empty()) {
        if let Some(run_id) = lookup_handoff_run_id_by_thread_id(connection, conversation_id)? {
            return Ok(run_id);
        }
        return Err(format!(
            "no handoff run found for conversation ID {conversation_id}"
        ));
    }
    Err("one of runId, auditId, or conversationId is required".to_owned())
}

pub fn load_handoff_run(connection: &Connection, run_id: &str) -> Result<HandoffRun, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, project_id, thread_id, source_agent_id, source_agent_name, target_provider_id,
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
        .get(14)
        .map_err(|error| format!("failed to decode approvals: {error}"))?;
    Ok(HandoffRun {
        id: row
            .get(0)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        project_id: row
            .get(1)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        thread_id: row
            .get(2)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        source_agent_id: row
            .get(3)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        source_agent_name: row
            .get(4)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        target_provider_id: row
            .get(5)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        target_provider_name: row
            .get(6)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        target_model_id: row
            .get(7)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        title: row
            .get(8)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        task: row
            .get(9)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        context: row
            .get(10)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        status: row
            .get(11)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        output: row
            .get(12)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        error: row
            .get(13)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        approvals: serde_json::from_str::<Vec<String>>(&approvals).unwrap_or_default(),
        audit_ref: row
            .get(15)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        created_at: row
            .get(16)
            .map_err(|error| format!("failed to decode handoff run: {error}"))?,
        updated_at: row
            .get(17)
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
        router_auto_apply: read_bool_setting(
            &connection,
            "router_auto_apply",
            DEFAULT_ROUTER_AUTO_APPLY,
        )?,
        menu_bar_service_mode: read_bool_setting(
            &connection,
            "menu_bar_service_mode",
            DEFAULT_MENU_BAR_SERVICE_MODE,
        )?,
        start_hidden: read_bool_setting(&connection, "start_hidden", DEFAULT_START_HIDDEN)?,
        close_hides_to_menu_bar: read_bool_setting(
            &connection,
            "close_hides_to_menu_bar",
            DEFAULT_CLOSE_HIDES_TO_MENU_BAR,
        )?,
        launch_at_login: read_bool_setting(
            &connection,
            "launch_at_login",
            DEFAULT_LAUNCH_AT_LOGIN,
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
    set_string_setting(
        &connection,
        "chat_last_model_id",
        &preferences.last_model_id,
    )?;
    Ok(())
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
    set_bool_setting(&connection, "router_auto_apply", settings.router_auto_apply)?;
    set_bool_setting(
        &connection,
        "menu_bar_service_mode",
        settings.menu_bar_service_mode,
    )?;
    set_bool_setting(&connection, "start_hidden", settings.start_hidden)?;
    set_bool_setting(
        &connection,
        "close_hides_to_menu_bar",
        settings.close_hides_to_menu_bar,
    )?;
    set_bool_setting(&connection, "launch_at_login", settings.launch_at_login)?;
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
        router_rules: load_router_rules(&connection)?,
        projects: load_project_workspaces(&connection)?,
        project_connector_settings: load_all_project_connector_settings(&connection)?,
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
    router_rules: Vec<RouterRule>,
    projects: Vec<ProjectWorkspace>,
    project_connector_settings: Vec<ProjectConnectorSettings>,
    app_settings: AppSettings,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RouterRulePayload {
    name: String,
    enabled: bool,
    source_agent_id: Option<String>,
    keyword: Option<String>,
    target_provider_id: String,
    target_model_id: Option<String>,
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
                project_id TEXT,
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
            CREATE TABLE IF NOT EXISTS project_workspaces (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE,
                active INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_project_workspaces_active
                ON project_workspaces(active DESC, updated_at DESC);
            CREATE TABLE IF NOT EXISTS project_connector_settings (
                project_id TEXT PRIMARY KEY,
                filesystem_enabled INTEGER NOT NULL DEFAULT 0,
                git_enabled INTEGER NOT NULL DEFAULT 0,
                claude_export_path TEXT NOT NULL,
                codex_export_path TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (project_id) REFERENCES project_workspaces(id) ON DELETE CASCADE
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
    if !migration_applied(connection, 5)? {
        purge_legacy_provider_credential_flags(connection)?;
        connection
            .execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![5_i64, Utc::now().to_rfc3339()],
            )
            .map_err(|error| format!("failed to record schema migration: {error}"))?;
    }
    if !migration_applied(connection, 6)? {
        seed_default_router_rules(connection)?;
        connection
            .execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![6_i64, Utc::now().to_rfc3339()],
            )
            .map_err(|error| format!("failed to record schema migration: {error}"))?;
    }
    if !migration_applied(connection, 7)? {
        connection
            .execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![7_i64, Utc::now().to_rfc3339()],
            )
            .map_err(|error| format!("failed to record schema migration: {error}"))?;
    }
    if !migration_applied(connection, 8)? {
        let has_project_id = connection
            .prepare("PRAGMA table_info(handoff_runs)")
            .and_then(|mut statement| {
                let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
                Ok(columns
                    .filter_map(Result::ok)
                    .any(|column| column == "project_id"))
            })
            .map_err(|error| format!("failed to inspect handoff schema: {error}"))?;
        if !has_project_id {
            connection
                .execute("ALTER TABLE handoff_runs ADD COLUMN project_id TEXT", [])
                .map_err(|error| format!("failed to add handoff project scope: {error}"))?;
        }
        connection
            .execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![8_i64, Utc::now().to_rfc3339()],
            )
            .map_err(|error| format!("failed to record schema migration: {error}"))?;
    }
    if !migration_applied(connection, 9)? {
        connection
            .execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![9_i64, Utc::now().to_rfc3339()],
            )
            .map_err(|error| format!("failed to record schema migration: {error}"))?;
    }
    if !migration_applied(connection, 10)? {
        let has_claude_code_serve = connection
            .prepare("PRAGMA table_info(project_connector_settings)")
            .and_then(|mut statement| {
                let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
                Ok(columns
                    .filter_map(Result::ok)
                    .any(|column| column == "claude_code_serve_enabled"))
            })
            .map_err(|error| format!("failed to inspect connector settings schema: {error}"))?;
        if !has_claude_code_serve {
            connection
                .execute(
                    "ALTER TABLE project_connector_settings
                     ADD COLUMN claude_code_serve_enabled INTEGER NOT NULL DEFAULT 0",
                    [],
                )
                .map_err(|error| {
                    format!("failed to add claude code serve connector flag: {error}")
                })?;
        }
        connection
            .execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![10_i64, Utc::now().to_rfc3339()],
            )
            .map_err(|error| format!("failed to record schema migration: {error}"))?;
    }
    if !migration_applied(connection, 11)? {
        for column in ["grok_mcp_enabled", "xai_research_mcp_enabled"] {
            let has_column = connection
                .prepare("PRAGMA table_info(project_connector_settings)")
                .and_then(|mut statement| {
                    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
                    Ok(columns.filter_map(Result::ok).any(|name| name == column))
                })
                .map_err(|error| format!("failed to inspect connector settings schema: {error}"))?;
            if !has_column {
                connection
                    .execute(
                        &format!(
                            "ALTER TABLE project_connector_settings
                             ADD COLUMN {column} INTEGER NOT NULL DEFAULT 1"
                        ),
                        [],
                    )
                    .map_err(|error| format!("failed to add connector column {column}: {error}"))?;
            }
        }
        connection
            .execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![11_i64, Utc::now().to_rfc3339()],
            )
            .map_err(|error| format!("failed to record schema migration: {error}"))?;
    }
    if !migration_applied(connection, 12)? {
        connection
            .execute(
                "CREATE TABLE IF NOT EXISTS webhook_endpoints (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    url TEXT NOT NULL,
                    enabled INTEGER NOT NULL,
                    event_types_json TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                )",
                [],
            )
            .map_err(|error| format!("failed to create webhook endpoints table: {error}"))?;
        connection
            .execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![12_i64, Utc::now().to_rfc3339()],
            )
            .map_err(|error| format!("failed to record schema migration: {error}"))?;
    }
    Ok(())
}

pub fn webhook_secret_slot(endpoint_id: &str) -> String {
    format!("webhook-secret:{endpoint_id}")
}

pub fn plugin_enabled(connection: &Connection, plugin_id: &str) -> Result<bool, String> {
    let enabled: Option<i64> = connection
        .query_row(
            "SELECT enabled FROM plugin_settings WHERE plugin_id = ?1",
            params![plugin_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("failed to read plugin setting: {error}"))?;
    Ok(enabled.map(|value| value != 0).unwrap_or(true))
}

pub fn load_webhook_endpoints(
    connection: &Connection,
    database_path: &Path,
) -> Result<Vec<WebhookEndpoint>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, name, url, enabled, event_types_json, updated_at
             FROM webhook_endpoints
             ORDER BY name ASC, id ASC",
        )
        .map_err(|error| format!("failed to prepare webhook endpoint query: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)? != 0,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .map_err(|error| format!("failed to load webhook endpoints: {error}"))?;

    rows.map(|row| {
        let (id, name, url, enabled, event_types_json, updated_at) =
            row.map_err(|error| format!("failed to decode webhook endpoint: {error}"))?;
        let event_types: Vec<String> = serde_json::from_str(&event_types_json)
            .map_err(|error| format!("failed to decode webhook event types for {id}: {error}"))?;
        let has_secret = read_provider_secret(database_path, &webhook_secret_slot(&id))?.is_some();
        Ok(WebhookEndpoint {
            id,
            name,
            url,
            enabled,
            event_types,
            has_secret,
            updated_at,
        })
    })
    .collect()
}

pub fn replace_webhook_endpoints(
    connection: &Connection,
    database_path: &Path,
    endpoints: &[WebhookEndpoint],
) -> Result<Vec<WebhookEndpoint>, String> {
    if endpoints.len() > 24 {
        return Err("webhook endpoint registry supports at most 24 entries".to_owned());
    }

    let existing_ids: Vec<String> = connection
        .prepare("SELECT id FROM webhook_endpoints")
        .map_err(|error| format!("failed to prepare webhook endpoint ids query: {error}"))?
        .query_map([], |row| row.get(0))
        .map_err(|error| format!("failed to load webhook endpoint ids: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to decode webhook endpoint ids: {error}"))?;

    let next_ids = endpoints
        .iter()
        .map(|endpoint| endpoint.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for endpoint in endpoints {
        validate_identifier("webhook endpoint ID", &endpoint.id)?;
        validate_identifier("webhook endpoint name", &endpoint.name)?;
        crate::webhooks::validate_url(&endpoint.url)?;
        if endpoint.event_types.is_empty() {
            return Err(format!(
                "webhook endpoint {} must subscribe to at least one event",
                endpoint.id
            ));
        }
        for event_type in &endpoint.event_types {
            crate::webhooks::validate_event_type(event_type)?;
        }
    }

    connection
        .execute("DELETE FROM webhook_endpoints", [])
        .map_err(|error| format!("failed to clear webhook endpoints: {error}"))?;

    let updated_at = Utc::now().to_rfc3339();
    for endpoint in endpoints {
        let event_types_json = serde_json::to_string(&endpoint.event_types).map_err(|error| {
            format!(
                "failed to encode webhook event types for {}: {error}",
                endpoint.id
            )
        })?;
        connection
            .execute(
                "INSERT INTO webhook_endpoints
                    (id, name, url, enabled, event_types_json, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    endpoint.id,
                    endpoint.name,
                    endpoint.url,
                    if endpoint.enabled { 1_i64 } else { 0_i64 },
                    event_types_json,
                    updated_at,
                ],
            )
            .map_err(|error| format!("failed to store webhook endpoint: {error}"))?;
    }

    for existing_id in existing_ids {
        if !next_ids.contains(existing_id.as_str()) {
            let _ = delete_provider_secret(database_path, &webhook_secret_slot(&existing_id));
        }
    }

    load_webhook_endpoints(connection, database_path)
}

pub fn load_project_workspaces(connection: &Connection) -> Result<Vec<ProjectWorkspace>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, name, path, active, created_at, updated_at
             FROM project_workspaces
             ORDER BY active DESC, updated_at DESC, name ASC",
        )
        .map_err(|error| format!("failed to prepare project query: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            let path: String = row.get(2)?;
            Ok(ProjectWorkspace {
                id: row.get(0)?,
                name: row.get(1)?,
                exists: Path::new(&path).is_dir(),
                path,
                active: row.get::<_, i64>(3)? != 0,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })
        .map_err(|error| format!("failed to query projects: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to decode projects: {error}"))
}

pub fn load_active_project(connection: &Connection) -> Result<Option<ProjectWorkspace>, String> {
    Ok(load_project_workspaces(connection)?
        .into_iter()
        .find(|project| project.active))
}

pub fn require_active_project(
    connection: &Connection,
    project_id: &str,
) -> Result<ProjectWorkspace, String> {
    let project = load_active_project(connection)?
        .ok_or_else(|| "no active project is configured".to_owned())?;
    if project.id != project_id {
        return Err("the requested project is no longer active".to_owned());
    }
    if !project.exists {
        return Err("the active project folder is unavailable".to_owned());
    }
    Ok(project)
}

pub fn load_project_connector_settings(
    connection: &Connection,
    project: &ProjectWorkspace,
) -> Result<Option<ProjectConnectorSettings>, String> {
    connection
        .query_row(
            "SELECT filesystem_enabled, git_enabled, claude_code_serve_enabled,
                    grok_mcp_enabled, xai_research_mcp_enabled,
                    claude_export_path, codex_export_path, updated_at
             FROM project_connector_settings
             WHERE project_id = ?1",
            [&project.id],
            |row| {
                let claude_export_path: String = row.get(5)?;
                let claude_code_serve_export_path = Path::new(&claude_export_path)
                    .parent()
                    .map(|directory| {
                        directory
                            .join("claude-code-serve.mcp.json")
                            .to_string_lossy()
                            .into_owned()
                    })
                    .unwrap_or_else(|| "claude-code-serve.mcp.json".to_owned());
                Ok(ProjectConnectorSettings {
                    project_id: project.id.clone(),
                    project_name: project.name.clone(),
                    project_path: project.path.clone(),
                    filesystem_enabled: row.get::<_, i64>(0)? != 0,
                    git_enabled: row.get::<_, i64>(1)? != 0,
                    claude_code_serve_enabled: row.get::<_, i64>(2)? != 0,
                    grok_mcp_enabled: row.get::<_, i64>(3)? != 0,
                    xai_research_mcp_enabled: row.get::<_, i64>(4)? != 0,
                    claude_export_path,
                    codex_export_path: row.get(6)?,
                    claude_code_serve_export_path,
                    updated_at: row.get(7)?,
                })
            },
        )
        .optional()
        .map_err(|error| format!("failed to load project connector settings: {error}"))
}

fn load_all_project_connector_settings(
    connection: &Connection,
) -> Result<Vec<ProjectConnectorSettings>, String> {
    let projects = load_project_workspaces(connection)?;
    let mut settings = Vec::new();
    for project in &projects {
        if let Some(project_settings) = load_project_connector_settings(connection, project)? {
            settings.push(project_settings);
        }
    }
    Ok(settings)
}

fn seed_default_router_rules(connection: &Connection) -> Result<(), String> {
    let rule_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM router_rules", [], |row| row.get(0))
        .map_err(|error| format!("failed to count router rules: {error}"))?;
    if rule_count > 0 {
        return Ok(());
    }

    let updated_at = Utc::now().to_rfc3339();
    let rules = [
        RouterRule {
            id: "router-rule:default-review".to_owned(),
            priority: 0,
            name: "Local review".to_owned(),
            enabled: true,
            source_agent_id: None,
            keyword: Some("review".to_owned()),
            target_provider_id: "lm-studio".to_owned(),
            target_model_id: None,
            updated_at: updated_at.clone(),
        },
        RouterRule {
            id: "router-rule:default-research".to_owned(),
            priority: 1,
            name: "Current research".to_owned(),
            enabled: true,
            source_agent_id: None,
            keyword: Some("research".to_owned()),
            target_provider_id: "xai".to_owned(),
            target_model_id: None,
            updated_at: updated_at.clone(),
        },
        RouterRule {
            id: "router-rule:default-code".to_owned(),
            priority: 2,
            name: "Code implementation".to_owned(),
            enabled: true,
            source_agent_id: None,
            keyword: Some("code".to_owned()),
            target_provider_id: "xai".to_owned(),
            target_model_id: None,
            updated_at: updated_at.clone(),
        },
        RouterRule {
            id: "router-rule:default-implement".to_owned(),
            priority: 3,
            name: "General implementation".to_owned(),
            enabled: true,
            source_agent_id: None,
            keyword: Some("implement".to_owned()),
            target_provider_id: "xai".to_owned(),
            target_model_id: None,
            updated_at,
        },
    ];
    replace_router_rules(connection, &rules)?;
    Ok(())
}

fn purge_legacy_provider_credential_flags(connection: &Connection) -> Result<(), String> {
    connection
        .execute(
            "DELETE FROM app_settings WHERE key LIKE 'provider_credential_stored:%'",
            [],
        )
        .map_err(|error| format!("failed to purge legacy provider credential flags: {error}"))?;
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

pub fn load_router_rules(connection: &Connection) -> Result<Vec<RouterRule>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, priority, rule_json, updated_at
             FROM router_rules
             ORDER BY priority ASC, id ASC",
        )
        .map_err(|error| format!("failed to prepare router rules query: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|error| format!("failed to query router rules: {error}"))?;

    let mut rules = Vec::new();
    for row in rows {
        let (id, priority, rule_json, updated_at) =
            row.map_err(|error| format!("failed to read router rule row: {error}"))?;
        let payload: RouterRulePayload = serde_json::from_str(&rule_json)
            .map_err(|error| format!("failed to parse router rule {id}: {error}"))?;
        rules.push(RouterRule {
            id,
            priority,
            name: payload.name,
            enabled: payload.enabled,
            source_agent_id: payload.source_agent_id,
            keyword: payload.keyword,
            target_provider_id: payload.target_provider_id,
            target_model_id: payload.target_model_id,
            updated_at,
        });
    }
    Ok(rules)
}

pub fn replace_router_rules(
    connection: &Connection,
    rules: &[RouterRule],
) -> Result<Vec<RouterRule>, String> {
    if rules.len() > 50 {
        return Err("router rule limit is 50".to_owned());
    }

    for rule in rules {
        validate_identifier("router rule id", &rule.id)?;
        validate_identifier("router rule name", &rule.name)?;
        validate_identifier("router target provider id", &rule.target_provider_id)?;
        if let Some(model_id) = rule.target_model_id.as_deref() {
            if !model_id.trim().is_empty() {
                validate_identifier("router target model id", model_id)?;
            }
        }
        if let Some(source) = rule.source_agent_id.as_deref() {
            if !source.trim().is_empty() {
                validate_identifier("router source agent id", source)?;
            }
        }
    }

    let updated_at = Utc::now().to_rfc3339();
    connection
        .execute("DELETE FROM router_rules", [])
        .map_err(|error| format!("failed to clear router rules: {error}"))?;

    for (index, rule) in rules.iter().enumerate() {
        let payload = RouterRulePayload {
            name: rule.name.clone(),
            enabled: rule.enabled,
            source_agent_id: normalize_optional_text(rule.source_agent_id.as_deref()),
            keyword: normalize_optional_text(rule.keyword.as_deref()),
            target_provider_id: rule.target_provider_id.clone(),
            target_model_id: normalize_optional_text(rule.target_model_id.as_deref()),
        };
        let rule_json = serde_json::to_string(&payload)
            .map_err(|error| format!("failed to encode router rule: {error}"))?;
        connection
            .execute(
                "INSERT INTO router_rules (id, priority, rule_json, updated_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![rule.id, index as i32, rule_json, updated_at],
            )
            .map_err(|error| format!("failed to store router rule: {error}"))?;
    }

    load_router_rules(connection)
}

fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
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
            "SELECT id, project_id, thread_id, source_agent_id, source_agent_name, target_provider_id,
                    target_provider_name, target_model_id, title, task, context, status,
                    output, error, approvals, audit_ref, created_at, updated_at
             FROM handoff_runs
             ORDER BY created_at ASC",
        )
        .map_err(|error| format!("failed to prepare handoff export: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            let approvals: String = row.get(14)?;
            Ok(HandoffRun {
                id: row.get(0)?,
                project_id: row.get(1)?,
                thread_id: row.get(2)?,
                source_agent_id: row.get(3)?,
                source_agent_name: row.get(4)?,
                target_provider_id: row.get(5)?,
                target_provider_name: row.get(6)?,
                target_model_id: row.get(7)?,
                title: row.get(8)?,
                task: row.get(9)?,
                context: row.get(10)?,
                status: row.get(11)?,
                output: row.get(12)?,
                error: row.get(13)?,
                approvals: serde_json::from_str::<Vec<String>>(&approvals).unwrap_or_default(),
                audit_ref: row.get(15)?,
                created_at: row.get(16)?,
                updated_at: row.get(17)?,
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
    let filter = filter.map(str::trim).filter(|value| !value.is_empty());
    let total = count_audit_events(connection, filter)?;
    let mut events = load_audit_events_page(connection, limit, offset, filter)?;

    enrich_audit_events_with_run_ids(connection, &mut events)?;

    Ok(AuditEventsPage {
        events,
        total,
        limit,
        offset,
    })
}

pub fn enrich_audit_events_with_run_ids(
    connection: &Connection,
    events: &mut [AuditEventRecord],
) -> Result<(), String> {
    for record in events.iter_mut() {
        if !record.action.starts_with("handoff.") {
            continue;
        }
        record.run_id = lookup_handoff_run_id_by_audit_ref(connection, &record.id)?.or_else(|| {
            lookup_handoff_run_id_by_thread_id(connection, &record.conversation_id)
                .ok()
                .flatten()
        });
    }
    Ok(())
}

fn count_audit_events(connection: &Connection, filter: Option<&str>) -> Result<u32, String> {
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
        run_id: None,
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
    snapshot.database_path = "[redacted path]".to_owned();
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
    snapshot.projects = snapshot
        .projects
        .into_iter()
        .map(|mut project| {
            project.path = "[redacted path]".to_owned();
            project
        })
        .collect();
    snapshot.project_connector_settings = snapshot
        .project_connector_settings
        .into_iter()
        .map(|mut settings| {
            settings.project_path = "[redacted path]".to_owned();
            settings.claude_export_path = "[redacted path]".to_owned();
            settings.codex_export_path = "[redacted path]".to_owned();
            settings.claude_code_serve_export_path = "[redacted path]".to_owned();
            settings
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

    #[test]
    fn enrich_audit_events_links_handoff_dispatch_rows_to_runs() {
        let path = std::env::temp_dir().join(format!(
            "agentdeck-audit-run-link-{}.sqlite3",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let connection = open_database(&path).expect("open database");
        connection
            .execute(
                "INSERT INTO audit_events
                 (id, action, status, model, conversation_id, duration_ms, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    "audit:handoff-1",
                    "handoff.dispatch",
                    "completed",
                    "grok-4.3",
                    "thread:handoff-1",
                    1200_i64,
                    "2026-06-14T12:00:00Z"
                ],
            )
            .expect("insert audit event");
        connection
            .execute(
                "INSERT INTO handoff_runs
                 (id, project_id, thread_id, source_agent_id, source_agent_name,
                  target_provider_id, target_provider_name, target_model_id,
                  title, task, context, status, output, error, approvals,
                  audit_ref, created_at, updated_at)
                 VALUES (?1, NULL, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, NULL, ?13, ?14, ?15, ?16)",
                rusqlite::params![
                    "run:linked",
                    "thread:handoff-1",
                    "agent:codex",
                    "Codex",
                    "provider:xai:grok",
                    "xAI Grok",
                    "grok-4.3",
                    "Review handoff",
                    "Summarize next steps",
                    "",
                    "completed",
                    "done",
                    "[\"user-approved\"]",
                    "audit:handoff-1",
                    "2026-06-14T12:00:00Z",
                    "2026-06-14T12:00:01Z"
                ],
            )
            .expect("insert handoff run");

        let page = query_audit_events(&connection, 10, 0, None).expect("audit page");
        assert_eq!(page.events.len(), 1);
        assert_eq!(page.events[0].run_id.as_deref(), Some("run:linked"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn purge_legacy_provider_credential_flags_removes_stale_rows() {
        let dir = std::env::temp_dir().join(format!(
            "agentdeck-purge-credential-flags-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("agentdeck.sqlite3");
        let connection = open_database(&path).unwrap();
        connection
            .execute(
                "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
                params![
                    "provider_credential_stored:anthropic",
                    "true",
                    Utc::now().to_rfc3339()
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
                params![
                    "provider_credential_stored:codex",
                    "true",
                    Utc::now().to_rfc3339()
                ],
            )
            .unwrap();

        purge_legacy_provider_credential_flags(&connection).unwrap();

        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM app_settings WHERE key LIKE 'provider_credential_stored:%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn replace_router_rules_persists_priority_order() {
        let dir = std::env::temp_dir().join(format!(
            "agentdeck-router-rules-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("agentdeck.sqlite3");
        let connection = open_database(&path).unwrap();
        let rules = vec![
            RouterRule {
                id: "router-rule:local".to_owned(),
                priority: 0,
                name: "Local review".to_owned(),
                enabled: true,
                source_agent_id: None,
                keyword: Some("review".to_owned()),
                target_provider_id: "lm-studio".to_owned(),
                target_model_id: None,
                updated_at: Utc::now().to_rfc3339(),
            },
            RouterRule {
                id: "router-rule:cloud".to_owned(),
                priority: 1,
                name: "Cloud research".to_owned(),
                enabled: true,
                source_agent_id: Some("agent:grok".to_owned()),
                keyword: Some("research".to_owned()),
                target_provider_id: "xai".to_owned(),
                target_model_id: None,
                updated_at: Utc::now().to_rfc3339(),
            },
        ];

        let saved = replace_router_rules(&connection, &rules).unwrap();
        assert_eq!(saved.len(), 2);
        assert_eq!(saved[0].id, "router-rule:local");
        assert_eq!(saved[1].target_provider_id, "xai");

        let loaded = load_router_rules(&connection).unwrap();
        assert_eq!(loaded[0].keyword.as_deref(), Some("review"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn seeds_default_router_rules_once() {
        let dir = std::env::temp_dir().join(format!(
            "agentdeck-default-router-rules-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("agentdeck.sqlite3");

        let connection = open_database(&path).unwrap();
        let rules = load_router_rules(&connection).unwrap();
        assert_eq!(rules.len(), 4);
        assert_eq!(rules[0].id, "router-rule:default-review");
        assert_eq!(rules[1].target_provider_id, "xai");
        assert_eq!(rules[2].target_provider_id, "xai");
        assert_eq!(rules[3].target_provider_id, "xai");

        replace_router_rules(&connection, &[]).unwrap();
        drop(connection);

        let reopened = open_database(&path).unwrap();
        assert!(load_router_rules(&reopened).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn loads_project_workspaces_with_active_first() {
        let connection = Connection::open_in_memory().unwrap();
        migrate_database(&connection).unwrap();
        connection
            .execute(
                "INSERT INTO project_workspaces
                    (id, name, path, active, created_at, updated_at)
                 VALUES
                    ('project:one', 'One', '/tmp/one', 0, '2026-01-01', '2026-01-01'),
                    ('project:two', 'Two', '/tmp/two', 1, '2026-01-01', '2026-01-02')",
                [],
            )
            .unwrap();
        let projects = load_project_workspaces(&connection).unwrap();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].id, "project:two");
        assert!(projects[0].active);
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
