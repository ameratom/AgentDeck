use rusqlite::{params, Connection};

use crate::models::AgentPermission;
pub const PERMISSION_ACTIONS: [&str; 5] = [
    "read-config",
    "write-config",
    "dispatch-handoff",
    "execute-skill",
    "call-mcp-tool",
];

pub const DEFAULT_AGENT_IDS: [&str; 7] = [
    "agent:agentdeck",
    "agent:codex",
    "agent:claude-code",
    "agent:hermes",
    "agent:openclaw",
    "agent:grok",
    "agent:lm-studio",
];

pub fn load_agent_permissions(connection: &Connection) -> Result<Vec<AgentPermission>, String> {
    seed_default_permissions(connection)?;
    let mut statement = connection
        .prepare("SELECT agent_id, action, allow FROM agent_permissions ORDER BY agent_id, action")
        .map_err(|error| format!("failed to prepare permission query: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok(AgentPermission {
                agent_id: row.get(0)?,
                action: row.get(1)?,
                allow: row.get::<_, i64>(2)? != 0,
            })
        })
        .map_err(|error| format!("failed to load agent permissions: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to decode agent permissions: {error}"))
}

pub fn set_agent_permission(
    connection: &Connection,
    agent_id: &str,
    action: &str,
    allow: bool,
) -> Result<(), String> {
    validate_agent_id(agent_id)?;
    validate_action(action)?;
    connection
        .execute(
            "INSERT INTO agent_permissions (agent_id, action, allow, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(agent_id, action) DO UPDATE SET
                allow = excluded.allow,
                updated_at = excluded.updated_at",
            params![
                agent_id,
                action,
                if allow { 1_i64 } else { 0_i64 },
                chrono::Utc::now().to_rfc3339()
            ],
        )
        .map_err(|error| format!("failed to store agent permission: {error}"))?;
    Ok(())
}

pub fn require_permission(
    connection: &Connection,
    agent_id: &str,
    action: &str,
) -> Result<(), String> {
    validate_agent_id(agent_id)?;
    validate_action(action)?;
    let allowed: i64 = connection
        .query_row(
            "SELECT allow FROM agent_permissions WHERE agent_id = ?1 AND action = ?2",
            params![agent_id, action],
            |row| row.get(0),
        )
        .map_err(|error| format!("failed to query permission for {agent_id}/{action}: {error}"))?;
    if allowed == 0 {
        return Err(format!(
            "permission denied: {agent_id} cannot perform {action}"
        ));
    }
    Ok(())
}

pub fn permission_allowed(
    connection: &Connection,
    agent_id: &str,
    action: &str,
) -> Result<bool, String> {
    validate_agent_id(agent_id)?;
    validate_action(action)?;
    let result = connection.query_row(
        "SELECT allow FROM agent_permissions WHERE agent_id = ?1 AND action = ?2",
        params![agent_id, action],
        |row| row.get::<_, i64>(0),
    );
    match result {
        Ok(value) => Ok(value != 0),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
        Err(error) => Err(format!("failed to query permission: {error}")),
    }
}

// New agent IDs should be seeded via migration or load_agent_permissions, not per-check.
fn seed_default_permissions(connection: &Connection) -> Result<(), String> {
    for agent_id in DEFAULT_AGENT_IDS {
        for action in PERMISSION_ACTIONS {
            let exists: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM agent_permissions WHERE agent_id = ?1 AND action = ?2",
                    params![agent_id, action],
                    |row| row.get(0),
                )
                .map_err(|error| format!("failed to query permission seed state: {error}"))?;
            if exists == 0 {
                set_agent_permission(
                    connection,
                    agent_id,
                    action,
                    default_allowed(agent_id, action),
                )?;
            }
        }
    }
    Ok(())
}

fn default_allowed(agent_id: &str, action: &str) -> bool {
    if agent_id == "agent:agentdeck" {
        return true;
    }
    matches!(action, "read-config" | "dispatch-handoff" | "call-mcp-tool")
}

fn validate_agent_id(agent_id: &str) -> Result<(), String> {
    if agent_id.is_empty() || agent_id.len() > 128 {
        return Err("agent ID must contain between 1 and 128 characters".to_owned());
    }
    Ok(())
}

fn validate_action(action: &str) -> Result<(), String> {
    if PERMISSION_ACTIONS.contains(&action) {
        Ok(())
    } else {
        Err(format!("unsupported permission action: {action}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage;

    #[test]
    fn seeds_default_permissions() {
        let path = std::env::temp_dir().join(format!(
            "agentdeck-permissions-{}.sqlite3",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let connection = storage::open_database(&path).expect("open database");
        let permissions = load_agent_permissions(&connection).expect("load permissions");
        assert!(permissions.iter().any(|entry| {
            entry.agent_id == "agent:codex" && entry.action == "dispatch-handoff" && entry.allow
        }));
        assert!(permissions.iter().any(|entry| {
            entry.agent_id == "agent:codex" && entry.action == "write-config" && !entry.allow
        }));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn seeds_missing_agents_on_existing_database() {
        let path = std::env::temp_dir().join(format!(
            "agentdeck-permissions-missing-{}.sqlite3",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let connection = storage::open_database(&path).expect("open database");
        let _ = load_agent_permissions(&connection).expect("initial seed");
        connection
            .execute(
                "DELETE FROM agent_permissions WHERE agent_id = 'agent:lm-studio'",
                [],
            )
            .expect("delete lm-studio permissions");
        let permissions = load_agent_permissions(&connection).expect("reload permissions");
        assert!(permissions.iter().any(|entry| {
            entry.agent_id == "agent:lm-studio" && entry.action == "dispatch-handoff" && entry.allow
        }));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn require_permission_works_after_migration_without_reseeding() {
        let path = std::env::temp_dir().join(format!(
            "agentdeck-permissions-require-{}.sqlite3",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let connection = storage::open_database(&path).expect("open database");
        require_permission(&connection, "agent:codex", "dispatch-handoff")
            .expect("codex should dispatch after migration seed");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn denies_write_config_by_default() {
        let path = std::env::temp_dir().join(format!(
            "agentdeck-permissions-deny-{}.sqlite3",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let connection = storage::open_database(&path).expect("open database");
        let result = require_permission(&connection, "agent:codex", "write-config");
        assert!(result.is_err());
        let _ = std::fs::remove_file(path);
    }
}