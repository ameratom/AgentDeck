mod commands;
mod connector_bridge;
mod router;
pub mod mcp_http;
mod mcp_public_url;
pub mod mcp_server;
mod models;
mod permissions;
mod secrets;
mod storage;
mod tray;
mod tunnel_control;

use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

const SCAN_UPDATE_INTERVAL_SECS: u64 = 10;

pub fn start_scan_event_bus(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(SCAN_UPDATE_INTERVAL_SECS));
        let scan = commands::scan_environment_for_app(&app);
        let _ = tray::refresh_from_scan(&app, &scan);
        let _ = app.emit("scan-updated", scan);
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            tray::setup(app.handle())?;
            mcp_http::start_http_server();
            if let Ok(database_path) = storage::database_path(app.handle()) {
                let _ = connector_bridge::sync_grok_mcp_bridge(&database_path);
            }
            let app_handle = app.handle().clone();
            thread::spawn(move || {
                let initial_scan = commands::scan_environment_for_app(&app_handle);
                let _ = tray::refresh_from_scan(&app_handle, &initial_scan);
                let _ = app_handle.emit("scan-updated", initial_scan);
            });
            start_scan_event_bus(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::run_preflight,
            commands::scan_project_environment,
            commands::chat::list_lm_studio_models,
            commands::chat::send_chat_message,
            commands::chat::stream_chat_message,
            commands::chat::cancel_stream_chat,
            commands::chat::load_chat_messages,
            commands::chat::load_chat_preferences,
            commands::chat::save_chat_preferences,
            commands::providers::list_provider_adapters,
            commands::providers::check_provider_adapter,
            commands::providers::save_provider_api_key,
            commands::providers::delete_provider_api_key,
            commands::providers::import_legacy_provider_credentials,
            commands::handoffs::run_handoff,
            commands::handoffs::load_handoff_runs,
            commands::router::load_router_rules,
            commands::router::save_router_rules,
            commands::router::suggest_handoff_route,
            commands::projects::list_projects,
            commands::projects::register_project,
            commands::projects::set_active_project,
            commands::projects::remove_project,
            commands::mcp::scan_mcp_inventory,
            commands::mcp::toggle_mcp_server,
            commands::mcp::load_project_connector_settings,
            commands::mcp::save_project_connector_settings,
            commands::connectors::sync_grok_mcp_bridge,
            commands::connectors::grok_mcp_bridge_status,
            commands::connectors::secure_tunnel_status,
            commands::connectors::start_secure_tunnel,
            commands::connectors::stop_secure_tunnel,
            commands::connectors::open_secure_tunnel_ui,
            commands::agent_permissions::load_agent_permissions,
            commands::agent_permissions::set_agent_permission,
            commands::plugins::load_plugin_inventory,
            commands::plugins::set_plugin_enabled,
            commands::plugins::execute_skill,
            commands::settings::load_app_settings,
            commands::settings::update_app_settings,
            commands::settings::export_local_data,
            commands::settings::delete_local_data,
            commands::settings::load_audit_events,
            commands::settings::complete_onboarding
        ])
        .run(tauri::generate_context!())
        .expect("error while running AgentDeck");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_update_payload_serializes_for_event_bus() {
        let scan = commands::scan_environment();
        let json = serde_json::to_string(&scan).expect("serialize scan payload");
        assert!(json.contains("scannedAt"));
        assert!(json.contains("entities"));
    }

    #[test]
    fn scan_update_interval_is_ten_seconds() {
        assert_eq!(SCAN_UPDATE_INTERVAL_SECS, 10);
    }
}

pub fn run_mcp_server() {
    if let Err(error) = mcp_server::run_stdio() {
        eprintln!("AgentDeck MCP server failed: {error}");
        std::process::exit(1);
    }
}
