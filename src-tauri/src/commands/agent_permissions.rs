use tauri::AppHandle;

use crate::models::AgentPermissionMatrix;
use crate::permissions::{self, PERMISSION_ACTIONS};
use crate::storage;

#[tauri::command]
pub async fn load_agent_permissions(app: AppHandle) -> Result<AgentPermissionMatrix, String> {
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let connection = storage::open_database(&database_path)?;
        let permissions = permissions::load_agent_permissions(&connection)?;
        Ok(AgentPermissionMatrix {
            agents: permissions::DEFAULT_AGENT_IDS
                .iter()
                .map(|agent| (*agent).to_owned())
                .collect(),
            actions: PERMISSION_ACTIONS
                .iter()
                .map(|action| (*action).to_owned())
                .collect(),
            permissions,
        })
    })
    .await
    .map_err(|error| format!("permission load task failed: {error}"))?
}

#[tauri::command]
pub async fn set_agent_permission(
    app: AppHandle,
    agent_id: String,
    action: String,
    allow: bool,
) -> Result<AgentPermissionMatrix, String> {
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let connection = storage::open_database(&database_path)?;
        permissions::set_agent_permission(&connection, &agent_id, &action, allow)?;
        let permissions = permissions::load_agent_permissions(&connection)?;
        Ok(AgentPermissionMatrix {
            agents: permissions::DEFAULT_AGENT_IDS
                .iter()
                .map(|agent| (*agent).to_owned())
                .collect(),
            actions: PERMISSION_ACTIONS
                .iter()
                .map(|action| (*action).to_owned())
                .collect(),
            permissions,
        })
    })
    .await
    .map_err(|error| format!("permission update task failed: {error}"))?
}