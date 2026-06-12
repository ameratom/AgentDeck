use tauri::AppHandle;

use crate::connector_bridge::{self, GrokMcpBridgeStatus};
use crate::storage;

#[tauri::command]
pub async fn sync_grok_mcp_bridge(app: AppHandle) -> Result<GrokMcpBridgeStatus, String> {
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || connector_bridge::sync_grok_mcp_bridge(&database_path))
        .await
        .map_err(|error| format!("grok MCP bridge sync failed: {error}"))?
}

#[tauri::command]
pub async fn grok_mcp_bridge_status(app: AppHandle) -> Result<GrokMcpBridgeStatus, String> {
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || connector_bridge::bridge_status_at(&database_path))
        .await
        .map_err(|error| format!("grok MCP bridge status failed: {error}"))?
}