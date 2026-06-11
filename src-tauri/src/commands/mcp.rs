use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde_json::{Map, Value};
use tauri::AppHandle;

use crate::models::{McpConfigSource, McpInventory, McpServerDefinition, McpToggleResult};
use crate::permissions;
use crate::storage;

const MAX_CONFIG_BYTES: u64 = 1_048_576;
const MAX_SERVERS_PER_SOURCE: usize = 100;
const MAX_ARGS: usize = 40;

#[derive(Debug)]
struct SourceCandidate {
    client: &'static str,
    path: PathBuf,
}

#[tauri::command]
pub async fn scan_mcp_inventory() -> Result<McpInventory, String> {
    tauri::async_runtime::spawn_blocking(scan_inventory)
        .await
        .map_err(|error| format!("MCP inventory task failed: {error}"))?
}

#[tauri::command]
pub async fn toggle_mcp_server(
    app: AppHandle,
    server_id: String,
    enabled: bool,
    agent_id: Option<String>,
) -> Result<McpToggleResult, String> {
    let database_path = storage::database_path(&app)?;
    let caller = agent_id.unwrap_or_else(|| "agent:agentdeck".to_owned());
    tauri::async_runtime::spawn_blocking(move || {
        let connection = storage::open_database(&database_path)?;
        permissions::require_permission(&connection, &caller, "write-config")?;
        toggle_server_config(&server_id, enabled)
    })
    .await
    .map_err(|error| format!("MCP toggle task failed: {error}"))?
}

pub(crate) fn toggle_server_config(server_id: &str, enabled: bool) -> Result<McpToggleResult, String> {
    let inventory = scan_inventory()?;
    let server = inventory
        .servers
        .into_iter()
        .find(|server| server.id == server_id)
        .ok_or_else(|| format!("MCP server not found: {server_id}"))?;
    let config_path = PathBuf::from(&server.source);
    if config_path.extension().and_then(|ext| ext.to_str()) != Some("json") {
        return Err("only JSON MCP config files can be toggled safely".to_owned());
    }

    let original = fs::read_to_string(&config_path)
        .map_err(|error| format!("failed to read config file: {error}"))?;
    let _: Value = serde_json::from_str(&original)
        .map_err(|error| format!("config file is not valid JSON: {error}"))?;

    let backup_path = PathBuf::from(format!("{}.backup", config_path.display()));
    fs::write(&backup_path, &original)
        .map_err(|error| format!("failed to write backup file: {error}"))?;

    let mut parsed: Value = serde_json::from_str(&original)
        .map_err(|error| format!("failed to parse config JSON: {error}"))?;
    if !set_server_enabled(&mut parsed, &server.name, enabled)? {
        let _ = fs::copy(&backup_path, &config_path);
        return Err(format!("server {} not found in config", server.name));
    }

    let updated = serde_json::to_string_pretty(&parsed)
        .map_err(|error| format!("failed to encode updated config: {error}"))?;
    fs::write(&config_path, &updated)
        .map_err(|error| format!("failed to write config file: {error}"))?;

    if let Err(error) = serde_json::from_str::<Value>(&updated) {
        fs::copy(&backup_path, &config_path)
            .map_err(|restore_error| format!("failed to restore config from backup: {restore_error}"))?;
        return Err(format!(
            "updated config failed validation; restored from backup: {error}"
        ));
    }

    Ok(McpToggleResult {
        server_id: server_id.to_owned(),
        server_name: server.name,
        enabled,
        config_path: config_path.to_string_lossy().into_owned(),
        backup_path: backup_path.to_string_lossy().into_owned(),
    })
}

fn set_server_enabled(value: &mut Value, server_name: &str, enabled: bool) -> Result<bool, String> {
    match value {
        Value::Object(object) => {
            let mut found = false;
            for (key, child) in object.iter_mut() {
                if matches!(key.as_str(), "mcpServers" | "mcp_servers") {
                    if let Some(map) = child.as_object_mut() {
                        if let Some(server) = map.get_mut(server_name) {
                            if let Some(definition) = server.as_object_mut() {
                                if enabled {
                                    definition.remove("disabled");
                                } else {
                                    definition.insert("disabled".to_owned(), Value::Bool(true));
                                }
                                found = true;
                            }
                        }
                    }
                }
                if set_server_enabled(child, server_name, enabled)? {
                    found = true;
                }
            }
            Ok(found)
        }
        Value::Array(items) => {
            let mut found = false;
            for item in items.iter_mut() {
                if set_server_enabled(item, server_name, enabled)? {
                    found = true;
                }
            }
            Ok(found)
        }
        _ => Ok(false),
    }
}

pub(crate) fn scan_inventory() -> Result<McpInventory, String> {
    let mut sources = Vec::new();
    let mut servers = Vec::new();

    for candidate in source_candidates() {
        let (source, mut source_servers) = inspect_source(&candidate);
        sources.push(source);
        servers.append(&mut source_servers);
    }

    sources.sort_by(|left, right| left.path.cmp(&right.path));
    servers.sort_by(|left, right| {
        left.client
            .cmp(&right.client)
            .then(left.name.cmp(&right.name))
            .then(left.source.cmp(&right.source))
    });

    Ok(McpInventory {
        scanned_at: Utc::now().to_rfc3339(),
        sources,
        servers,
    })
}

fn source_candidates() -> Vec<SourceCandidate> {
    let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| env::current_dir().unwrap_or_default());

    vec![
        candidate("Codex", home.join(".codex/config.toml")),
        candidate("Claude Code", home.join(".claude.json")),
        candidate("Claude Code", home.join(".claude/settings.json")),
        candidate("Hermes", home.join(".hermes/config.yaml")),
        candidate("Hermes", home.join(".hermes/config.json")),
        candidate("OpenClaw", home.join(".openclaw/openclaw.json")),
        candidate("OpenClaw", home.join(".openclaw/config.json")),
        candidate("LM Studio", home.join(".lmstudio/mcp.json")),
        candidate("Gemini", home.join(".gemini/config/mcp_config.json")),
        candidate("Project", project.join(".mcp.json")),
        candidate("Project Codex", project.join(".codex/config.toml")),
        candidate("Project Claude", project.join(".claude/settings.json")),
    ]
}

fn candidate(client: &'static str, path: PathBuf) -> SourceCandidate {
    SourceCandidate { client, path }
}

fn inspect_source(candidate: &SourceCandidate) -> (McpConfigSource, Vec<McpServerDefinition>) {
    let path_text = candidate.path.to_string_lossy().into_owned();
    let id = format!("mcp-source:{:016x}", storage::stable_hash(&path_text));
    let mut source = McpConfigSource {
        id,
        client: candidate.client.to_owned(),
        path: path_text.clone(),
        exists: candidate.path.is_file(),
        parsed: false,
        server_count: 0,
        error: None,
    };

    if !source.exists {
        return (source, Vec::new());
    }

    let contents = match read_bounded(&candidate.path) {
        Ok(contents) => contents,
        Err(error) => {
            source.error = Some(error);
            return (source, Vec::new());
        }
    };
    let value = match parse_config(&candidate.path, &contents) {
        Ok(value) => value,
        Err(error) => {
            source.error = Some(error);
            return (source, Vec::new());
        }
    };

    let mut definitions = Vec::new();
    collect_server_maps(&value, &mut definitions);
    let mut servers = Vec::new();
    for (name, definition) in definitions.into_iter().take(MAX_SERVERS_PER_SOURCE) {
        if let Some(server) = normalize_server(candidate.client, &path_text, &name, definition) {
            servers.push(server);
        }
    }

    source.parsed = true;
    source.server_count = servers.len();
    (source, servers)
}

fn read_bounded(path: &Path) -> Result<String, String> {
    let metadata =
        fs::metadata(path).map_err(|error| safe_error(&format!("metadata failed: {error}")))?;
    if metadata.len() > MAX_CONFIG_BYTES {
        return Err(format!(
            "skipped: file is larger than {MAX_CONFIG_BYTES} bytes"
        ));
    }
    fs::read_to_string(path).map_err(|error| safe_error(&format!("read failed: {error}")))
}

fn parse_config(path: &Path, contents: &str) -> Result<Value, String> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("json") => serde_json::from_str(contents)
            .map_err(|error| safe_error(&format!("JSON parse failed: {error}"))),
        Some("toml") => {
            let value: toml::Value = toml::from_str(contents)
                .map_err(|error| safe_error(&format!("TOML parse failed: {error}")))?;
            serde_json::to_value(value)
                .map_err(|error| safe_error(&format!("TOML conversion failed: {error}")))
        }
        Some("yaml" | "yml") => serde_yaml::from_str(contents)
            .map_err(|error| safe_error(&format!("YAML parse failed: {error}"))),
        _ => Err("unsupported MCP config format".to_owned()),
    }
}

fn collect_server_maps<'a>(value: &'a Value, output: &mut Vec<(String, &'a Map<String, Value>)>) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if matches!(key.as_str(), "mcpServers" | "mcp_servers") {
                    collect_named_servers(child, output);
                } else if key == "mcp" && looks_like_server_map(child) {
                    collect_named_servers(child, output);
                }
                collect_server_maps(child, output);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_server_maps(item, output);
            }
        }
        _ => {}
    }
}

fn collect_named_servers<'a>(value: &'a Value, output: &mut Vec<(String, &'a Map<String, Value>)>) {
    let Some(object) = value.as_object() else {
        return;
    };
    for (name, definition) in object {
        if let Some(definition) = definition.as_object() {
            if looks_like_server_definition(definition) {
                output.push((name.clone(), definition));
            }
        }
    }
}

fn looks_like_server_map(value: &Value) -> bool {
    value.as_object().is_some_and(|object| {
        object
            .values()
            .filter_map(Value::as_object)
            .any(looks_like_server_definition)
    })
}

fn looks_like_server_definition(definition: &Map<String, Value>) -> bool {
    ["command", "url", "args", "transport", "type"]
        .iter()
        .any(|key| definition.contains_key(*key))
}

fn normalize_server(
    client: &str,
    source: &str,
    name: &str,
    definition: &Map<String, Value>,
) -> Option<McpServerDefinition> {
    let command = string_field(definition, "command").map(sanitize_text);
    let url = string_field(definition, "url")
        .or_else(|| string_field(definition, "endpoint"))
        .map(sanitize_url);
    if command.is_none() && url.is_none() {
        return None;
    }

    let args = sanitized_args(definition.get("args"));
    let cwd = string_field(definition, "cwd").map(sanitize_text);
    let env_keys = environment_keys(definition.get("env"));
    let declared_tools = string_list(definition.get("tools"), 40);
    let enabled = definition
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            !definition
                .get("disabled")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        });
    let transport = string_field(definition, "transport")
        .or_else(|| string_field(definition, "type"))
        .map(sanitize_text)
        .unwrap_or_else(|| {
            if url.is_some() {
                "http".to_owned()
            } else {
                "stdio".to_owned()
            }
        });
    let command_available = command.as_deref().map(command_exists);
    let (risk_level, risk_reasons) = assess_risk(
        command.as_deref(),
        &args,
        url.as_deref(),
        &env_keys,
        command_available,
    );
    let identity = format!("{client}:{source}:{name}");

    Some(McpServerDefinition {
        id: format!("mcp-server:{:016x}", storage::stable_hash(&identity)),
        name: sanitize_text(name),
        client: client.to_owned(),
        transport,
        command,
        args,
        cwd,
        url,
        env_keys,
        source: source.to_owned(),
        enabled,
        command_available,
        declared_tools,
        risk_level,
        risk_reasons,
    })
}

fn string_field<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object.get(key).and_then(Value::as_str)
}

fn sanitized_args(value: Option<&Value>) -> Vec<String> {
    let mut redact_next = false;
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .take(MAX_ARGS)
        .map(|argument| {
            if redact_next {
                redact_next = false;
                return "[redacted]".to_owned();
            }
            let lowered = argument.to_lowercase();
            if is_secret_flag(&lowered) {
                redact_next = true;
                return sanitize_text(argument);
            }
            if let Some((key, _)) = argument.split_once('=') {
                if is_secret_flag(&key.to_lowercase()) {
                    return format!("{}=[redacted]", sanitize_text(key));
                }
            }
            sanitize_text(argument)
        })
        .collect()
}

fn is_secret_flag(value: &str) -> bool {
    [
        "token", "secret", "password", "api-key", "api_key", "apikey",
    ]
    .iter()
    .any(|marker| value.contains(marker))
}

fn environment_keys(value: Option<&Value>) -> Vec<String> {
    let mut keys = BTreeSet::new();
    if let Some(object) = value.and_then(Value::as_object) {
        for key in object.keys().take(60) {
            keys.insert(sanitize_text(key));
        }
    }
    keys.into_iter().collect()
}

fn string_list(value: Option<&Value>, limit: usize) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item.as_str().map(sanitize_text).or_else(|| {
                item.as_object()
                    .and_then(|object| object.get("name"))
                    .and_then(Value::as_str)
                    .map(sanitize_text)
            })
        })
        .take(limit)
        .collect()
}

fn command_exists(command: &str) -> bool {
    let path = Path::new(command);
    if path.components().count() > 1 {
        return path.is_file();
    }
    env::var_os("PATH").is_some_and(|paths| {
        env::split_paths(&paths).any(|directory| directory.join(command).is_file())
    })
}

fn assess_risk(
    command: Option<&str>,
    args: &[String],
    url: Option<&str>,
    env_keys: &[String],
    command_available: Option<bool>,
) -> (String, Vec<String>) {
    let mut level = 1_u8;
    let mut reasons = Vec::new();

    if url.is_some() {
        level = level.max(2);
        reasons.push("Uses a remote network endpoint.".to_owned());
    }
    if !env_keys.is_empty() {
        level = level.max(2);
        reasons.push("Receives environment variables; values remain redacted.".to_owned());
    }
    if command_available == Some(false) {
        level = level.max(2);
        reasons.push("Configured command is not currently available.".to_owned());
    }
    if let Some(command) = command {
        let executable = Path::new(command)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(command)
            .to_lowercase();
        if matches!(
            executable.as_str(),
            "sh" | "bash" | "zsh" | "fish" | "osascript" | "powershell"
        ) || args.iter().any(|argument| argument == "-c")
        {
            level = 3;
            reasons.push("Invokes a shell or inline command.".to_owned());
        }
        if executable == "npx" && args.iter().any(|argument| argument == "-y") {
            level = 3;
            reasons.push("May download and execute an npm package.".to_owned());
        }
    }
    if reasons.is_empty() {
        reasons.push("Local stdio definition with no elevated indicators.".to_owned());
    }

    let label = match level {
        3 => "high",
        2 => "medium",
        _ => "low",
    };
    (label.to_owned(), reasons)
}

fn sanitize_url(value: &str) -> String {
    let Ok(mut url) = reqwest::Url::parse(value) else {
        return sanitize_text(value);
    };
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_query(None);
    url.set_fragment(None);
    sanitize_text(url.as_str())
}

fn sanitize_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(300)
        .collect()
}

fn safe_error(value: &str) -> String {
    sanitize_text(value.lines().next().unwrap_or("MCP config error"))
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_nested_claude_server_maps() {
        let value: Value = serde_json::from_str(
            r#"{"projects":{"/tmp/project":{"mcpServers":{"demo":{"command":"demo"}}}}}"#,
        )
        .unwrap();
        let mut definitions = Vec::new();
        collect_server_maps(&value, &mut definitions);
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].0, "demo");
    }

    #[test]
    fn redacts_secret_arguments_and_environment_values() {
        let value: Value = serde_json::from_str(
            r#"{"command":"tool","args":["--api-key","secret-value"],"env":{"TOKEN":"secret"}}"#,
        )
        .unwrap();
        let definition = value.as_object().unwrap();
        let server = normalize_server("Test", "/tmp/test.json", "demo", definition).unwrap();
        assert_eq!(server.args, vec!["--api-key", "[redacted]"]);
        assert_eq!(server.env_keys, vec!["TOKEN"]);
    }

    #[test]
    fn labels_shell_and_npx_auto_install_as_high_risk() {
        let (shell, _) = assess_risk(Some("bash"), &["-c".to_owned()], None, &[], Some(true));
        let (npx, _) = assess_risk(
            Some("npx"),
            &["-y".to_owned(), "package".to_owned()],
            None,
            &[],
            Some(true),
        );
        assert_eq!(shell, "high");
        assert_eq!(npx, "high");
    }

    #[test]
    fn removes_query_secrets_from_urls() {
        assert_eq!(
            sanitize_url("https://example.com/mcp?token=secret"),
            "https://example.com/mcp"
        );
    }

    #[test]
    fn toggles_disabled_flag_in_nested_json_config() {
        let mut value: Value = serde_json::from_str(
            r#"{"projects":{"/tmp":{"mcpServers":{"demo":{"command":"demo","disabled":false}}}}}"#,
        )
        .unwrap();
        assert!(set_server_enabled(&mut value, "demo", false).unwrap());
        let disabled = value["projects"]["/tmp"]["mcpServers"]["demo"]["disabled"]
            .as_bool()
            .unwrap();
        assert!(disabled);
        assert!(set_server_enabled(&mut value, "demo", true).unwrap());
        assert!(value["projects"]["/tmp"]["mcpServers"]["demo"]
            .as_object()
            .unwrap()
            .get("disabled")
            .is_none());
    }
}
