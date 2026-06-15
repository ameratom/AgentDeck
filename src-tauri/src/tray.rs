use std::path::Path;
use std::sync::Mutex;

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::chatgpt_review::ChatgptReviewHealth;
use crate::commands;
use crate::commands::handoffs;
use crate::models::{EnvironmentScan, HandoffRun};
use crate::storage;

const MENU_QUICK_HANDOFF: &str = "quick_handoff";
const MENU_RUN_PREFIX: &str = "recent_run_";
const MENU_QUIT: &str = "quit";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayHealth {
    Green,
    Yellow,
    Red,
}

struct TrayState {
    tray: TrayIcon<Wry>,
    run_items: [MenuItem<Wry>; 3],
    agent_health: TrayHealth,
    review_ready: Option<bool>,
}

static TRAY_STATE: Mutex<Option<TrayState>> = Mutex::new(None);

pub fn setup(app: &AppHandle) -> Result<(), String> {
    let quick_handoff =
        MenuItem::with_id(app, MENU_QUICK_HANDOFF, "Quick Handoff", true, None::<&str>)
            .map_err(|error| format!("failed to create tray menu item: {error}"))?;
    let run_items = [
        MenuItem::with_id(
            app,
            format!("{MENU_RUN_PREFIX}0"),
            "No recent runs",
            false,
            None::<&str>,
        )
        .map_err(|error| format!("failed to create tray menu item: {error}"))?,
        MenuItem::with_id(
            app,
            format!("{MENU_RUN_PREFIX}1"),
            "No recent runs",
            false,
            None::<&str>,
        )
        .map_err(|error| format!("failed to create tray menu item: {error}"))?,
        MenuItem::with_id(
            app,
            format!("{MENU_RUN_PREFIX}2"),
            "No recent runs",
            false,
            None::<&str>,
        )
        .map_err(|error| format!("failed to create tray menu item: {error}"))?,
    ];
    let separator = PredefinedMenuItem::separator(app)
        .map_err(|error| format!("failed to create separator: {error}"))?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "Quit", true, None::<&str>)
        .map_err(|error| format!("failed to create tray menu item: {error}"))?;

    let menu = Menu::with_items(
        app,
        &[
            &quick_handoff,
            &separator,
            &run_items[0],
            &run_items[1],
            &run_items[2],
            &PredefinedMenuItem::separator(app)
                .map_err(|error| format!("failed to create separator: {error}"))?,
            &quit,
        ],
    )
    .map_err(|error| format!("failed to create tray menu: {error}"))?;

    let icon = tray_icon(TrayHealth::Yellow)?;
    let tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("AgentDeck — starting up")
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            let id = event.id.as_ref();
            if id == MENU_QUIT {
                app.exit(0);
                return;
            }
            if id == MENU_QUICK_HANDOFF {
                focus_main_window(app);
                let _ = app.emit("navigate-view", "Handoffs");
                return;
            }
            if let Some(index) = id.strip_prefix(MENU_RUN_PREFIX) {
                focus_main_window(app);
                let _ = app.emit(
                    "navigate-view",
                    serde_json::json!({
                        "view": "Handoffs",
                        "runIndex": index,
                    }),
                );
            }
        })
        .build(app)
        .map_err(|error| format!("failed to build tray icon: {error}"))?;

    {
        let mut guard = TRAY_STATE
            .lock()
            .map_err(|_| "tray state lock poisoned".to_owned())?;
        *guard = Some(TrayState {
            tray,
            run_items,
            agent_health: TrayHealth::Yellow,
            review_ready: None,
        });
    }

    refresh_from_scan(app, &commands::scan_environment_for_app(app))?;
    Ok(())
}

pub fn set_chatgpt_review_tooltip(
    app: &AppHandle,
    health: &ChatgptReviewHealth,
) -> Result<(), String> {
    let mut guard = TRAY_STATE
        .lock()
        .map_err(|_| "tray state lock poisoned".to_owned())?;
    let Some(state) = guard.as_mut() else {
        return Ok(());
    };
    state.review_ready = Some(health.ready_for_reviewers);
    state
        .tray
        .set_tooltip(Some(combined_tray_tooltip(
            state.agent_health,
            state.review_ready,
        )))
        .map_err(|error| format!("failed to update tray tooltip: {error}"))?;
    let _ = app;
    Ok(())
}

pub fn refresh_from_scan(app: &AppHandle, scan: &EnvironmentScan) -> Result<(), String> {
    let health = agent_health(scan);
    let runs = recent_runs(app)?;
    update_tray(health, &runs)
}

fn update_tray(health: TrayHealth, runs: &[HandoffRun]) -> Result<(), String> {
    let mut guard = TRAY_STATE
        .lock()
        .map_err(|_| "tray state lock poisoned".to_owned())?;
    let Some(state) = guard.as_mut() else {
        return Ok(());
    };

    state.agent_health = health;
    state
        .tray
        .set_icon(Some(tray_icon(health)?))
        .map_err(|error| format!("failed to update tray icon: {error}"))?;
    state
        .tray
        .set_tooltip(Some(combined_tray_tooltip(health, state.review_ready)))
        .map_err(|error| format!("failed to update tray tooltip: {error}"))?;

    for (index, item) in state.run_items.iter().enumerate() {
        if let Some(run) = runs.get(index) {
            let label = format!("{} — {}", run.title, run.status);
            item.set_text(label)
                .map_err(|error| format!("failed to update tray menu label: {error}"))?;
            item.set_enabled(true)
                .map_err(|error| format!("failed to enable tray menu item: {error}"))?;
        } else {
            item.set_text("No recent runs")
                .map_err(|error| format!("failed to update tray menu label: {error}"))?;
            item.set_enabled(false)
                .map_err(|error| format!("failed to disable tray menu item: {error}"))?;
        }
    }

    Ok(())
}

fn recent_runs(app: &AppHandle) -> Result<Vec<HandoffRun>, String> {
    let database_path = storage::database_path(app)?;
    handoffs::load_recent_runs(&database_path, 3)
}

pub fn focus_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn agent_health(scan: &EnvironmentScan) -> TrayHealth {
    let agents: Vec<_> = scan
        .entities
        .iter()
        .filter(|entity| entity.entity_type == "agent")
        .collect();
    if agents.is_empty() {
        return TrayHealth::Red;
    }

    let healthy = agents
        .iter()
        .filter(|agent| matches!(agent.status.as_str(), "running" | "available"))
        .count();

    if healthy == agents.len() {
        TrayHealth::Green
    } else if healthy > 0 {
        TrayHealth::Yellow
    } else {
        TrayHealth::Red
    }
}

fn tray_tooltip(health: TrayHealth) -> &'static str {
    match health {
        TrayHealth::Green => "AgentDeck — all agents healthy",
        TrayHealth::Yellow => "AgentDeck — some agents unavailable",
        TrayHealth::Red => "AgentDeck — no agents available",
    }
}

fn combined_tray_tooltip(health: TrayHealth, review_ready: Option<bool>) -> String {
    let base = tray_tooltip(health);
    match review_ready {
        Some(true) => format!("{base} | ChatGPT review: ready for reviewers"),
        Some(false) => format!("{base} | ChatGPT review: action needed"),
        None => base.to_owned(),
    }
}

fn tray_icon(health: TrayHealth) -> Result<Image<'static>, String> {
    let file_name = match health {
        TrayHealth::Green => "tray-green.png",
        TrayHealth::Yellow => "tray-yellow.png",
        TrayHealth::Red => "tray-red.png",
    };
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("icons")
        .join(file_name);
    let bytes = std::fs::read(&path)
        .map_err(|error| format!("failed to read tray icon {}: {error}", path.display()))?;
    Image::from_bytes(&bytes).map_err(|error| format!("failed to decode tray icon: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DiscoveredEntity;
    use std::collections::BTreeMap;

    fn agent(id: &str, status: &str) -> DiscoveredEntity {
        DiscoveredEntity {
            id: id.to_owned(),
            entity_type: "agent".to_owned(),
            name: id.to_owned(),
            status: status.to_owned(),
            source: "test".to_owned(),
            metadata: BTreeMap::new(),
        }
    }

    #[test]
    fn combined_tooltip_mentions_chatgpt_review_state() {
        let ready = combined_tray_tooltip(TrayHealth::Green, Some(true));
        assert!(ready.contains("ready for reviewers"));
        let action = combined_tray_tooltip(TrayHealth::Yellow, Some(false));
        assert!(action.contains("action needed"));
    }

    #[test]
    fn classifies_agent_health_for_tray() {
        let all_healthy = EnvironmentScan {
            scanned_at: "now".to_owned(),
            project: None,
            tools: vec![],
            providers: vec![],
            processes: vec![],
            configs: vec![],
            entities: vec![
                agent("agent:codex", "running"),
                agent("agent:grok", "available"),
            ],
        };
        assert_eq!(agent_health(&all_healthy), TrayHealth::Green);

        let partial = EnvironmentScan {
            entities: vec![
                agent("agent:codex", "running"),
                agent("agent:grok", "unavailable"),
            ],
            ..all_healthy.clone()
        };
        assert_eq!(agent_health(&partial), TrayHealth::Yellow);

        let none = EnvironmentScan {
            entities: vec![agent("agent:grok", "unavailable")],
            ..all_healthy
        };
        assert_eq!(agent_health(&none), TrayHealth::Red);
    }
}
