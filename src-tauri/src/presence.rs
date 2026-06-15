use tauri::{AppHandle, Manager};

use crate::models::AppSettings;
use crate::storage;
use crate::tray;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppPresence {
    Foreground,
    Background,
}

pub fn dev_forces_foreground() -> bool {
    std::env::var("AGENTDECK_DEV_SHOW_DOCK")
        .ok()
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "yes"))
}

pub fn load_settings(app: &AppHandle) -> Result<AppSettings, String> {
    let database_path = storage::database_path(app)?;
    storage::load_app_settings(&database_path)
}

pub fn is_main_window_visible(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false)
}

pub fn should_hide_on_close(settings: &AppSettings) -> bool {
    settings.menu_bar_service_mode
        && settings.close_hides_to_menu_bar
        && !dev_forces_foreground()
}

pub fn should_start_hidden(settings: &AppSettings) -> bool {
    settings.menu_bar_service_mode
        && settings.start_hidden
        && settings.onboarding_complete
        && !dev_forces_foreground()
}

pub fn set_dock_visible(app: &AppHandle, visible: bool) -> Result<(), String> {
    app.set_dock_visibility(visible)
        .map_err(|error| format!("failed to set dock visibility: {error}"))
}

pub fn apply_presence(app: &AppHandle, presence: AppPresence) -> Result<(), String> {
    match presence {
        AppPresence::Foreground => {
            set_dock_visible(app, true)?;
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        AppPresence::Background => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
            set_dock_visible(app, false)?;
        }
    }
    tray::refresh_menu_state(app);
    Ok(())
}

pub fn show_main_window(app: &AppHandle) -> Result<(), String> {
    apply_presence(app, AppPresence::Foreground)
}

pub fn hide_main_window(app: &AppHandle) -> Result<(), String> {
    let settings = load_settings(app)?;
    if !settings.menu_bar_service_mode {
        return Ok(());
    }
    apply_presence(app, AppPresence::Background)
}

pub fn apply_startup_presence(app: &AppHandle) -> Result<(), String> {
    let settings = load_settings(app)?;
    if dev_forces_foreground() {
        return apply_presence(app, AppPresence::Foreground);
    }
    if should_start_hidden(&settings) {
        apply_presence(app, AppPresence::Background)
    } else {
        apply_presence(app, AppPresence::Foreground)
    }
}

pub fn sync_presence_from_settings(app: &AppHandle) -> Result<(), String> {
    let settings = load_settings(app)?;
    if dev_forces_foreground() || !settings.menu_bar_service_mode {
        return apply_presence(app, AppPresence::Foreground);
    }
    if is_main_window_visible(app) {
        apply_presence(app, AppPresence::Foreground)
    } else {
        apply_presence(app, AppPresence::Background)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_settings() -> AppSettings {
        AppSettings {
            redact_sensitive_exports: true,
            crash_safe_logging: true,
            grok_subscription_active: true,
            onboarding_complete: true,
            router_auto_apply: true,
            menu_bar_service_mode: true,
            start_hidden: true,
            close_hides_to_menu_bar: true,
            launch_at_login: false,
        }
    }

    #[test]
    fn hides_on_close_only_in_service_mode() {
        let mut settings = base_settings();
        assert!(should_hide_on_close(&settings));

        settings.menu_bar_service_mode = false;
        assert!(!should_hide_on_close(&settings));

        settings.menu_bar_service_mode = true;
        settings.close_hides_to_menu_bar = false;
        assert!(!should_hide_on_close(&settings));
    }

    #[test]
    fn start_hidden_requires_onboarding_complete() {
        let mut settings = base_settings();
        assert!(should_start_hidden(&settings));

        settings.onboarding_complete = false;
        assert!(!should_start_hidden(&settings));
    }
}