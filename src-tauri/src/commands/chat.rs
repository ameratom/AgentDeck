use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use rusqlite::{params, Connection};
use tauri::ipc::Channel;
use tauri::AppHandle;

use crate::commands::chat_providers;
use crate::commands::providers;
use crate::models::{
    ChatMessage, ChatPreferences, ChatRequest, ChatResponse, ChatStreamEvent, LocalModel,
};
use crate::storage;

const MAX_MESSAGES: usize = 80;
const MAX_MESSAGE_CHARS: usize = 32_000;
const MAX_TOTAL_CHARS: usize = 120_000;

static CHAT_CANCEL_FLAG: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

#[tauri::command]
pub async fn list_lm_studio_models() -> Result<Vec<LocalModel>, String> {
    let definition = providers::find_provider("lm-studio")?;
    tauri::async_runtime::spawn_blocking(move || {
        let base_url = providers::provider_base_url(&definition);
        providers::fetch_provider_models_blocking(&definition, &base_url)
    })
    .await
    .map_err(|error| format!("model discovery task failed: {error}"))?
}

#[tauri::command]
pub async fn load_chat_preferences(app: AppHandle) -> Result<ChatPreferences, String> {
    let database_path = database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || storage::load_chat_preferences(&database_path))
        .await
        .map_err(|error| format!("chat preferences load failed: {error}"))?
}

#[tauri::command]
pub async fn save_chat_preferences(
    app: AppHandle,
    preferences: ChatPreferences,
) -> Result<ChatPreferences, String> {
    let database_path = database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        storage::save_chat_preferences(&database_path, &preferences)?;
        storage::load_chat_preferences(&database_path)
    })
    .await
    .map_err(|error| format!("chat preferences save failed: {error}"))?
}

#[tauri::command]
pub async fn send_chat_message(
    app: AppHandle,
    request: ChatRequest,
) -> Result<ChatResponse, String> {
    validate_chat_request(&request)?;
    let database_path = database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || send_chat_blocking(database_path, request))
        .await
        .map_err(|error| format!("chat task failed: {error}"))?
}

#[tauri::command]
pub async fn stream_chat_message(
    app: AppHandle,
    request: ChatRequest,
    on_event: Channel<ChatStreamEvent>,
) -> Result<ChatResponse, String> {
    validate_chat_request(&request)?;
    let database_path = database_path(&app)?;
    let contextual_messages = project_scoped_messages(&database_path, &request)?;
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut guard = CHAT_CANCEL_FLAG
            .lock()
            .map_err(|_| "chat cancellation lock poisoned".to_owned())?;
        *guard = Some(cancelled.clone());
    }

    let definition = providers::find_provider(&request.provider_id)?;
    let verification_definition = definition.clone();
    let verification_model = request.model.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let base_url = providers::provider_base_url(&verification_definition);
        providers::verify_provider_model(&verification_definition, &base_url, &verification_model)
    })
    .await
    .map_err(|error| format!("provider verification task failed: {error}"))??;
    let started_at = Utc::now();
    let stream_result = chat_providers::stream_provider_chat(
        &definition,
        &request.model,
        &contextual_messages,
        request.enable_agent_tools,
        &on_event,
        cancelled.clone(),
    )
    .await;

    {
        let mut guard = CHAT_CANCEL_FLAG
            .lock()
            .map_err(|_| "chat cancellation lock poisoned".to_owned())?;
        *guard = None;
    }

    match stream_result {
        Ok((content, finish_reason)) => {
            let response =
                persist_chat_exchange(&database_path, &request, &content, started_at, "success")?;
            let _ = on_event.send(ChatStreamEvent::Done {
                finish_reason: finish_reason.clone(),
                message: response.message.clone(),
            });
            Ok(response)
        }
        Err(error) => {
            let _ = store_audit_event(
                &database_path,
                "chat.stream",
                "error",
                &request.model,
                &request.conversation_id,
                started_at,
            );
            let _ = on_event.send(ChatStreamEvent::Error {
                message: error.clone(),
            });
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn cancel_stream_chat() -> Result<(), String> {
    let guard = CHAT_CANCEL_FLAG
        .lock()
        .map_err(|_| "chat cancellation lock poisoned".to_owned())?;
    if let Some(flag) = guard.as_ref() {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
pub async fn load_chat_messages(
    app: AppHandle,
    conversation_id: String,
) -> Result<Vec<ChatMessage>, String> {
    storage::validate_identifier("conversation ID", &conversation_id)?;
    let database_path = database_path(&app)?;

    tauri::async_runtime::spawn_blocking(move || load_messages(&database_path, &conversation_id))
        .await
        .map_err(|error| format!("message load task failed: {error}"))?
}

#[tauri::command]
pub async fn clear_chat_messages(
    app: AppHandle,
    conversation_id: String,
) -> Result<(), String> {
    storage::validate_identifier("conversation ID", &conversation_id)?;
    let database_path = database_path(&app)?;

    tauri::async_runtime::spawn_blocking(move || {
        let started_at = Utc::now();
        let connection = open_database(&database_path)?;
        connection
            .execute(
                "DELETE FROM chat_messages WHERE conversation_id = ?1",
                params![conversation_id],
            )
            .map_err(|error| format!("failed to clear chat messages: {error}"))?;
        store_audit_event(
            &database_path,
            "chat.clear",
            "success",
            "",
            &conversation_id,
            started_at,
        )?;
        Ok(())
    })
    .await
    .map_err(|error| format!("chat clear task failed: {error}"))?
}

fn send_chat_blocking(
    database_path: PathBuf,
    request: ChatRequest,
) -> Result<ChatResponse, String> {
    let started_at = Utc::now();
    let definition = providers::find_provider(&request.provider_id)?;
    let messages = project_scoped_messages(&database_path, &request)?;
    let completion_result =
        chat_providers::complete_provider_chat(&definition, &request.model, &messages);

    match completion_result {
        Ok((content, finish_reason)) => {
            persist_chat_exchange(&database_path, &request, &content, started_at, "success").map(
                |mut response| {
                    response.finish_reason = finish_reason;
                    response
                },
            )
        }
        Err(error) => {
            let _ = store_audit_event(
                &database_path,
                "chat.complete",
                "error",
                &request.model,
                &request.conversation_id,
                started_at,
            );
            Err(error)
        }
    }
}

fn project_scoped_messages(
    database_path: &Path,
    request: &ChatRequest,
) -> Result<Vec<crate::models::ChatMessageInput>, String> {
    let Some(project_id) = request.project_id.as_deref() else {
        return Ok(request.messages.clone());
    };
    let connection = open_database(database_path)?;
    let project = storage::require_active_project(&connection, project_id)?;
    let mut messages = Vec::with_capacity(request.messages.len() + 1);
    messages.push(crate::models::ChatMessageInput {
        role: "system".to_owned(),
        content: format!(
            "Active AgentDeck project: {}. Project root: {}. Keep file references and workspace actions scoped to this project unless the user explicitly asks otherwise.",
            project.name, project.path
        ),
    });
    messages.extend(request.messages.clone());
    Ok(messages)
}

fn persist_chat_exchange(
    database_path: &Path,
    request: &ChatRequest,
    content: &str,
    started_at: chrono::DateTime<Utc>,
    audit_status: &str,
) -> Result<ChatResponse, String> {
    let connection = open_database(database_path)?;
    let user_message = request
        .messages
        .last()
        .ok_or_else(|| "at least one message is required".to_owned())?;
    store_message(
        &connection,
        &request.conversation_id,
        "user",
        &user_message.content,
        &request.model,
    )?;
    let assistant = store_message(
        &connection,
        &request.conversation_id,
        "assistant",
        content,
        &request.model,
    )?;
    store_audit_event(
        database_path,
        "chat.complete",
        audit_status,
        &request.model,
        &request.conversation_id,
        started_at,
    )?;
    Ok(ChatResponse {
        message: assistant,
        finish_reason: None,
    })
}

fn open_database(path: &Path) -> Result<Connection, String> {
    storage::open_database(path)
}

fn database_path(app: &AppHandle) -> Result<PathBuf, String> {
    storage::database_path(app)
}

fn store_message(
    connection: &Connection,
    conversation_id: &str,
    role: &str,
    content: &str,
    model: &str,
) -> Result<ChatMessage, String> {
    let created_at = Utc::now().to_rfc3339();
    let id = format!(
        "message:{:016x}",
        storage::stable_hash(&format!("{conversation_id}:{role}:{created_at}:{content}"))
    );
    connection
        .execute(
            "INSERT INTO chat_messages
                (id, conversation_id, role, content, model, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, conversation_id, role, content, model, created_at],
        )
        .map_err(|error| format!("failed to store chat message: {error}"))?;

    Ok(ChatMessage {
        id: Some(id),
        conversation_id: conversation_id.to_owned(),
        role: role.to_owned(),
        content: content.to_owned(),
        model: model.to_owned(),
        created_at: Some(created_at),
    })
}

fn load_messages(path: &Path, conversation_id: &str) -> Result<Vec<ChatMessage>, String> {
    let connection = open_database(path)?;
    let mut statement = connection
        .prepare(
            "SELECT id, conversation_id, role, content, model, created_at
             FROM chat_messages
             WHERE conversation_id = ?1
             ORDER BY created_at ASC",
        )
        .map_err(|error| format!("failed to prepare message query: {error}"))?;
    let rows = statement
        .query_map([conversation_id], |row| {
            Ok(ChatMessage {
                id: Some(row.get(0)?),
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                model: row.get(4)?,
                created_at: Some(row.get(5)?),
            })
        })
        .map_err(|error| format!("failed to load chat messages: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to decode chat messages: {error}"))
}

fn store_audit_event(
    path: &Path,
    action: &str,
    status: &str,
    model: &str,
    conversation_id: &str,
    started_at: chrono::DateTime<Utc>,
) -> Result<(), String> {
    let connection = open_database(path)?;
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
    Ok(())
}

fn validate_chat_request(request: &ChatRequest) -> Result<(), String> {
    storage::validate_identifier("conversation ID", &request.conversation_id)?;
    if let Some(project_id) = &request.project_id {
        storage::validate_identifier("project ID", project_id)?;
    }
    storage::validate_identifier("provider ID", &request.provider_id)?;
    storage::validate_identifier("model", &request.model)?;
    providers::find_provider(&request.provider_id)?;
    if request.messages.is_empty() || request.messages.len() > MAX_MESSAGES {
        return Err(format!(
            "messages must contain between 1 and {MAX_MESSAGES} entries"
        ));
    }

    let mut total_chars = 0;
    for message in &request.messages {
        if !matches!(message.role.as_str(), "system" | "user" | "assistant") {
            return Err(format!("unsupported message role: {}", message.role));
        }
        let chars = message.content.chars().count();
        if chars == 0 || chars > MAX_MESSAGE_CHARS {
            return Err(format!(
                "each message must contain between 1 and {MAX_MESSAGE_CHARS} characters"
            ));
        }
        total_chars += chars;
    }
    if total_chars > MAX_TOTAL_CHARS {
        return Err(format!(
            "combined message content exceeds {MAX_TOTAL_CHARS} characters"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ChatMessageInput;

    fn valid_request() -> ChatRequest {
        ChatRequest {
            conversation_id: "conversation:test".to_owned(),
            project_id: None,
            provider_id: "lm-studio".to_owned(),
            model: "local-model".to_owned(),
            messages: vec![ChatMessageInput {
                role: "user".to_owned(),
                content: "Hello".to_owned(),
            }],
            enable_agent_tools: false,
        }
    }

    #[test]
    fn validates_chat_request() {
        assert!(validate_chat_request(&valid_request()).is_ok());
    }

    #[test]
    fn rejects_unknown_roles() {
        let mut request = valid_request();
        request.messages[0].role = "tool".to_owned();
        assert!(validate_chat_request(&request).is_err());
    }

    #[test]
    fn adds_verified_active_project_context() {
        let directory = std::env::temp_dir().join(format!(
            "agentdeck-chat-project-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let database_path = directory.join("agentdeck.sqlite3");
        let connection = storage::open_database(&database_path).unwrap();
        connection
            .execute(
                "INSERT INTO project_workspaces
                    (id, name, path, active, created_at, updated_at)
                 VALUES (?1, 'Chat Project', ?2, 1, 'now', 'now')",
                params!["project:chat", directory.to_string_lossy()],
            )
            .unwrap();

        let mut request = valid_request();
        request.project_id = Some("project:chat".to_owned());
        let messages = project_scoped_messages(&database_path, &request).unwrap();
        assert_eq!(messages[0].role, "system");
        assert!(messages[0].content.contains("Chat Project"));

        request.project_id = Some("project:stale".to_owned());
        assert!(project_scoped_messages(&database_path, &request).is_err());
        drop(connection);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn stores_and_loads_messages() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "
                CREATE TABLE chat_messages (
                    id TEXT PRIMARY KEY,
                    conversation_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    model TEXT NOT NULL,
                    created_at TEXT NOT NULL
                );
                ",
            )
            .unwrap();
        let message =
            store_message(&connection, "conversation:test", "user", "Hello", "model").unwrap();
        assert_eq!(message.content, "Hello");
    }
}
