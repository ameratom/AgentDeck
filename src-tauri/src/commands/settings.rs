use tauri::AppHandle;

use crate::models::{
    AppSettings, AppSettingsUpdateRequest, AuditEventsPage, LocalDeleteResult, LocalExportResult,
};
use crate::presence;
use crate::storage;

#[cfg(desktop)]
fn sync_launch_at_login(app: &AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;

    let manager = app.autolaunch();
    if enabled {
        manager
            .enable()
            .map_err(|error| format!("failed to enable launch at login: {error}"))
    } else {
        manager
            .disable()
            .map_err(|error| format!("failed to disable launch at login: {error}"))
    }
}

#[cfg(not(desktop))]
fn sync_launch_at_login(_app: &AppHandle, _enabled: bool) -> Result<(), String> {
    Ok(())
}

fn apply_settings_side_effects(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    sync_launch_at_login(app, settings.launch_at_login)?;
    presence::sync_presence_from_settings(app)
}

#[tauri::command]
pub async fn load_app_settings(app: AppHandle) -> Result<AppSettings, String> {
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || storage::load_app_settings(&database_path))
        .await
        .map_err(|error| format!("settings load task failed: {error}"))?
}

#[tauri::command]
pub async fn update_app_settings(
    app: AppHandle,
    request: AppSettingsUpdateRequest,
) -> Result<AppSettings, String> {
    let database_path = storage::database_path(&app)?;
    let settings = AppSettings {
        redact_sensitive_exports: request.redact_sensitive_exports,
        crash_safe_logging: request.crash_safe_logging,
        grok_subscription_active: request.grok_subscription_active,
        onboarding_complete: request.onboarding_complete,
        router_auto_apply: request.router_auto_apply,
        menu_bar_service_mode: request.menu_bar_service_mode,
        start_hidden: request.start_hidden,
        close_hides_to_menu_bar: request.close_hides_to_menu_bar,
        launch_at_login: request.launch_at_login,
    };
    let saved = tauri::async_runtime::spawn_blocking(move || {
        storage::update_app_settings(&database_path, &settings)
    })
    .await
    .map_err(|error| format!("settings update task failed: {error}"))??;
    apply_settings_side_effects(&app, &saved)?;
    Ok(saved)
}

#[tauri::command]
pub async fn complete_onboarding(app: AppHandle) -> Result<AppSettings, String> {
    let database_path = storage::database_path(&app)?;
    let saved = tauri::async_runtime::spawn_blocking(move || {
        let current = storage::load_app_settings(&database_path)?;
        storage::update_app_settings(
            &database_path,
            &AppSettings {
                onboarding_complete: true,
                ..current
            },
        )
    })
    .await
    .map_err(|error| format!("onboarding completion task failed: {error}"))??;
    if presence::should_start_hidden(&saved) {
        let _ = presence::hide_main_window(&app);
    }
    apply_settings_side_effects(&app, &saved)?;
    Ok(saved)
}

#[tauri::command]
pub async fn sync_app_presence(app: AppHandle) -> Result<(), String> {
    presence::sync_presence_from_settings(&app)
}

#[tauri::command]
pub async fn show_main_window(app: AppHandle) -> Result<(), String> {
    presence::show_main_window(&app)
}

#[tauri::command]
pub async fn hide_main_window(app: AppHandle) -> Result<(), String> {
    presence::hide_main_window(&app)
}

#[tauri::command]
pub async fn is_main_window_visible(app: AppHandle) -> Result<bool, String> {
    Ok(presence::is_main_window_visible(&app))
}

#[tauri::command]
pub async fn export_local_data(app: AppHandle) -> Result<LocalExportResult, String> {
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || storage::export_local_data(&database_path))
        .await
        .map_err(|error| format!("data export task failed: {error}"))?
}

#[tauri::command]
pub async fn delete_local_data(app: AppHandle) -> Result<LocalDeleteResult, String> {
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || storage::delete_local_data(&database_path))
        .await
        .map_err(|error| format!("data deletion task failed: {error}"))?
}

#[tauri::command]
pub async fn load_audit_events(
    app: AppHandle,
    limit: u32,
    offset: u32,
    filter: Option<String>,
) -> Result<AuditEventsPage, String> {
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let connection = storage::open_database(&database_path)?;
        storage::query_audit_events(&connection, limit, offset, filter.as_deref())
    })
    .await
    .map_err(|error| format!("audit load task failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use chrono::Utc;

    use crate::storage;

    fn temp_database_path() -> PathBuf {
        let stamp = Utc::now().timestamp_nanos_opt().unwrap_or(0);
        std::env::temp_dir().join(format!("agentdeck-audit-test-{stamp}.sqlite3"))
    }

    fn seed_audit_events(path: &std::path::Path) -> Result<(), String> {
        let connection = storage::open_database(path)?;
        for index in 0..5 {
            connection
                .execute(
                    "INSERT INTO audit_events
                     (id, action, status, model, conversation_id, duration_ms, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    rusqlite::params![
                        format!("audit:{index}"),
                        if index % 2 == 0 {
                            "handoff.run"
                        } else {
                            "skill.execute"
                        },
                        if index == 4 { "failed" } else { "completed" },
                        if index % 2 == 0 {
                            "grok-4.3"
                        } else {
                            "hermes-local"
                        },
                        format!("thread:{index}"),
                        index * 100,
                        format!("2026-06-10T12:00:{index:02}Z"),
                    ],
                )
                .map_err(|error| format!("failed to seed audit event: {error}"))?;
        }
        Ok(())
    }

    #[test]
    fn paginates_audit_events() {
        let path = temp_database_path();
        seed_audit_events(&path).expect("seed audit events");
        let connection = storage::open_database(&path).expect("open database");

        let first_page = storage::query_audit_events(&connection, 2, 0, None).expect("first page");
        assert_eq!(first_page.total, 5);
        assert_eq!(first_page.events.len(), 2);
        assert_eq!(first_page.offset, 0);

        let second_page =
            storage::query_audit_events(&connection, 2, 2, None).expect("second page");
        assert_eq!(second_page.events.len(), 2);
        assert_eq!(second_page.offset, 2);
        assert_ne!(
            first_page.events[0].id, second_page.events[0].id,
            "pages should not overlap"
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn filters_audit_events_by_action_and_model() {
        let path = temp_database_path();
        seed_audit_events(&path).expect("seed audit events");
        let connection = storage::open_database(&path).expect("open database");

        let filtered =
            storage::query_audit_events(&connection, 10, 0, Some("grok")).expect("filtered page");
        assert_eq!(filtered.total, 3);
        assert!(filtered
            .events
            .iter()
            .all(|event| event.action.contains("handoff") || event.model.contains("grok")));

        let _ = fs::remove_file(path);
    }
}