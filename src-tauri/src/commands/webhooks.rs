use std::path::Path;

use chrono::Utc;
use rusqlite::{params, Connection};
use tauri::AppHandle;

use crate::models::{
    SaveWebhookEndpointsRequest, WebhookDispatchRequest, WebhookDispatchResult,
    WebhookEndpointMatrix, WebhookSecretRequest,
};
use crate::permissions;
use crate::secrets;
use crate::storage;
use crate::webhooks;

const WEBHOOKS_PLUGIN_ID: &str = "agentdeck-plugin-webhooks";

#[tauri::command]
pub async fn load_webhook_endpoints(app: AppHandle) -> Result<WebhookEndpointMatrix, String> {
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let connection = storage::open_database(&database_path)?;
        let plugin_enabled = storage::plugin_enabled(&connection, WEBHOOKS_PLUGIN_ID)?;
        let endpoints = storage::load_webhook_endpoints(&connection, &database_path)?;
        Ok(WebhookEndpointMatrix {
            loaded_at: Utc::now().to_rfc3339(),
            plugin_enabled,
            endpoints,
        })
    })
    .await
    .map_err(|error| format!("webhook endpoint load task failed: {error}"))?
}

#[tauri::command]
pub async fn save_webhook_endpoints(
    app: AppHandle,
    request: SaveWebhookEndpointsRequest,
) -> Result<WebhookEndpointMatrix, String> {
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let connection = storage::open_database(&database_path)?;
        let endpoints =
            storage::replace_webhook_endpoints(&connection, &database_path, &request.endpoints)?;
        let plugin_enabled = storage::plugin_enabled(&connection, WEBHOOKS_PLUGIN_ID)?;
        storage::append_log_event(
            &database_path,
            "webhook.endpoints.save",
            serde_json::json!({
                "count": endpoints.len(),
            }),
        );
        Ok(WebhookEndpointMatrix {
            loaded_at: Utc::now().to_rfc3339(),
            plugin_enabled,
            endpoints,
        })
    })
    .await
    .map_err(|error| format!("webhook endpoint save task failed: {error}"))?
}

#[tauri::command]
pub async fn save_webhook_secret(
    app: AppHandle,
    request: WebhookSecretRequest,
) -> Result<WebhookEndpointMatrix, String> {
    storage::validate_identifier("webhook endpoint ID", &request.endpoint_id)?;
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let connection = storage::open_database(&database_path)?;
        ensure_endpoint_exists(&connection, &request.endpoint_id)?;
        let slot = storage::webhook_secret_slot(&request.endpoint_id);
        let secret = request.secret.trim().to_owned();
        if secret.is_empty() {
            storage::delete_provider_secret(&database_path, &slot)?;
        } else {
            if secret.len() < 8 {
                return Err("webhook signing secret must contain at least 8 characters".to_owned());
            }
            let master = secrets::master_key(&database_path)?;
            let ciphertext = secrets::encrypt(&master, &secret)?;
            storage::store_provider_secret(&database_path, &slot, &ciphertext)?;
        }
        storage::append_log_event(
            &database_path,
            "webhook.secret.save",
            serde_json::json!({
                "endpointId": request.endpoint_id,
                "cleared": secret.is_empty(),
            }),
        );
        let endpoints = storage::load_webhook_endpoints(&connection, &database_path)?;
        let plugin_enabled = storage::plugin_enabled(&connection, WEBHOOKS_PLUGIN_ID)?;
        Ok(WebhookEndpointMatrix {
            loaded_at: Utc::now().to_rfc3339(),
            plugin_enabled,
            endpoints,
        })
    })
    .await
    .map_err(|error| format!("webhook secret save task failed: {error}"))?
}

#[tauri::command]
pub async fn dispatch_webhook_event(
    app: AppHandle,
    request: WebhookDispatchRequest,
) -> Result<WebhookDispatchResult, String> {
    if request.approvals.is_empty() {
        return Err("Webhook dispatch requires explicit approval".to_owned());
    }
    storage::validate_identifier("webhook endpoint ID", &request.endpoint_id)?;
    webhooks::validate_event_type(&request.event_type)?;

    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        dispatch_webhook(&database_path, request)
    })
    .await
    .map_err(|error| format!("webhook dispatch task failed: {error}"))?
}

fn dispatch_webhook(
    database_path: &Path,
    request: WebhookDispatchRequest,
) -> Result<WebhookDispatchResult, String> {
    let connection = storage::open_database(database_path)?;
    if !storage::plugin_enabled(&connection, WEBHOOKS_PLUGIN_ID)? {
        return Err("Webhooks plugin is disabled".to_owned());
    }
    permissions::require_permission(&connection, "agent:agentdeck", "dispatch-handoff")?;

    let endpoint = storage::load_webhook_endpoints(&connection, database_path)?
        .into_iter()
        .find(|endpoint| endpoint.id == request.endpoint_id)
        .ok_or_else(|| "webhook endpoint was not found".to_owned())?;
    if !endpoint.enabled {
        return Err("webhook endpoint is disabled".to_owned());
    }
    if !endpoint.event_types.contains(&request.event_type) {
        return Err(format!(
            "webhook endpoint is not subscribed to {}",
            request.event_type
        ));
    }

    let secret = read_webhook_secret(database_path, &endpoint.id)?;
    let started_at = Utc::now();
    let dispatch_result =
        webhooks::dispatch_outbound(&endpoint.url, secret.as_deref(), &request.event_type, request.payload);
    let dispatched_at = Utc::now().to_rfc3339();

    let (success, status_code, detail, audit_status) = match &dispatch_result {
        Ok(response) if (200..300).contains(&response.status_code) => (
            true,
            response.status_code,
            response.detail.clone(),
            "success",
        ),
        Ok(response) => (
            false,
            response.status_code,
            response.detail.clone(),
            "error",
        ),
        Err(error) => (false, 0_u16, error.clone(), "error"),
    };

    let audit_ref = store_webhook_audit(
        database_path,
        &connection,
        &request.endpoint_id,
        &request.event_type,
        audit_status,
        started_at,
    )?;
    storage::append_log_event(
        database_path,
        "webhook.dispatch",
        serde_json::json!({
            "endpointId": request.endpoint_id,
            "eventType": request.event_type,
            "statusCode": status_code,
            "success": success,
            "detail": detail,
            "auditRef": audit_ref,
            "approvals": request.approvals,
        }),
    );

    dispatch_result?;

    Ok(WebhookDispatchResult {
        endpoint_id: request.endpoint_id,
        event_type: request.event_type,
        status_code,
        success,
        detail,
        audit_ref,
        dispatched_at,
    })
}

fn read_webhook_secret(database_path: &Path, endpoint_id: &str) -> Result<Option<String>, String> {
    let slot = storage::webhook_secret_slot(endpoint_id);
    let Some(ciphertext) = storage::read_provider_secret(database_path, &slot)? else {
        return Ok(None);
    };
    let master = secrets::master_key(database_path)?;
    let secret = secrets::decrypt(&master, &ciphertext)?;
    Ok(Some(secret))
}

fn ensure_endpoint_exists(connection: &Connection, endpoint_id: &str) -> Result<(), String> {
    let exists: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM webhook_endpoints WHERE id = ?1",
            params![endpoint_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("failed to query webhook endpoint: {error}"))?;
    if exists == 0 {
        return Err("webhook endpoint was not found".to_owned());
    }
    Ok(())
}

fn store_webhook_audit(
    path: &Path,
    connection: &Connection,
    endpoint_id: &str,
    event_type: &str,
    status: &str,
    started_at: chrono::DateTime<Utc>,
) -> Result<String, String> {
    let created_at = Utc::now();
    let id = format!(
        "audit:{:016x}",
        storage::stable_hash(&format!("webhook.dispatch:{endpoint_id}:{event_type}:{created_at}"))
    );
    connection
        .execute(
            "INSERT INTO audit_events
                (id, action, status, model, conversation_id, duration_ms, created_at)
             VALUES (?1, 'webhook.dispatch', ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                status,
                event_type,
                endpoint_id,
                (created_at - started_at).num_milliseconds(),
                created_at.to_rfc3339()
            ],
        )
        .map_err(|error| format!("failed to store webhook audit event: {error}"))?;
    storage::append_log_event(
        path,
        "audit_event",
        serde_json::json!({
            "id": id,
            "action": "webhook.dispatch",
            "status": status,
            "model": event_type,
            "conversationId": endpoint_id,
            "durationMs": (created_at - started_at).num_milliseconds(),
            "createdAt": created_at.to_rfc3339(),
        }),
    );
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::WebhookEndpoint;

    fn sample_endpoint(id: &str) -> WebhookEndpoint {
        WebhookEndpoint {
            id: id.to_owned(),
            name: "Test hook".to_owned(),
            url: "http://127.0.0.1:9/unreachable".to_owned(),
            enabled: true,
            event_types: vec!["test.ping".to_owned()],
            has_secret: false,
            updated_at: Utc::now().to_rfc3339(),
        }
    }

    #[test]
    fn dispatch_requires_plugin_enabled() {
        let path = std::env::temp_dir().join(format!(
            "agentdeck-webhook-dispatch-{}.sqlite3",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let connection = storage::open_database(&path).expect("open database");
        storage::replace_webhook_endpoints(&connection, &path, &[sample_endpoint("webhook:test")])
            .expect("save endpoint");
        connection
            .execute(
                "INSERT INTO plugin_settings (plugin_id, enabled, updated_at)
                 VALUES (?1, 0, ?2)",
                params![WEBHOOKS_PLUGIN_ID, Utc::now().to_rfc3339()],
            )
            .expect("disable plugin");

        let result = dispatch_webhook(
            &path,
            WebhookDispatchRequest {
                endpoint_id: "webhook:test".to_owned(),
                event_type: "test.ping".to_owned(),
                payload: serde_json::json!({ "message": "hello" }),
                approvals: vec!["user-approved".to_owned()],
            },
        );
        assert!(result.is_err());
        assert!(result
            .expect_err("expected plugin gate")
            .contains("Webhooks plugin is disabled"));
        let _ = std::fs::remove_file(path);
    }
}