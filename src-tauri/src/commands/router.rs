use chrono::Utc;
use tauri::AppHandle;

use crate::commands::providers;
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
        if let Some(suggestion) = router::suggest_route(&rules, &request) {
            return Ok(Some(suggestion));
        }
        let xai = providers::xai_readiness();
        if xai.health.available {
            return Ok(Some(HandoffRouteSuggestion {
                rule_id: "router-rule:grok-default".to_owned(),
                rule_name: "Grok default".to_owned(),
                target_provider_id: "xai".to_owned(),
                target_model_id: None,
                reason: "No keyword rule matched; xAI is available.".to_owned(),
            }));
        }
        Ok(None)
    })
    .await
    .map_err(|error| format!("router suggestion task failed: {error}"))?
}