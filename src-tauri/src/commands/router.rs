use tauri::AppHandle;

use crate::models::RouterRule;
use crate::storage;

#[tauri::command]
pub async fn load_router_rules(app: AppHandle) -> Result<Vec<RouterRule>, String> {
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let connection = storage::open_database(&database_path)?;
        storage::load_router_rules(&connection)
    })
    .await
    .map_err(|error| format!("router load task failed: {error}"))?
}

#[tauri::command]
pub async fn save_router_rules(
    app: AppHandle,
    rules: Vec<RouterRule>,
) -> Result<Vec<RouterRule>, String> {
    let database_path = storage::database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let connection = storage::open_database(&database_path)?;
        storage::save_router_rules(&connection, &rules)?;
        storage::load_router_rules(&connection)
    })
    .await
    .map_err(|error| format!("router save task failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use chrono::Utc;

    use crate::storage;

    fn temp_database_path() -> PathBuf {
        let stamp = Utc::now().timestamp_nanos_opt().unwrap_or(0);
        std::env::temp_dir().join(format!("agentdeck-router-test-{stamp}.sqlite3"))
    }

    #[test]
    fn seeds_default_router_rules_on_migration() {
        let path = temp_database_path();
        let connection = storage::open_database(&path).expect("open database");
        let rules = storage::load_router_rules(&connection).expect("load rules");
        assert!(!rules.is_empty());
        assert!(rules.iter().any(|rule| rule.id == "rule:code-review-grok"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn saves_and_reloads_router_rules() {
        let path = temp_database_path();
        let connection = storage::open_database(&path).expect("open database");
        let mut rules = storage::default_router_rules();
        rules[0].route.model_id = "grok-test".to_owned();
        storage::save_router_rules(&connection, &rules).expect("save rules");
        let loaded = storage::load_router_rules(&connection).expect("load rules");
        assert_eq!(loaded[0].route.model_id, "grok-test");
        let _ = fs::remove_file(path);
    }
}