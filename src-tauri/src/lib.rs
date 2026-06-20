pub mod autonomy;
pub mod chatgpt_review;
mod commands;
mod connector_bridge;
pub mod mcp_http;
mod mcp_input_schemas;
mod mcp_output_schemas;
mod mcp_public_url;
pub mod mcp_server;
mod models;
mod permissions;
mod presence;
mod router;
mod secrets;
mod storage;
mod tool_path;
mod tray;
mod tunnel_control;
mod webhooks;
mod xai_research;

use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, WindowEvent};

const SCAN_UPDATE_INTERVAL_SECS: u64 = 10;

pub fn start_scan_event_bus(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(SCAN_UPDATE_INTERVAL_SECS));
        let scan = commands::scan_environment_for_app(&app);
        let _ = tray::refresh_from_scan(&app, &scan);
        let _ = app.emit("scan-updated", scan);
    });
}

const CHATGPT_REVIEW_MONITOR_INTERVAL_SECS: u64 = 90;

pub fn start_chatgpt_review_monitor(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(CHATGPT_REVIEW_MONITOR_INTERVAL_SECS));
        let Ok(database_path) = storage::resolve_database_path(None) else {
            continue;
        };
        let Ok(health) = chatgpt_review::evaluate_review_health(&database_path) else {
            continue;
        };
        let _ = tray::set_chatgpt_review_tooltip(&app, &health);
        let _ = app.emit("chatgpt-review-updated", health);
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        use tauri_plugin_autostart::MacosLauncher;

        builder = builder
            .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None::<Vec<&str>>))
            .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
                let _ = presence::show_main_window(app);
            }));
    }

    builder
        .setup(|app| {
            tray::setup(app.handle())?;
            if let Some(window) = app.get_webview_window("main") {
                let app_handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        let Ok(settings) = presence::load_settings(&app_handle) else {
                            return;
                        };
                        if presence::should_hide_on_close(&settings) {
                            api.prevent_close();
                            let _ = presence::hide_main_window(&app_handle);
                        }
                    }
                });
            }
            if let Ok(settings) = presence::load_settings(app.handle()) {
                #[cfg(desktop)]
                {
                    use tauri_plugin_autostart::ManagerExt;
                    let manager = app.autolaunch();
                    if settings.launch_at_login {
                        let _ = manager.enable();
                    } else {
                        let _ = manager.disable();
                    }
                }
            }
            presence::apply_startup_presence(app.handle())?;
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
            start_chatgpt_review_monitor(app.handle().clone());
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
            commands::chat::clear_chat_messages,
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
            commands::webhooks::load_webhook_endpoints,
            commands::webhooks::save_webhook_endpoints,
            commands::webhooks::save_webhook_secret,
            commands::webhooks::dispatch_webhook_event,
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
            commands::connectors::chatgpt_review_health,
            commands::agent_permissions::load_agent_permissions,
            commands::agent_permissions::set_agent_permission,
            commands::plugins::load_plugin_inventory,
            commands::plugins::set_plugin_enabled,
            commands::plugins::execute_skill,
            commands::settings::load_app_settings,
            commands::settings::update_app_settings,
            commands::settings::sync_app_presence,
            commands::settings::show_main_window,
            commands::settings::hide_main_window,
            commands::settings::is_main_window_visible,
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
