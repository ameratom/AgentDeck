use tauri::AppHandle;

use crate::connector_bridge::{self, GrokMcpBridgeStatus};
use crate::storage;
use crate::tunnel_control::{self, TunnelStatus};

#[tauri::command]
pub async fn sync_grok_mcp_bridge(app: AppHandle) -> Result<GrokMcpBridgeStatus, String> {
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        connector_bridge::sync_grok_mcp_bridge(&database_path)
    })
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

#[tauri::command]
pub async fn secure_tunnel_status(app: AppHandle) -> Result<TunnelStatus, String> {
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || tunnel_control::status(&database_path))
        .await
        .map_err(|error| format!("secure tunnel status task failed: {error}"))?
}

#[tauri::command]
pub async fn start_secure_tunnel(app: AppHandle) -> Result<TunnelStatus, String> {
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || tunnel_control::start(&database_path))
        .await
        .map_err(|error| format!("secure tunnel start task failed: {error}"))?
}

#[tauri::command]
pub async fn stop_secure_tunnel(app: AppHandle) -> Result<TunnelStatus, String> {
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || tunnel_control::stop(&database_path))
        .await
        .map_err(|error| format!("secure tunnel stop task failed: {error}"))?
}

#[tauri::command]
pub async fn open_secure_tunnel_ui(app: AppHandle) -> Result<TunnelStatus, String> {
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || tunnel_control::open_operator_ui(&database_path))
        .await
        .map_err(|error| format!("secure tunnel operator UI task failed: {error}"))?
}
