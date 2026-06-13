use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use tauri::{AppHandle, Emitter};

use crate::models::{ProjectWorkspaceList, RegisterProjectRequest};
use crate::storage;

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
            .execute("DELETE FROM project_workspaces WHERE id = ?1", [&project_id])
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
}
