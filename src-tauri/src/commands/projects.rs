use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::models::{
    ProjectDocumentConnectors, ProjectDocumentCredentials, ProjectDocumentMetadata,
    AutonomyRestrictions, ProjectDocumentV2, ProjectDocumentV3, ProjectFileChange,
    ProjectFilePreview, ProjectWorkspace, ProjectWorkspaceList, RegisterProjectRequest,
    SaveProjectConnectorSettingsRequest, SaveProjectRestrictionsRequest,
};
use crate::storage;

use super::mcp::{default_connector_settings, write_connector_exports};

const PROJECT_KIND: &str = "agentdeck.project";
const PROJECT_FORMAT_VERSION: u32 = 3;
const MAX_PROJECT_FILE_BYTES: u64 = 1_048_576;
const MAX_DESCRIPTION_CHARS: usize = 2_000;

#[tauri::command]
pub async fn list_projects(app: AppHandle) -> Result<ProjectWorkspaceList, String> {
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let connection = storage::open_database(&database_path)?;
        project_list(&connection)
    })
    .await
    .map_err(|error| format!("project list task failed: {error}"))?
}

#[tauri::command]
pub async fn register_project(
    app: AppHandle,
    request: RegisterProjectRequest,
) -> Result<ProjectWorkspaceList, String> {
    let database_path = storage::database_path(&app)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let canonical_path = canonical_project_path(&request.path)?;
        let project_id = project_id(&canonical_path);
        let name = project_name(&canonical_path, request.name.as_deref())?;
        let connection = storage::open_database(&database_path)?;
        let created_at = Utc::now().to_rfc3339();
        let has_projects: bool = connection
            .query_row("SELECT COUNT(*) > 0 FROM project_workspaces", [], |row| {
                row.get(0)
            })
            .map_err(|error| format!("failed to count projects: {error}"))?;
        connection
            .execute(
                "INSERT INTO project_workspaces
                    (id, name, path, active, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)
                 ON CONFLICT(path) DO UPDATE SET
                    name = excluded.name,
                    updated_at = excluded.updated_at",
                params![
                    project_id,
                    name,
                    canonical_path.to_string_lossy(),
                    if has_projects { 0_i64 } else { 1_i64 },
                    created_at
                ],
            )
            .map_err(|error| format!("failed to register project: {error}"))?;
        audit_project_action(&connection, "project.register", &project_id)?;
        storage::append_log_event(
            &database_path,
            "project.register",
            serde_json::json!({ "projectId": project_id }),
        );
        project_list(&connection)
    })
    .await
    .map_err(|error| format!("project registration task failed: {error}"))??;
    app.emit("project-changed", ())
        .map_err(|error| format!("failed to emit project update: {error}"))?;
    Ok(result)
}

#[tauri::command]
pub async fn preview_project_file(
    app: AppHandle,
    project_id: String,
) -> Result<ProjectFilePreview, String> {
    storage::validate_identifier("project ID", &project_id)?;
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let connection = storage::open_database(&database_path)?;
        let project = find_project(&connection, &project_id)?;
        preview_project_document(&connection, &database_path, &project)
    })
    .await
    .map_err(|error| format!("project preview task failed: {error}"))?
}

#[tauri::command]
pub async fn save_project_file(
    app: AppHandle,
    project_id: String,
) -> Result<ProjectWorkspaceList, String> {
    storage::validate_identifier("project ID", &project_id)?;
    let database_path = storage::database_path(&app)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let connection = storage::open_database(&database_path)?;
        let project = find_project(&connection, &project_id)?;
        if !project.exists {
            return Err("the project folder is unavailable".to_owned());
        }
        let settings = storage::load_project_connector_settings(&connection, &project)?
            .unwrap_or_else(|| default_connector_settings(&database_path, &project));
        let document = document_from_project(&project, &settings);
        validate_project_document(&document)?;
        let (file_path, digest) = write_project_document(&project, &document)?;
        let updated_at = document.metadata.updated_at.clone();
        connection
            .execute(
                "UPDATE project_workspaces
                 SET description = ?2, project_format_version = ?3,
                     project_file_digest = ?4, updated_at = ?5
                 WHERE id = ?1",
                params![
                    project.id,
                    document.metadata.description,
                    PROJECT_FORMAT_VERSION,
                    digest,
                    updated_at,
                ],
            )
            .map_err(|error| format!("failed to record project file state: {error}"))?;
        audit_project_action(&connection, "project.file.save", &project.id)?;
        storage::append_log_event(
            &database_path,
            "project.file.save",
            serde_json::json!({ "projectId": project.id, "path": file_path }),
        );
        project_list(&connection)
    })
    .await
    .map_err(|error| format!("project save task failed: {error}"))??;
    app.emit("project-changed", ())
        .map_err(|error| format!("failed to emit project update: {error}"))?;
    Ok(result)
}

#[tauri::command]
pub async fn save_project_restrictions(
    app: AppHandle,
    request: SaveProjectRestrictionsRequest,
) -> Result<ProjectWorkspaceList, String> {
    storage::validate_identifier("project ID", &request.project_id)?;
    validate_restrictions(&request.restrictions)?;
    let database_path = storage::database_path(&app)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let connection = storage::open_database(&database_path)?;
        find_project(&connection, &request.project_id)?;
        let encoded = serde_json::to_string(&request.restrictions)
            .map_err(|error| format!("failed to encode project restrictions: {error}"))?;
        connection
            .execute(
                "UPDATE project_workspaces
                 SET autonomy_restrictions = ?2, updated_at = ?3
                 WHERE id = ?1",
                params![request.project_id, encoded, Utc::now().to_rfc3339()],
            )
            .map_err(|error| format!("failed to save project restrictions: {error}"))?;
        audit_project_action(
            &connection,
            "project.autonomy-restrictions.save",
            &request.project_id,
        )?;
        project_list(&connection)
    })
    .await
    .map_err(|error| format!("project restriction save task failed: {error}"))??;
    app.emit("project-changed", ())
        .map_err(|error| format!("failed to emit project update: {error}"))?;
    Ok(result)
}

#[tauri::command]
pub async fn apply_project_file(
    app: AppHandle,
    project_id: String,
    expected_digest: String,
) -> Result<ProjectWorkspaceList, String> {
    storage::validate_identifier("project ID", &project_id)?;
    storage::validate_identifier("project file digest", &expected_digest)?;
    let database_path = storage::database_path(&app)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut connection = storage::open_database(&database_path)?;
        let project = find_project(&connection, &project_id)?;
        let (document, digest) = read_project_document(&project)?;
        ensure_expected_digest(&digest, &expected_digest)?;
        validate_project_document(&document)?;
        if document.connectors.git && !Path::new(&project.path).join(".git").is_dir() {
            return Err("Git MCP requires the project folder to be a Git repository".to_owned());
        }

        let connector_request = connector_request_from_document(&document);
        let settings = write_connector_exports(&database_path, &project, &connector_request)?;
        let transaction = connection
            .transaction()
            .map_err(|error| format!("failed to begin project file apply: {error}"))?;
        transaction
            .execute(
                "UPDATE project_workspaces
                 SET name = ?2, description = ?3, project_format_version = ?4,
                     project_file_digest = ?5, updated_at = ?6,
                     autonomy_restrictions = ?7
                 WHERE id = ?1",
                params![
                    project.id,
                    document.metadata.name,
                    document.metadata.description,
                    PROJECT_FORMAT_VERSION,
                    digest,
                    document.metadata.updated_at,
                    serde_json::to_string(&document.autonomy_restrictions)
                        .map_err(|error| format!("failed to encode project restrictions: {error}"))?,
                ],
            )
            .map_err(|error| format!("failed to apply project metadata: {error}"))?;
        transaction
            .execute(
                "INSERT INTO project_connector_settings
                    (project_id, filesystem_enabled, git_enabled, claude_code_serve_enabled,
                     grok_mcp_enabled, xai_research_mcp_enabled,
                     claude_export_path, codex_export_path, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(project_id) DO UPDATE SET
                    filesystem_enabled = excluded.filesystem_enabled,
                    git_enabled = excluded.git_enabled,
                    claude_code_serve_enabled = excluded.claude_code_serve_enabled,
                    grok_mcp_enabled = excluded.grok_mcp_enabled,
                    xai_research_mcp_enabled = excluded.xai_research_mcp_enabled,
                    claude_export_path = excluded.claude_export_path,
                    codex_export_path = excluded.codex_export_path,
                    updated_at = excluded.updated_at",
                params![
                    project.id,
                    bool_i64(settings.filesystem_enabled),
                    bool_i64(settings.git_enabled),
                    bool_i64(settings.claude_code_serve_enabled),
                    bool_i64(settings.grok_mcp_enabled),
                    bool_i64(settings.xai_research_mcp_enabled),
                    settings.claude_export_path,
                    settings.codex_export_path,
                    settings.updated_at,
                ],
            )
            .map_err(|error| format!("failed to apply connector settings: {error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("failed to commit project file apply: {error}"))?;
        audit_project_action(&connection, "project.file.apply", &project.id)?;
        storage::append_log_event(
            &database_path,
            "project.file.apply",
            serde_json::json!({ "projectId": project.id, "digest": expected_digest }),
        );
        project_list(&connection)
    })
    .await
    .map_err(|error| format!("project apply task failed: {error}"))??;
    app.emit("project-changed", ())
        .map_err(|error| format!("failed to emit project update: {error}"))?;
    Ok(result)
}

#[tauri::command]
pub async fn set_active_project(
    app: AppHandle,
    project_id: String,
) -> Result<ProjectWorkspaceList, String> {
    storage::validate_identifier("project ID", &project_id)?;
    let database_path = storage::database_path(&app)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut connection = storage::open_database(&database_path)?;
        let transaction = connection
            .transaction()
            .map_err(|error| format!("failed to begin project activation: {error}"))?;
        let exists: bool = transaction
            .query_row(
                "SELECT COUNT(*) > 0 FROM project_workspaces WHERE id = ?1",
                [&project_id],
                |row| row.get(0),
            )
            .map_err(|error| format!("failed to find project: {error}"))?;
        if !exists {
            return Err("project was not found".to_owned());
        }
        transaction
            .execute("UPDATE project_workspaces SET active = 0", [])
            .map_err(|error| format!("failed to clear active project: {error}"))?;
        transaction
            .execute(
                "UPDATE project_workspaces
                 SET active = 1, updated_at = ?2
                 WHERE id = ?1",
                params![project_id, Utc::now().to_rfc3339()],
            )
            .map_err(|error| format!("failed to activate project: {error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("failed to commit project activation: {error}"))?;
        audit_project_action(&connection, "project.activate", &project_id)?;
        storage::append_log_event(
            &database_path,
            "project.activate",
            serde_json::json!({ "projectId": project_id }),
        );
        project_list(&connection)
    })
    .await
    .map_err(|error| format!("project activation task failed: {error}"))??;
    app.emit("project-changed", ())
        .map_err(|error| format!("failed to emit project update: {error}"))?;
    Ok(result)
}

#[tauri::command]
pub async fn remove_project(
    app: AppHandle,
    project_id: String,
) -> Result<ProjectWorkspaceList, String> {
    storage::validate_identifier("project ID", &project_id)?;
    let database_path = storage::database_path(&app)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut connection = storage::open_database(&database_path)?;
        let transaction = connection
            .transaction()
            .map_err(|error| format!("failed to begin project removal: {error}"))?;
        let was_active: Option<i64> = transaction
            .query_row(
                "SELECT active FROM project_workspaces WHERE id = ?1",
                [&project_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("failed to find project: {error}"))?;
        if was_active.is_none() {
            return Err("project was not found".to_owned());
        }
        transaction
            .execute(
                "DELETE FROM project_workspaces WHERE id = ?1",
                [&project_id],
            )
            .map_err(|error| format!("failed to remove project: {error}"))?;
        if was_active == Some(1) {
            transaction
                .execute(
                    "UPDATE project_workspaces SET active = 1
                     WHERE id = (
                        SELECT id FROM project_workspaces
                        ORDER BY updated_at DESC, name ASC
                        LIMIT 1
                     )",
                    [],
                )
                .map_err(|error| format!("failed to select replacement project: {error}"))?;
        }
        transaction
            .commit()
            .map_err(|error| format!("failed to commit project removal: {error}"))?;
        audit_project_action(&connection, "project.remove", &project_id)?;
        storage::append_log_event(
            &database_path,
            "project.remove",
            serde_json::json!({ "projectId": project_id }),
        );
        project_list(&connection)
    })
    .await
    .map_err(|error| format!("project removal task failed: {error}"))??;
    app.emit("project-changed", ())
        .map_err(|error| format!("failed to emit project update: {error}"))?;
    Ok(result)
}

fn find_project(
    connection: &rusqlite::Connection,
    project_id: &str,
) -> Result<ProjectWorkspace, String> {
    storage::load_project_workspaces(connection)?
        .into_iter()
        .find(|project| project.id == project_id)
        .ok_or_else(|| "project was not found".to_owned())
}

fn document_from_project(
    project: &ProjectWorkspace,
    settings: &crate::models::ProjectConnectorSettings,
) -> ProjectDocumentV3 {
    let connectors = ProjectDocumentConnectors {
        filesystem: settings.filesystem_enabled,
        git: settings.git_enabled,
        claude_code_serve: settings.claude_code_serve_enabled,
        grok_mcp: settings.grok_mcp_enabled,
        xai_research_mcp: settings.xai_research_mcp_enabled,
    };
    ProjectDocumentV3 {
        kind: PROJECT_KIND.to_owned(),
        format_version: PROJECT_FORMAT_VERSION,
        metadata: ProjectDocumentMetadata {
            name: project.name.clone(),
            description: project.description.clone(),
            created_at: project.created_at.clone(),
            updated_at: Utc::now().to_rfc3339(),
        },
        credentials: ProjectDocumentCredentials {
            required_slots: required_slots(&connectors),
        },
        connectors,
        autonomy_restrictions: project.autonomy_restrictions.clone(),
    }
}

fn project_document_path(
    project: &ProjectWorkspace,
    create_directory: bool,
) -> Result<PathBuf, String> {
    let root = fs::canonicalize(&project.path)
        .map_err(|error| format!("project folder is unavailable: {error}"))?;
    let directory = root.join(".agentdeck");
    if directory.exists() {
        let metadata = fs::symlink_metadata(&directory)
            .map_err(|error| format!("failed to inspect .agentdeck directory: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(".agentdeck must be a real directory inside the project".to_owned());
        }
    } else if create_directory {
        fs::create_dir(&directory)
            .map_err(|error| format!("failed to create .agentdeck directory: {error}"))?;
    }
    if directory.exists() {
        let canonical_directory = fs::canonicalize(&directory)
            .map_err(|error| format!("failed to resolve .agentdeck directory: {error}"))?;
        if !canonical_directory.starts_with(&root) {
            return Err(".agentdeck resolves outside the registered project".to_owned());
        }
    }
    let file_path = directory.join("project.json");
    if file_path.exists() {
        let metadata = fs::symlink_metadata(&file_path)
            .map_err(|error| format!("failed to inspect project file: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err("project.json must be a regular file".to_owned());
        }
    }
    Ok(file_path)
}

fn read_project_document(
    project: &ProjectWorkspace,
) -> Result<(ProjectDocumentV3, String), String> {
    let file_path = project_document_path(project, false)?;
    let metadata = fs::metadata(&file_path)
        .map_err(|error| format!("project file is unavailable: {error}"))?;
    if metadata.len() > MAX_PROJECT_FILE_BYTES {
        return Err("project file exceeds the 1 MiB size limit".to_owned());
    }
    let bytes =
        fs::read(&file_path).map_err(|error| format!("failed to read project file: {error}"))?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("project file is not valid JSON: {error}"))?;
    let detected_format = value.get("formatVersion").and_then(Value::as_u64);
    let document = match detected_format {
        Some(2) => {
            let legacy: ProjectDocumentV2 = serde_json::from_value(value)
                .map_err(|error| format!("project file does not match Project Format v2: {error}"))?;
            ProjectDocumentV3 {
                kind: legacy.kind,
                format_version: PROJECT_FORMAT_VERSION,
                metadata: legacy.metadata,
                connectors: legacy.connectors,
                credentials: legacy.credentials,
                autonomy_restrictions: AutonomyRestrictions::default(),
            }
        }
        Some(3) => serde_json::from_value(value)
            .map_err(|error| format!("project file does not match Project Format v3: {error}"))?,
        Some(version) => {
            return Err(format!(
                "unsupported project format version {version}; this AgentDeck supports versions 2 and 3"
            ))
        }
        None => return Err("project file is missing formatVersion".to_owned()),
    };
    validate_project_document(&document)?;
    Ok((document, digest_bytes(&bytes)))
}

pub(crate) fn validate_project_document(document: &ProjectDocumentV3) -> Result<(), String> {
    if document.kind != PROJECT_KIND {
        return Err(format!("project kind must be {PROJECT_KIND}"));
    }
    if document.format_version != PROJECT_FORMAT_VERSION {
        return Err("project formatVersion must be 3".to_owned());
    }
    storage::validate_identifier("project name", document.metadata.name.trim())?;
    if document.metadata.description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(format!(
            "project description must not exceed {MAX_DESCRIPTION_CHARS} characters"
        ));
    }
    for (label, timestamp) in [
        ("createdAt", &document.metadata.created_at),
        ("updatedAt", &document.metadata.updated_at),
    ] {
        chrono::DateTime::parse_from_rfc3339(timestamp)
            .map_err(|_| format!("project metadata {label} must be an RFC 3339 timestamp"))?;
    }
    let expected_slots = required_slots(&document.connectors);
    if document.credentials.required_slots != expected_slots {
        return Err(format!(
            "credentials.requiredSlots must be {:?} for the enabled connectors",
            expected_slots
        ));
    }
    validate_restrictions(&document.autonomy_restrictions)?;
    Ok(())
}

fn validate_restrictions(restrictions: &AutonomyRestrictions) -> Result<(), String> {
    for value in restrictions
        .ask_first
        .iter()
        .chain(restrictions.deny.iter())
    {
        storage::validate_identifier("autonomy restriction", value)?;
    }
    Ok(())
}

fn known_autonomy_action(value: &str) -> bool {
    matches!(
        value,
        "write_files"
            | "run_shell"
            | "modify_git"
            | "manage_processes"
            | "deploy"
            | "send_messages"
            | "use_browser"
            | "network"
            | "dependency_change"
            | "git_commit"
    )
}

fn required_slots(connectors: &ProjectDocumentConnectors) -> Vec<String> {
    if connectors.grok_mcp || connectors.xai_research_mcp {
        vec!["xai".to_owned()]
    } else {
        Vec::new()
    }
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:016x}", storage::stable_hash_bytes(bytes))
}

fn ensure_expected_digest(actual: &str, expected: &str) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err("project file changed after preview; review the latest file before applying".to_owned())
    }
}

fn write_project_document(
    project: &ProjectWorkspace,
    document: &ProjectDocumentV3,
) -> Result<(String, String), String> {
    let file_path = project_document_path(project, true)?;
    let payload = serde_json::to_vec_pretty(document)
        .map_err(|error| format!("failed to encode project file: {error}"))?;
    if payload.len() as u64 > MAX_PROJECT_FILE_BYTES {
        return Err("project file exceeds the 1 MiB size limit".to_owned());
    }
    let decoded: ProjectDocumentV3 = serde_json::from_slice(&payload)
        .map_err(|error| format!("generated project file failed validation: {error}"))?;
    validate_project_document(&decoded)?;

    let temporary_path = file_path.with_extension("json.tmp");
    if temporary_path.exists() {
        fs::remove_file(&temporary_path)
            .map_err(|error| format!("failed to clear stale project temporary file: {error}"))?;
    }
    let mut temporary = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary_path)
        .map_err(|error| format!("failed to create project temporary file: {error}"))?;
    temporary
        .write_all(&payload)
        .and_then(|_| temporary.sync_all())
        .map_err(|error| format!("failed to write project temporary file: {error}"))?;

    if file_path.exists() {
        let current = fs::read(&file_path)
            .map_err(|error| format!("failed to read existing project file: {error}"))?;
        if serde_json::from_slice::<ProjectDocumentV2>(&current).is_ok()
            || serde_json::from_slice::<ProjectDocumentV3>(&current)
                .ok()
                .filter(|value| validate_project_document(value).is_ok())
                .is_some()
        {
            fs::copy(&file_path, file_path.with_extension("json.bak"))
                .map_err(|error| format!("failed to back up existing project file: {error}"))?;
        }
    }
    fs::rename(&temporary_path, &file_path)
        .map_err(|error| format!("failed to install project file: {error}"))?;
    Ok((
        file_path.to_string_lossy().into_owned(),
        digest_bytes(&payload),
    ))
}

fn preview_project_document(
    connection: &rusqlite::Connection,
    database_path: &Path,
    project: &ProjectWorkspace,
) -> Result<ProjectFilePreview, String> {
    let path = Path::new(&project.path)
        .join(".agentdeck")
        .join("project.json");
    let invalid = |error: String, detected_format: Option<u32>| ProjectFilePreview {
        project_id: project.id.clone(),
        path: path.to_string_lossy().into_owned(),
        valid: false,
        detected_format,
        current_digest: project.project_file_digest.clone(),
        file_digest: None,
        changes: Vec::new(),
        warnings: Vec::new(),
        can_apply: false,
        error: Some(error),
    };
    let (document, digest) = match read_project_document(project) {
        Ok(result) => result,
        Err(error) => {
            return Ok(invalid(error, None));
        }
    };
    let current = storage::load_project_connector_settings(connection, project)?
        .unwrap_or_else(|| default_connector_settings(database_path, project));
    let mut changes = Vec::new();
    push_change(
        &mut changes,
        "metadata.name",
        &project.name,
        &document.metadata.name,
    );
    push_change(
        &mut changes,
        "autonomyRestrictions",
        &serde_json::to_string(&project.autonomy_restrictions).unwrap_or_default(),
        &serde_json::to_string(&document.autonomy_restrictions).unwrap_or_default(),
    );
    push_change(
        &mut changes,
        "metadata.description",
        &project.description,
        &document.metadata.description,
    );
    for (field, current_value, file_value) in [
        (
            "connectors.filesystem",
            current.filesystem_enabled,
            document.connectors.filesystem,
        ),
        (
            "connectors.git",
            current.git_enabled,
            document.connectors.git,
        ),
        (
            "connectors.claudeCodeServe",
            current.claude_code_serve_enabled,
            document.connectors.claude_code_serve,
        ),
        (
            "connectors.grokMcp",
            current.grok_mcp_enabled,
            document.connectors.grok_mcp,
        ),
        (
            "connectors.xaiResearchMcp",
            current.xai_research_mcp_enabled,
            document.connectors.xai_research_mcp,
        ),
    ] {
        push_change(
            &mut changes,
            field,
            &current_value.to_string(),
            &file_value.to_string(),
        );
    }
    let mut warnings = Vec::new();
    for action in document
        .autonomy_restrictions
        .ask_first
        .iter()
        .chain(document.autonomy_restrictions.deny.iter())
        .filter(|action| !known_autonomy_action(action))
    {
        warnings.push(format!(
            "Unknown autonomy action {action} will be ignored by this AgentDeck version."
        ));
    }
    for slot in &document.credentials.required_slots {
        if storage::read_provider_secret(database_path, slot)?.is_none() {
            warnings.push(format!(
                "Credential slot {slot} is not stored on this device; load can continue, but its connector will remain unavailable."
            ));
        }
    }
    let git_blocked = document.connectors.git && !Path::new(&project.path).join(".git").is_dir();
    if git_blocked {
        warnings.push(
            "Git connector cannot be applied because this folder is not a Git repository."
                .to_owned(),
        );
    }
    Ok(ProjectFilePreview {
        project_id: project.id.clone(),
        path: path.to_string_lossy().into_owned(),
        valid: true,
        detected_format: Some(PROJECT_FORMAT_VERSION),
        current_digest: project.project_file_digest.clone(),
        file_digest: Some(digest),
        changes,
        warnings,
        can_apply: !git_blocked,
        error: None,
    })
}

fn push_change(changes: &mut Vec<ProjectFileChange>, field: &str, current: &str, file: &str) {
    if current != file {
        changes.push(ProjectFileChange {
            field: field.to_owned(),
            current_value: current.to_owned(),
            file_value: file.to_owned(),
        });
    }
}

fn connector_request_from_document(
    document: &ProjectDocumentV3,
) -> SaveProjectConnectorSettingsRequest {
    SaveProjectConnectorSettingsRequest {
        filesystem_enabled: document.connectors.filesystem,
        git_enabled: document.connectors.git,
        claude_code_serve_enabled: document.connectors.claude_code_serve,
        grok_mcp_enabled: document.connectors.grok_mcp,
        xai_research_mcp_enabled: document.connectors.xai_research_mcp,
    }
}

fn bool_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn project_list(connection: &rusqlite::Connection) -> Result<ProjectWorkspaceList, String> {
    Ok(ProjectWorkspaceList {
        loaded_at: Utc::now().to_rfc3339(),
        projects: storage::load_project_workspaces(connection)?,
    })
}

fn canonical_project_path(raw_path: &str) -> Result<PathBuf, String> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err("project path is required".to_owned());
    }
    let expanded = if trimmed == "~" {
        home_directory()?
    } else if let Some(relative) = trimmed.strip_prefix("~/") {
        home_directory()?.join(relative)
    } else {
        PathBuf::from(trimmed)
    };
    let canonical = fs::canonicalize(&expanded)
        .map_err(|error| format!("project path is unavailable: {error}"))?;
    if !canonical.is_dir() {
        return Err("project path must be a directory".to_owned());
    }
    Ok(canonical)
}

fn home_directory() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_owned())
}

fn project_id(path: &Path) -> String {
    format!(
        "project:{:016x}",
        storage::stable_hash(&path.to_string_lossy())
    )
}

fn project_name(path: &Path, requested_name: Option<&str>) -> Result<String, String> {
    let name = requested_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            path.file_name()
                .and_then(|value| value.to_str())
                .map(str::to_owned)
        })
        .ok_or_else(|| "project name could not be derived".to_owned())?;
    storage::validate_identifier("project name", &name)?;
    Ok(name)
}

fn audit_project_action(
    connection: &rusqlite::Connection,
    action: &str,
    project_id: &str,
) -> Result<(), String> {
    let created_at = Utc::now().to_rfc3339();
    let audit_id = format!(
        "audit:{:016x}",
        storage::stable_hash(&format!("{action}:{project_id}:{created_at}"))
    );
    connection
        .execute(
            "INSERT INTO audit_events
                (id, action, status, model, conversation_id, duration_ms, created_at)
             VALUES (?1, ?2, 'success', 'project-registry', ?3, 0, ?4)",
            params![audit_id, action, project_id, created_at],
        )
        .map_err(|error| format!("failed to store project audit event: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_project(root: &Path) -> ProjectWorkspace {
        ProjectWorkspace {
            id: "project:test".to_owned(),
            name: "Test Project".to_owned(),
            description: "Portable project".to_owned(),
            path: root.to_string_lossy().into_owned(),
            exists: true,
            active: true,
            format_version: None,
            project_file_state: "legacy".to_owned(),
            project_file_digest: None,
            autonomy_restrictions: AutonomyRestrictions::default(),
            created_at: "2026-06-20T18:00:00Z".to_owned(),
            updated_at: "2026-06-20T18:00:00Z".to_owned(),
        }
    }

    fn test_document(project: &ProjectWorkspace) -> ProjectDocumentV3 {
        let settings = crate::models::ProjectConnectorSettings {
            project_id: project.id.clone(),
            project_name: project.name.clone(),
            project_path: project.path.clone(),
            filesystem_enabled: true,
            git_enabled: false,
            claude_code_serve_enabled: false,
            grok_mcp_enabled: false,
            xai_research_mcp_enabled: false,
            claude_export_path: "generated/claude.json".to_owned(),
            codex_export_path: "generated/codex.toml".to_owned(),
            claude_code_serve_export_path: "generated/serve.json".to_owned(),
            updated_at: "2026-06-20T18:00:00Z".to_owned(),
        };
        document_from_project(project, &settings)
    }

    #[test]
    fn canonical_paths_produce_stable_ids() {
        let directory = std::env::temp_dir().join(format!(
            "agentdeck-project-id-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        fs::create_dir_all(&directory).unwrap();
        let canonical = canonical_project_path(directory.to_str().unwrap()).unwrap();
        assert_eq!(project_id(&canonical), project_id(&canonical));
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn rejects_missing_project_paths() {
        assert!(canonical_project_path("/definitely/missing/agentdeck-project").is_err());
    }

    #[test]
    fn derives_name_from_directory() {
        assert_eq!(
            project_name(Path::new("/tmp/AgentDeck"), None).unwrap(),
            "AgentDeck"
        );
        assert_eq!(
            project_name(Path::new("/tmp/AgentDeck"), Some("Control Plane")).unwrap(),
            "Control Plane"
        );
    }

    #[test]
    fn project_v3_round_trips_without_machine_paths_or_secrets() {
        let project = test_project(Path::new("/tmp/agentdeck-project-v2"));
        let document = test_document(&project);
        let encoded = serde_json::to_string(&document).unwrap();
        let decoded: ProjectDocumentV3 = serde_json::from_str(&encoded).unwrap();

        assert_eq!(decoded, document);
        assert!(!encoded.contains(&project.path));
        assert!(!encoded.contains("ciphertext"));
        assert!(!encoded.contains("apiKey"));
    }

    #[test]
    fn project_v3_rejects_unknown_secret_fields_and_future_versions() {
        let project = test_project(Path::new("/tmp/agentdeck-project-v2"));
        let mut value = serde_json::to_value(test_document(&project)).unwrap();
        value["credentials"]["apiKey"] = Value::String("secret".to_owned());
        assert!(serde_json::from_value::<ProjectDocumentV3>(value).is_err());

        let mut future = test_document(&project);
        future.format_version = 4;
        assert!(validate_project_document(&future)
            .unwrap_err()
            .contains("formatVersion"));
    }

    #[test]
    fn project_v2_reads_as_v3_without_gaining_authority() {
        let directory = std::env::temp_dir().join(format!(
            "agentdeck-project-v2-upgrade-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        fs::create_dir_all(directory.join(".agentdeck")).unwrap();
        let project = test_project(&directory);
        let v3 = test_document(&project);
        let v2 = ProjectDocumentV2 {
            kind: v3.kind,
            format_version: 2,
            metadata: v3.metadata,
            connectors: v3.connectors,
            credentials: v3.credentials,
        };
        fs::write(
            directory.join(".agentdeck/project.json"),
            serde_json::to_vec_pretty(&v2).unwrap(),
        )
        .unwrap();
        let (upgraded, _) = read_project_document(&project).unwrap();
        assert_eq!(upgraded.format_version, 3);
        assert!(upgraded.autonomy_restrictions.ask_first.is_empty());
        assert!(upgraded.autonomy_restrictions.deny.is_empty());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn project_file_write_is_atomic_and_preserves_valid_backup() {
        let directory = std::env::temp_dir().join(format!(
            "agentdeck-project-v2-write-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        fs::create_dir_all(&directory).unwrap();
        let project = test_project(&directory);
        let first = test_document(&project);
        let (_, first_digest) = write_project_document(&project, &first).unwrap();
        let mut second = first.clone();
        second.metadata.description = "Updated".to_owned();
        second.metadata.updated_at = "2026-06-20T19:00:00Z".to_owned();
        let (_, second_digest) = write_project_document(&project, &second).unwrap();

        let file_path = directory.join(".agentdeck/project.json");
        let backup: ProjectDocumentV3 =
            serde_json::from_slice(&fs::read(file_path.with_extension("json.bak")).unwrap())
                .unwrap();
        assert_eq!(backup, first);
        assert_ne!(first_digest, second_digest);
        assert!(!file_path.with_extension("json.tmp").exists());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn preview_warns_for_missing_referenced_credential_without_blocking_apply() {
        let directory = std::env::temp_dir().join(format!(
            "agentdeck-project-v2-preview-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let project_root = directory.join("project");
        fs::create_dir_all(&project_root).unwrap();
        let database_path = directory.join("agentdeck.sqlite3");
        let connection = storage::open_database(&database_path).unwrap();
        let project = test_project(&project_root);
        connection
            .execute(
                "INSERT INTO project_workspaces
                (id, name, description, path, active, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6)",
                params![
                    project.id,
                    project.name,
                    project.description,
                    project.path,
                    project.created_at,
                    project.updated_at
                ],
            )
            .unwrap();
        let mut document = test_document(&project);
        document.connectors.grok_mcp = true;
        document.credentials.required_slots = vec!["xai".to_owned()];
        write_project_document(&project, &document).unwrap();

        let preview = preview_project_document(&connection, &database_path, &project).unwrap();
        assert!(preview.valid);
        assert!(preview.can_apply);
        assert!(preview
            .warnings
            .iter()
            .any(|warning| warning.contains("xai")));
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn stale_preview_digest_is_rejected() {
        assert!(ensure_expected_digest("new", "old")
            .unwrap_err()
            .contains("changed after preview"));
        assert!(ensure_expected_digest("same", "same").is_ok());
    }
}
