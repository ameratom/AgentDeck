use chrono::Utc;
use tauri::AppHandle;

use crate::models::{
    HandoffRouteRequest, HandoffRouteSuggestion, RouterRuleMatrix, SaveRouterRulesRequest,
};
use crate::router;
use crate::storage;

#[tauri::command]
pub async fn load_router_rules(app: AppHandle) -> Result<RouterRuleMatrix, String> {
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let connection = storage::open_database(&database_path)?;
        let rules = storage::load_router_rules(&connection)?;
        Ok(RouterRuleMatrix {
            loaded_at: Utc::now().to_rfc3339(),
            rules,
        })
    })
    .await
    .map_err(|error| format!("router rule load task failed: {error}"))?
}

#[tauri::command]
pub async fn save_router_rules(
    app: AppHandle,
    request: SaveRouterRulesRequest,
) -> Result<RouterRuleMatrix, String> {
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let connection = storage::open_database(&database_path)?;
        let rules = storage::replace_router_rules(&connection, &request.rules)?;
        Ok(RouterRuleMatrix {
            loaded_at: Utc::now().to_rfc3339(),
            rules,
        })
    })
    .await
    .map_err(|error| format!("router rule save task failed: {error}"))?
}

#[tauri::command]
pub async fn suggest_handoff_route(
    app: AppHandle,
    request: HandoffRouteRequest,
) -> Result<Option<HandoffRouteSuggestion>, String> {
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let connection = storage::open_database(&database_path)?;
        let rules = storage::load_router_rules(&connection)?;
        Ok(router::suggest_route(&rules, &request))
    })
    .await
    .map_err(|error| format!("router suggestion task failed: {error}"))?
}