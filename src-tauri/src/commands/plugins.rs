use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{params, Connection};
use serde::Deserialize;
use tauri::{AppHandle, Emitter};

use crate::commands::providers;
use crate::commands::webhooks;
use crate::models::{
    PluginDefinition, PluginInventory, PluginToggleRequest, SkillDefinition, SkillExecutionRecord,
    SkillExecutionRequest,
};
use crate::permissions;
use crate::storage;

const MAX_DATA_BYTES: u64 = 1_048_576;

#[derive(Debug, Deserialize)]
struct PluginFile {
    plugins: Vec<PluginRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginRecord {
    id: String,
    name: String,
    description: String,
    category: String,
    capabilities: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkillFrontmatter {
    id: String,
    name: String,
    description: String,
    #[serde(default)]
    plugin_ids: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[tauri::command]
pub async fn load_plugin_inventory(app: AppHandle) -> Result<PluginInventory, String> {
    let database_path = database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || load_inventory(&database_path))
        .await
        .map_err(|error| format!("plugin inventory task failed: {error}"))?
}

#[tauri::command]
pub async fn set_plugin_enabled(
    app: AppHandle,
    request: PluginToggleRequest,
) -> Result<PluginInventory, String> {
    validate_id("plugin ID", &request.plugin_id)?;
    let database_path = database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let known_plugins = load_plugin_records()?;
        if !known_plugins
            .iter()
            .any(|plugin| plugin.id == request.plugin_id)
        {
            return Err("plugin was not found".to_owned());
        }

        let connection = open_database(&database_path)?;
        connection
            .execute(
                "INSERT INTO plugin_settings (plugin_id, enabled, updated_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(plugin_id) DO UPDATE SET
                    enabled = excluded.enabled,
                    updated_at = excluded.updated_at",
                params![
                    request.plugin_id,
                    if request.enabled { 1_i64 } else { 0_i64 },
                    Utc::now().to_rfc3339()
                ],
            )
            .map_err(|error| format!("failed to save plugin setting: {error}"))?;
        storage::append_log_event(
            &database_path,
            "plugin.enable",
            serde_json::json!({
                "pluginId": request.plugin_id,
                "enabled": request.enabled,
            }),
        );

        load_inventory(&database_path)
    })
    .await
    .map_err(|error| format!("plugin setting task failed: {error}"))?
}

#[tauri::command]
pub async fn execute_skill(
    app: AppHandle,
    request: SkillExecutionRequest,
) -> Result<SkillExecutionRecord, String> {
    validate_id("skill ID", &request.skill_id)?;
    let database_path = database_path(&app)?;
    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let record = execute_skill_pipeline(&database_path, &request.skill_id)?;
        let _ = app_handle.emit("skill-completed", &record);
        Ok(record)
    })
    .await
    .map_err(|error| format!("skill execution task failed: {error}"))?
}

pub(crate) fn execute_skill_pipeline(
    path: &Path,
    skill_id: &str,
) -> Result<SkillExecutionRecord, String> {
    let connection = open_database(path)?;
    permissions::require_permission(&connection, "agent:agentdeck", "execute-skill")?;

    let inventory = load_inventory(path)?;
    let skill = inventory
        .skills
        .into_iter()
        .find(|skill| skill.id == skill_id)
        .ok_or_else(|| "skill was not found".to_owned())?;
    if !skill.available {
        return Err("skill is unavailable because a required plugin is disabled".to_owned());
    }

    let (provider_id, model_id) = resolve_provider_for_skill(path, &skill);
    let prompt = format!(
        "AgentDeck skill execution\n\nSkill: {}\n\nInstructions:\n{}\n\nReturn a concise result with concrete next steps.",
        skill.name, skill.instructions
    );
    let started_at = Utc::now();
    let dispatch_result = providers::dispatch_provider_handoff(
        &provider_id,
        &model_id,
        &format!("Skill: {}", skill.name),
        &skill.description,
        &skill.source,
        "agentdeck-skill",
        &prompt,
    );

    let created_at = Utc::now();
    let execution_id = format!(
        "skill-run:{:016x}",
        storage::stable_hash(&format!("{skill_id}:{created_at}"))
    );
    let audit_ref = format!(
        "audit:{:016x}",
        storage::stable_hash(&format!("skill.execute:{skill_id}:{created_at}"))
    );

    let (status, output, audit_status) = match dispatch_result {
        Ok((content, _)) => ("completed", content, "success"),
        Err(error) => ("failed", error.clone(), "error"),
    };

    connection
        .execute(
            "INSERT INTO skill_execution_runs
                (id, skill_id, skill_name, status, audit_ref, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                execution_id,
                skill.id,
                skill.name,
                status,
                audit_ref,
                created_at.to_rfc3339()
            ],
        )
        .map_err(|error| format!("failed to store skill execution: {error}"))?;
    connection
        .execute(
            "INSERT INTO audit_events
                (id, action, status, model, conversation_id, duration_ms, created_at)
             VALUES (?1, 'skill.execute', ?2, ?3, ?4, ?5, ?6)",
            params![
                audit_ref,
                audit_status,
                model_id,
                execution_id,
                (created_at - started_at).num_milliseconds(),
                created_at.to_rfc3339()
            ],
        )
        .map_err(|error| format!("failed to store skill audit event: {error}"))?;
    storage::append_log_event(
        path,
        "skill.execute",
        serde_json::json!({
            "skillId": skill.id,
            "skillName": skill.name,
            "status": status,
            "auditRef": audit_ref,
            "output": output,
        }),
    );
    webhooks::emit_webhook_events(
        path,
        "skill.completed",
        serde_json::json!({
            "executionId": execution_id,
            "skillId": skill.id,
            "skillName": skill.name,
            "status": status,
            "auditRef": audit_ref,
        }),
    );

    Ok(SkillExecutionRecord {
        id: execution_id,
        skill_id: skill.id,
        skill_name: skill.name,
        status: status.to_owned(),
        audit_ref,
        created_at: created_at.to_rfc3339(),
        output,
    })
}

fn resolve_provider_for_skill(path: &Path, skill: &SkillDefinition) -> (String, String) {
    for plugin_id in &skill.plugin_ids {
        match plugin_id.as_str() {
            "agentdeck-plugin-xai" => {
                return ("xai".to_owned(), "grok-4-1-fast-reasoning".to_owned());
            }
            "agentdeck-plugin-lmstudio" => {
                let model_id = providers::resolve_lm_studio_model_id(Some(path))
                    .unwrap_or_else(|_| "local-model".to_owned());
                return ("lm-studio".to_owned(), model_id);
            }
            "agentdeck-plugin-codex" => return ("codex".to_owned(), "codex-1".to_owned()),
            "agentdeck-plugin-claude-code" => {
                return ("claude-code".to_owned(), "claude-code".to_owned());
            }
            _ => {}
        }
    }

    let model_id = providers::resolve_lm_studio_model_id(Some(path))
        .unwrap_or_else(|_| "local-model".to_owned());
    ("lm-studio".to_owned(), model_id)
}

fn load_inventory(database_path: &Path) -> Result<PluginInventory, String> {
    let connection = open_database(database_path)?;
    let enabled_settings = load_enabled_settings(&connection)?;
    let mut plugins = load_plugin_records()?
        .into_iter()
        .map(|plugin| PluginDefinition {
            enabled: enabled_settings.get(&plugin.id).copied().unwrap_or(true),
            id: plugin.id,
            name: plugin.name,
            description: plugin.description,
            category: plugin.category,
            capabilities: plugin.capabilities,
        })
        .collect::<Vec<_>>();
    plugins.sort_by(|left, right| left.name.cmp(&right.name));

    let enabled_ids = plugins
        .iter()
        .filter(|plugin| plugin.enabled)
        .map(|plugin| plugin.id.as_str())
        .collect::<BTreeSet<_>>();
    let known_ids = plugins
        .iter()
        .map(|plugin| plugin.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut skills = load_skills()?;
    for skill in &mut skills {
        skill.available = skill.plugin_ids.iter().all(|plugin_id| {
            known_ids.contains(plugin_id.as_str()) && enabled_ids.contains(plugin_id.as_str())
        });
    }
    skills.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(PluginInventory {
        loaded_at: Utc::now().to_rfc3339(),
        plugins,
        skills,
    })
}

fn load_plugin_records() -> Result<Vec<PluginRecord>, String> {
    let path = data_root().join("plugins.yaml");
    let contents = read_bounded(&path)?;
    let file: PluginFile = serde_yaml::from_str(&contents)
        .map_err(|error| format!("plugin registry parse failed: {error}"))?;
    validate_unique_ids(
        file.plugins.iter().map(|plugin| plugin.id.as_str()),
        "plugin",
    )?;
    Ok(file.plugins)
}

fn load_skills() -> Result<Vec<SkillDefinition>, String> {
    let directory = data_root().join("skills");
    let entries = fs::read_dir(&directory)
        .map_err(|error| format!("failed to read skill directory: {error}"))?;
    let mut skills = Vec::new();

    for entry in entries {
        let path = entry
            .map_err(|error| format!("failed to read skill entry: {error}"))?
            .path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
            continue;
        }
        let contents = read_bounded(&path)?;
        let (frontmatter, instructions) = parse_skill_document(&contents)?;
        validate_id("skill ID", &frontmatter.id)?;
        skills.push(SkillDefinition {
            id: frontmatter.id,
            name: frontmatter.name,
            description: frontmatter.description,
            plugin_ids: frontmatter.plugin_ids,
            tags: frontmatter.tags,
            instructions,
            source: path.to_string_lossy().into_owned(),
            available: false,
        });
    }

    validate_unique_ids(skills.iter().map(|skill| skill.id.as_str()), "skill")?;
    Ok(skills)
}

fn parse_skill_document(contents: &str) -> Result<(SkillFrontmatter, String), String> {
    let normalized = contents.replace("\r\n", "\n");
    let Some(rest) = normalized.strip_prefix("---\n") else {
        return Err("skill file is missing YAML frontmatter".to_owned());
    };
    let Some((yaml, body)) = rest.split_once("\n---\n") else {
        return Err("skill file has unterminated YAML frontmatter".to_owned());
    };
    let frontmatter: SkillFrontmatter = serde_yaml::from_str(yaml)
        .map_err(|error| format!("skill frontmatter parse failed: {error}"))?;
    let instructions = body.trim().to_owned();
    if instructions.is_empty() {
        return Err(format!("skill {} has no instructions", frontmatter.id));
    }
    Ok((frontmatter, instructions))
}

fn load_enabled_settings(connection: &Connection) -> Result<BTreeMap<String, bool>, String> {
    let mut statement = connection
        .prepare("SELECT plugin_id, enabled FROM plugin_settings")
        .map_err(|error| format!("failed to prepare plugin setting query: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? != 0))
        })
        .map_err(|error| format!("failed to load plugin settings: {error}"))?;
    rows.collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(|error| format!("failed to decode plugin settings: {error}"))
}

fn open_database(path: &Path) -> Result<Connection, String> {
    storage::open_database(path)
}

fn database_path(app: &AppHandle) -> Result<PathBuf, String> {
    storage::database_path(app)
}

fn data_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|path| path.join("data"))
        .unwrap_or_else(|| PathBuf::from("data"))
}

fn read_bounded(path: &Path) -> Result<String, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
    if metadata.len() > MAX_DATA_BYTES {
        return Err(format!(
            "{} exceeds the data file size limit",
            path.display()
        ));
    }
    fs::read_to_string(path).map_err(|error| format!("failed to read {}: {error}", path.display()))
}

fn validate_unique_ids<'a>(ids: impl Iterator<Item = &'a str>, kind: &str) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for id in ids {
        validate_id(&format!("{kind} ID"), id)?;
        if !seen.insert(id) {
            return Err(format!("duplicate {kind} ID: {id}"));
        }
    }
    Ok(())
}

fn validate_id(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(format!("{label} contains unsupported characters"));
    }
    Ok(())
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_declared_plugins_and_skills() {
        let plugins = load_plugin_records().unwrap();
        let skills = load_skills().unwrap();
        assert_eq!(plugins.len(), 11);
        assert_eq!(skills.len(), 9);
    }

    #[test]
    fn parses_markdown_skill_frontmatter() {
        let (frontmatter, instructions) = parse_skill_document(
            "---\nid: test-skill\nname: Test\ndescription: Test skill\npluginIds: []\ntags: [test]\n---\n\nRun a test.",
        )
        .unwrap();
        assert_eq!(frontmatter.id, "test-skill");
        assert_eq!(instructions, "Run a test.");
    }

    #[test]
    fn rejects_invalid_identifiers() {
        assert!(validate_id("plugin ID", "plugin/unsafe").is_err());
    }
}
