use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{params, Connection};
use tauri::AppHandle;

use crate::commands::providers;
use crate::commands::webhooks;
use crate::models::{HandoffRequest, HandoffRun};
use crate::permissions;
use crate::storage;

const MAX_TEXT_CHARS: usize = 32_000;

#[tauri::command]
pub async fn run_handoff(app: AppHandle, request: HandoffRequest) -> Result<HandoffRun, String> {
    validate_handoff_request(&request)?;
    let database_path = database_path(&app)?;

    tauri::async_runtime::spawn_blocking(move || dispatch_handoff(&database_path, request))
        .await
        .map_err(|error| format!("handoff task failed: {error}"))?
}

#[tauri::command]
pub async fn load_handoff_runs(
    app: AppHandle,
    limit: Option<u32>,
) -> Result<Vec<HandoffRun>, String> {
    let database_path = database_path(&app)?;
    let limit = limit.unwrap_or(12).clamp(1, 100) as usize;

    tauri::async_runtime::spawn_blocking(move || load_runs(&database_path, limit))
        .await
        .map_err(|error| format!("handoff run load task failed: {error}"))?
}

pub(crate) fn dispatch_handoff(path: &Path, request: HandoffRequest) -> Result<HandoffRun, String> {
    if request.approvals.is_empty() {
        return Err("Handoff requires explicit approval before dispatch".to_owned());
    }

    let connection = storage::open_database(path)?;
    let project = request
        .project_id
        .as_deref()
        .map(|project_id| storage::require_active_project(&connection, project_id))
        .transpose()?;
    permissions::require_permission(&connection, &request.source_agent_id, "dispatch-handoff")?;
    let started_at = Utc::now();
    let run_id = run_identifier(&request, started_at);
    let thread_id = thread_identifier(&request);
    let approvals = serde_json::to_string(&request.approvals)
        .map_err(|error| format!("failed to encode handoff approvals: {error}"))?;
    let created_at = started_at.to_rfc3339();

    connection
        .execute(
            "INSERT INTO handoff_runs
                (id, project_id, thread_id, source_agent_id, source_agent_name, target_provider_id,
                 target_provider_name, target_model_id, title, task, context, status,
                 output, error, approvals, audit_ref, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'running', '', NULL, ?12, NULL, ?13, ?13)",
            params![
                run_id,
                request.project_id,
                thread_id,
                request.source_agent_id,
                request.source_agent_name,
                request.target_provider_id,
                request.target_provider_name,
                request.target_model_id,
                request.title,
                request.task,
                request.context,
                approvals,
                created_at,
            ],
        )
        .map_err(|error| format!("failed to create handoff run: {error}"))?;

    let prompt = build_handoff_prompt(&request, project.as_ref());
    let dispatch_result = providers::dispatch_provider_handoff(
        &request.target_provider_id,
        &request.target_model_id,
        &request.title,
        &request.task,
        &request.context,
        &request.source_agent_name,
        &prompt,
    );

    match dispatch_result {
        Ok((output, finish_reason)) => {
            let audit_ref = store_audit_event(
                &path,
                &connection,
                "handoff.dispatch",
                "success",
                &request.target_model_id,
                &thread_id,
                started_at,
            )?;
            update_run(
                &connection,
                &run_id,
                "completed",
                &output,
                None,
                Some(&audit_ref),
            )?;
            let run = storage::load_handoff_run(&connection, &run_id)?;
            webhooks::emit_webhook_events(
                path,
                "handoff.completed",
                serde_json::json!({
                    "runId": run.id,
                    "threadId": run.thread_id,
                    "projectId": run.project_id,
                    "status": run.status,
                    "title": run.title,
                    "sourceAgentId": run.source_agent_id,
                    "targetProviderId": run.target_provider_id,
                    "targetModelId": run.target_model_id,
                    "auditRef": run.audit_ref,
                }),
            );
            let _ = finish_reason;
            Ok(run)
        }
        Err(error) => {
            let audit_ref = store_audit_event(
                &path,
                &connection,
                "handoff.dispatch",
                "error",
                &request.target_model_id,
                &thread_id,
                started_at,
            )?;
            update_run(
                &connection,
                &run_id,
                "failed",
                "",
                Some(&error),
                Some(&audit_ref),
            )?;
            let run = storage::load_handoff_run(&connection, &run_id)?;
            webhooks::emit_webhook_events(
                path,
                "handoff.failed",
                serde_json::json!({
                    "runId": run.id,
                    "threadId": run.thread_id,
                    "projectId": run.project_id,
                    "status": run.status,
                    "title": run.title,
                    "sourceAgentId": run.source_agent_id,
                    "targetProviderId": run.target_provider_id,
                    "targetModelId": run.target_model_id,
                    "error": run.error,
                    "auditRef": run.audit_ref,
                }),
            );
            Ok(run)
        }
    }
}

pub(crate) fn load_recent_runs(path: &Path, limit: usize) -> Result<Vec<HandoffRun>, String> {
    load_runs(path, limit)
}

fn load_runs(path: &Path, limit: usize) -> Result<Vec<HandoffRun>, String> {
    let connection = storage::open_database(path)?;
    let mut statement = connection
        .prepare(
            "SELECT id, project_id, thread_id, source_agent_id, source_agent_name, target_provider_id,
                    target_provider_name, target_model_id, title, task, context, status,
                    output, error, approvals, audit_ref, created_at, updated_at
             FROM handoff_runs
             ORDER BY created_at DESC
             LIMIT ?1",
        )
        .map_err(|error| format!("failed to prepare handoff run query: {error}"))?;
    let rows = statement
        .query_map([limit as i64], |row| {
            let approvals: String = row.get(14)?;
            let approvals = serde_json::from_str::<Vec<String>>(&approvals).unwrap_or_default();
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
                approvals,
                audit_ref: row.get(15)?,
                created_at: row.get(16)?,
                updated_at: row.get(17)?,
                mission_id: None,
                parent_run_id: None,
                required_capabilities: Vec::new(),
            })
        })
        .map_err(|error| format!("failed to load handoff runs: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to decode handoff runs: {error}"))
}

fn update_run(
    connection: &Connection,
    run_id: &str,
    status: &str,
    output: &str,
    error: Option<&str>,
    audit_ref: Option<&str>,
) -> Result<(), String> {
    let updated_at = Utc::now().to_rfc3339();
    connection
        .execute(
            "UPDATE handoff_runs
             SET status = ?2, output = ?3, error = ?4, audit_ref = ?5, updated_at = ?6
             WHERE id = ?1",
            params![run_id, status, output, error, audit_ref, updated_at],
        )
        .map_err(|error| format!("failed to update handoff run: {error}"))?;
    Ok(())
}

fn store_audit_event(
    path: &Path,
    connection: &Connection,
    action: &str,
    status: &str,
    model: &str,
    conversation_id: &str,
    started_at: chrono::DateTime<Utc>,
) -> Result<String, String> {
    let created_at = Utc::now();
    let id = format!(
        "audit:{:016x}",
        storage::stable_hash(&format!("{action}:{conversation_id}:{created_at}"))
    );
    connection
        .execute(
            "INSERT INTO audit_events
                (id, action, status, model, conversation_id, duration_ms, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                action,
                status,
                model,
                conversation_id,
                (created_at - started_at).num_milliseconds(),
                created_at.to_rfc3339()
            ],
        )
        .map_err(|error| format!("failed to store audit event: {error}"))?;
    storage::append_log_event(
        path,
        "audit_event",
        serde_json::json!({
            "id": id,
            "action": action,
            "status": status,
            "model": model,
            "conversationId": conversation_id,
            "durationMs": (created_at - started_at).num_milliseconds(),
            "createdAt": created_at.to_rfc3339(),
        }),
    );
    Ok(id)
}

fn database_path(app: &AppHandle) -> Result<PathBuf, String> {
    storage::database_path(app)
}

fn validate_handoff_request(request: &HandoffRequest) -> Result<(), String> {
    if request.approvals.is_empty() {
        return Err("Handoff requires explicit approval before dispatch".to_owned());
    }
    if let Some(project_id) = &request.project_id {
        storage::validate_identifier("project ID", project_id)?;
    }
    storage::validate_identifier("source agent ID", &request.source_agent_id)?;
    storage::validate_identifier("source agent name", &request.source_agent_name)?;
    storage::validate_identifier("target provider ID", &request.target_provider_id)?;
    storage::validate_identifier("target provider name", &request.target_provider_name)?;
    storage::validate_identifier("target model ID", &request.target_model_id)?;
    validate_text("title", &request.title)?;
    validate_text("task", &request.task)?;
    validate_optional_text("context", &request.context)?;
    Ok(())
}

fn validate_text(label: &str, value: &str) -> Result<(), String> {
    let length = value.chars().count();
    if length == 0 || length > MAX_TEXT_CHARS {
        return Err(format!(
            "{label} must contain between 1 and {MAX_TEXT_CHARS} characters"
        ));
    }
    Ok(())
}

fn validate_optional_text(label: &str, value: &str) -> Result<(), String> {
    let length = value.chars().count();
    if length > MAX_TEXT_CHARS {
        return Err(format!(
            "{label} must contain at most {MAX_TEXT_CHARS} characters"
        ));
    }
    Ok(())
}

fn build_handoff_prompt(
    request: &HandoffRequest,
    project: Option<&crate::models::ProjectWorkspace>,
) -> String {
    let project_context = project
        .map(|project| format!("Project: {}\nProject root: {}\n", project.name, project.path))
        .unwrap_or_default();
    format!(
        "Manual AgentDeck handoff\n\n{}Source agent: {}\nTarget provider: {}\nTarget model: {}\nTitle: {}\nTask:\n{}\n\nContext:\n{}\n\nKeep workspace actions scoped to the project root. Return a concise implementation or review result with concrete next steps.",
        project_context,
        request.source_agent_name,
        request.target_provider_name,
        request.target_model_id,
        request.title,
        request.task,
        request.context
    )
}

fn thread_identifier(request: &HandoffRequest) -> String {
    format!(
        "handoff:{}:{}:{}",
        request.project_id.as_deref().unwrap_or("global"),
        request.source_agent_id,
        request.target_provider_id
    )
}

fn run_identifier(request: &HandoffRequest, started_at: chrono::DateTime<Utc>) -> String {
    format!(
        "run:{:016x}",
        storage::stable_hash(&format!(
            "{}:{}:{}:{}",
            request.source_agent_id, request.target_provider_id, request.title, started_at
        ))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_request() -> HandoffRequest {
        HandoffRequest {
            project_id: None,
            source_agent_id: "agent:codex".to_owned(),
            source_agent_name: "Codex".to_owned(),
            target_provider_id: "lm-studio".to_owned(),
            target_provider_name: "LM Studio".to_owned(),
            target_model_id: "qwen/qwen3.5-9b".to_owned(),
            title: "Review the changes".to_owned(),
            task: "Summarize the changes.".to_owned(),
            context: "Focus on phase 6.".to_owned(),
            approvals: vec!["user-approved".to_owned()],
            mission_id: None,
            parent_run_id: None,
            required_capabilities: Vec::new(),
        }
    }

    #[test]
    fn rejects_handoff_without_approval() {
        let mut request = valid_request();
        request.approvals.clear();
        assert!(validate_handoff_request(&request).is_err());
    }

    #[test]
    fn validates_handoff_request() {
        assert!(validate_handoff_request(&valid_request()).is_ok());
    }

    #[test]
    fn rejects_empty_task() {
        let mut request = valid_request();
        request.task.clear();
        assert!(validate_handoff_request(&request).is_err());
    }

    #[test]
    fn allows_empty_context() {
        let mut request = valid_request();
        request.context.clear();
        assert!(validate_handoff_request(&request).is_ok());
    }

    #[test]
    fn builds_thread_identifier_from_source_and_target() {
        let request = valid_request();
        assert_eq!(
            thread_identifier(&request),
            "handoff:global:agent:codex:lm-studio"
        );
    }

    #[test]
    fn includes_project_in_thread_identifier() {
        let mut request = valid_request();
        request.project_id = Some("project:agentdeck".to_owned());
        assert_eq!(
            thread_identifier(&request),
            "handoff:project:agentdeck:agent:codex:lm-studio"
        );
    }
}
