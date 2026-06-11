pub mod agent_permissions;
pub mod chat;
pub mod chat_providers;
pub mod handoffs;
pub mod mcp;
pub mod plugins;
pub mod providers;

pub mod settings;

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use chrono::Utc;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::models::{
    DetectedConfig, DetectedProcess, DiscoveredEntity, EnvironmentScan, PreflightResult,
    ProviderHealth, ToolStatus,
};
use crate::storage;

const TOOL_NAMES: [&str; 17] = [
    "node", "npx", "pnpm", "npm", "rustc", "cargo", "git", "python", "python3", "uvx", "codex",
    "claude", "lms", "lmstudio", "hermes", "openclaw", "ollama",
];
const AGENT_PROCESS_MARKERS: [&str; 8] = [
    "codex",
    "claude",
    "hermes",
    "openclaw",
    "lm studio",
    "lmstudio",
    "mcp",
    "agentdeck",
];
const MAX_CONFIG_BYTES: u64 = 1_048_576;
const TOOL_VERSION_TIMEOUT: Duration = Duration::from_secs(3);

#[tauri::command]
pub fn run_preflight() -> PreflightResult {
    let tools = inspect_tools(&TOOL_NAMES);
    let providers = vec![check_lm_studio()];
    let ready = required_tools_available(&tools);

    PreflightResult {
        checked_at: Utc::now().to_rfc3339(),
        tools,
        providers,
        ready,
    }
}

#[tauri::command]
pub fn scan_environment() -> EnvironmentScan {
    let tools = inspect_tools(&TOOL_NAMES);
    let xai = providers::xai_readiness();
    let provider_healths = vec![check_lm_studio(), xai.health.clone()];
    let processes = scan_agent_processes();
    let configs = detect_known_configs();
    let grok = providers::grok_source_agent(&xai);
    let entities = normalize_entities(&tools, &provider_healths, &processes, &configs, grok);

    EnvironmentScan {
        scanned_at: Utc::now().to_rfc3339(),
        tools,
        providers: provider_healths,
        processes,
        configs,
        entities,
    }
}

fn inspect_tools(names: &[&str]) -> Vec<ToolStatus> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let name = *name;
                scope.spawn(move || (index, inspect_tool(name)))
            })
            .collect();

        let mut results = vec![None; names.len()];
        for handle in handles {
            let (index, status) = handle.join().expect("tool inspection thread panicked");
            results[index] = Some(status);
        }

        results
            .into_iter()
            .map(|status| status.expect("missing tool inspection result"))
            .collect()
    })
}

fn inspect_tool(name: &str) -> ToolStatus {
    let Some(path) = find_executable(name) else {
        return ToolStatus {
            name: name.to_owned(),
            available: false,
            version: None,
            path: None,
            error: Some("unavailable".to_owned()),
        };
    };

    let path_text = path.to_string_lossy().into_owned();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let result = Command::new(&path)
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();
        let _ = sender.send(result);
    });

    match receiver.recv_timeout(TOOL_VERSION_TIMEOUT) {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let version = stdout
                .lines()
                .chain(stderr.lines())
                .find(|line| !line.trim().is_empty())
                .map(|line| line.trim().to_owned());

            ToolStatus {
                name: name.to_owned(),
                available: output.status.success(),
                version,
                path: Some(path_text),
                error: (!output.status.success()).then(|| "version check failed".to_owned()),
            }
        }
        Ok(Err(error)) => ToolStatus {
            name: name.to_owned(),
            available: false,
            version: None,
            path: Some(path_text),
            error: Some(error.to_string()),
        },
        Err(mpsc::RecvTimeoutError::Timeout) => ToolStatus {
            name: name.to_owned(),
            available: false,
            version: None,
            path: Some(path_text),
            error: Some("version check timed out".to_owned()),
        },
        Err(mpsc::RecvTimeoutError::Disconnected) => ToolStatus {
            name: name.to_owned(),
            available: false,
            version: None,
            path: Some(path_text),
            error: Some("version check worker exited".to_owned()),
        },
    }
}

fn find_executable(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;

    env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

fn check_lm_studio() -> ProviderHealth {
    let endpoint = "http://localhost:1234/v1/models";
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    {
        Ok(client) => client,
        Err(error) => return provider_unavailable(endpoint, error.to_string()),
    };

    match client.get(endpoint).send() {
        Ok(response) if response.status().is_success() => ProviderHealth {
            name: "LM Studio".to_owned(),
            endpoint: endpoint.to_owned(),
            available: true,
            detail: format!("HTTP {}", response.status().as_u16()),
        },
        Ok(response) => provider_unavailable(
            endpoint,
            format!("endpoint returned HTTP {}", response.status().as_u16()),
        ),
        Err(error) => provider_unavailable(endpoint, error.to_string()),
    }
}

fn provider_unavailable(endpoint: &str, detail: String) -> ProviderHealth {
    ProviderHealth {
        name: "LM Studio".to_owned(),
        endpoint: endpoint.to_owned(),
        available: false,
        detail,
    }
}

fn scan_agent_processes() -> Vec<DetectedProcess> {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_exe(UpdateKind::OnlyIfNotSet)
            .with_cmd(UpdateKind::OnlyIfNotSet),
    );

    let mut processes: Vec<_> = system
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            let name = process.name().to_string_lossy().into_owned();
            let command = process
                .cmd()
                .iter()
                .map(|part| part.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ");
            let searchable = format!("{name} {command}").to_lowercase();
            AGENT_PROCESS_MARKERS
                .iter()
                .any(|marker| searchable.contains(*marker))
                .then(|| DetectedProcess {
                    id: format!("process:{}", pid.as_u32()),
                    pid: pid.as_u32(),
                    name,
                    executable: process
                        .exe()
                        .map(|path| path.to_string_lossy().into_owned()),
                    command: process
                        .cmd()
                        .first()
                        .map(|part| part.to_string_lossy().into_owned()),
                    category: classify_process(&searchable),
                })
        })
        .collect();

    processes.sort_by_key(|process| process.pid);
    processes
}

fn detect_known_configs() -> Vec<DetectedConfig> {
    let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let current = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| env::current_dir().unwrap_or_default());
    let project_content_safe = !["Desktop", "Documents", "Downloads"]
        .iter()
        .any(|folder| current.starts_with(home.join(folder)));
    let candidates = [
        ("Codex", home.join(".codex/config.toml"), true),
        ("Claude Code", home.join(".claude.json"), true),
        ("Claude Code", home.join(".claude/settings.json"), true),
        ("Hermes", home.join(".hermes/config.yaml"), false),
        ("Hermes", home.join(".hermes/config.json"), true),
        ("OpenClaw", home.join(".openclaw/config.json"), true),
        ("LM Studio", home.join(".lmstudio/mcp.json"), true),
        ("Project", current.join("AGENTS.md"), project_content_safe),
        (
            "Codex",
            current.join(".codex/config.toml"),
            project_content_safe,
        ),
        (
            "Claude Code",
            current.join(".claude/settings.json"),
            project_content_safe,
        ),
        ("MCP", current.join(".mcp.json"), project_content_safe),
    ];

    candidates
        .into_iter()
        .map(|(kind, path, parse_content)| inspect_config(kind, &path, parse_content))
        .collect()
}

fn inspect_config(kind: &str, path: &Path, parse_content: bool) -> DetectedConfig {
    let path_text = path.to_string_lossy().into_owned();
    let id = format!("config:{:016x}", storage::stable_hash(&path_text));
    let format = config_format(path);
    let mut config = DetectedConfig {
        id,
        kind: kind.to_owned(),
        path: path_text,
        exists: path.is_file(),
        format: format.map(str::to_owned),
        valid: None,
        top_level_keys: Vec::new(),
        error: None,
    };

    if !config.exists {
        return config;
    }

    if !parse_content {
        config.error = Some("content parsing deferred pending file access".to_owned());
        return config;
    }

    let Some(format) = format else {
        return config;
    };
    if !matches!(format, "json" | "toml") {
        return config;
    }
    match fs::metadata(path) {
        Ok(metadata) if metadata.len() > MAX_CONFIG_BYTES => {
            config.error = Some(format!(
                "skipped: file is larger than {} bytes",
                MAX_CONFIG_BYTES
            ));
            return config;
        }
        Ok(_) => {}
        Err(error) => {
            config.valid = Some(false);
            config.error = Some(safe_error(&error.to_string()));
            return config;
        }
    }

    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) => {
            config.valid = Some(false);
            config.error = Some(safe_error(&error.to_string()));
            return config;
        }
    };

    let parsed = match format {
        "json" => parse_json_keys(&contents),
        "toml" => parse_toml_keys(&contents),
        _ => return config,
    };

    match parsed {
        Ok(keys) => {
            config.valid = Some(true);
            config.top_level_keys = keys;
        }
        Err(error) => {
            config.valid = Some(false);
            config.error = Some(safe_error(&error));
        }
    }

    config
}

fn config_format(path: &Path) -> Option<&'static str> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("json") => Some("json"),
        Some("toml") => Some("toml"),
        Some("yaml" | "yml") => Some("yaml"),
        Some("md") => Some("markdown"),
        _ => None,
    }
}

fn parse_json_keys(contents: &str) -> Result<Vec<String>, String> {
    let value: serde_json::Value =
        serde_json::from_str(contents).map_err(|error| error.to_string())?;
    Ok(value
        .as_object()
        .map(|object| sanitized_keys(object.keys()))
        .unwrap_or_default())
}

fn parse_toml_keys(contents: &str) -> Result<Vec<String>, String> {
    let value: toml::Value = toml::from_str(contents).map_err(|error| error.to_string())?;
    Ok(value
        .as_table()
        .map(|table| sanitized_keys(table.keys()))
        .unwrap_or_default())
}

fn sanitized_keys<'a>(keys: impl Iterator<Item = &'a String>) -> Vec<String> {
    let mut keys: Vec<_> = keys
        .map(|key| {
            if is_secret_key(key) {
                "[redacted-key]".to_owned()
            } else {
                key.to_owned()
            }
        })
        .collect();
    keys.sort();
    keys.dedup();
    keys.truncate(24);
    keys
}

fn is_secret_key(key: &str) -> bool {
    let lowered = key.to_lowercase();
    ["token", "secret", "password", "api_key", "apikey", "auth"]
        .iter()
        .any(|marker| lowered.contains(marker))
}

fn safe_error(error: &str) -> String {
    truncate(error.lines().next().unwrap_or("parse failed"), 180)
}

fn classify_process(searchable: &str) -> String {
    [
        ("codex", "agent"),
        ("claude", "agent"),
        ("hermes", "agent"),
        ("openclaw", "agent"),
        ("lm studio", "provider"),
        ("lmstudio", "provider"),
        ("mcp", "mcp-server"),
    ]
    .iter()
    .find_map(|(marker, category)| searchable.contains(marker).then_some(*category))
    .unwrap_or("related")
    .to_owned()
}

fn normalize_entities(
    tools: &[ToolStatus],
    providers: &[ProviderHealth],
    processes: &[DetectedProcess],
    configs: &[DetectedConfig],
    grok: DiscoveredEntity,
) -> Vec<DiscoveredEntity> {
    let mut entities = Vec::new();

    entities.extend(normalize_agents(tools, processes, configs, providers));
    entities.push(grok);

    entities.extend(tools.iter().map(|tool| {
        let mut metadata = BTreeMap::new();
        insert_some(&mut metadata, "version", tool.version.as_deref());
        insert_some(&mut metadata, "path", tool.path.as_deref());
        DiscoveredEntity {
            id: format!("tool:{}", tool.name),
            entity_type: "tool".to_owned(),
            name: tool.name.clone(),
            status: if tool.available {
                "available"
            } else {
                "unavailable"
            }
            .to_owned(),
            source: "PATH".to_owned(),
            metadata,
        }
    }));

    entities.extend(providers.iter().map(|provider| {
        let mut metadata = BTreeMap::new();
        metadata.insert("providerId".to_owned(), provider_identifier(provider));
        metadata.insert("endpoint".to_owned(), provider.endpoint.clone());
        metadata.insert("detail".to_owned(), provider.detail.clone());
        DiscoveredEntity {
            id: provider_entity_id(provider),
            entity_type: "provider".to_owned(),
            name: provider.name.clone(),
            status: if provider.available {
                "available"
            } else {
                "unavailable"
            }
            .to_owned(),
            source: provider.endpoint.clone(),
            metadata,
        }
    }));

    entities.extend(processes.iter().map(|process| {
        let mut metadata = BTreeMap::new();
        metadata.insert("pid".to_owned(), process.pid.to_string());
        insert_some(&mut metadata, "executable", process.executable.as_deref());
        DiscoveredEntity {
            id: process.id.clone(),
            entity_type: "process".to_owned(),
            name: process.name.clone(),
            status: "running".to_owned(),
            source: "process-table".to_owned(),
            metadata,
        }
    }));

    entities.extend(configs.iter().filter(|config| config.exists).map(|config| {
        let mut metadata = BTreeMap::new();
        insert_some(&mut metadata, "format", config.format.as_deref());
        if let Some(valid) = config.valid {
            metadata.insert("valid".to_owned(), valid.to_string());
        }
        DiscoveredEntity {
            id: config.id.clone(),
            entity_type: "config".to_owned(),
            name: config.kind.clone(),
            status: match config.valid {
                Some(true) | None => "detected",
                Some(false) => "invalid",
            }
            .to_owned(),
            source: config.path.clone(),
            metadata,
        }
    }));

    entities.sort_by(|left, right| left.id.cmp(&right.id));
    entities
}

fn normalize_agents(
    tools: &[ToolStatus],
    processes: &[DetectedProcess],
    configs: &[DetectedConfig],
    providers: &[ProviderHealth],
) -> Vec<DiscoveredEntity> {
    let mut agents = [
        ("agent:codex", "Codex", "codex", "codex"),
        ("agent:claude-code", "Claude Code", "claude", "claude"),
        ("agent:hermes", "Hermes", "hermes", "hermes"),
        ("agent:openclaw", "OpenClaw", "openclaw", "openclaw"),
    ]
    .into_iter()
    .map(|(id, name, command_name, marker)| {
        let tool = tools.iter().find(|tool| tool.name == command_name);
        let running_process = processes
            .iter()
            .find(|process| process_matches(process, marker));
        let running = running_process.is_some();
        let config_count = configs
            .iter()
            .filter(|config| config.exists && config.kind == name)
            .count();
        let mut metadata = BTreeMap::new();
        metadata.insert("configCount".to_owned(), config_count.to_string());
        if let Some(process) = running_process {
            metadata.insert("pid".to_owned(), process.pid.to_string());
        }
        insert_some(
            &mut metadata,
            "command",
            tool.map(|tool| tool.name.as_str()),
        );
        insert_some(
            &mut metadata,
            "version",
            tool.and_then(|tool| tool.version.as_deref()),
        );

        DiscoveredEntity {
            id: id.to_owned(),
            entity_type: "agent".to_owned(),
            name: name.to_owned(),
            status: if running {
                "running"
            } else if tool.is_some_and(|tool| tool.available) {
                "available"
            } else if config_count > 0 {
                "configured"
            } else {
                "unavailable"
            }
            .to_owned(),
            source: "agent-discovery".to_owned(),
            metadata,
        }
    })
    .collect::<Vec<_>>();
    agents.push(normalize_lm_studio_agent(tools, processes, configs, providers));
    agents
}

fn normalize_lm_studio_agent(
    tools: &[ToolStatus],
    processes: &[DetectedProcess],
    configs: &[DetectedConfig],
    providers: &[ProviderHealth],
) -> DiscoveredEntity {
    let tool = tools
        .iter()
        .find(|tool| tool.name == "lms")
        .or_else(|| tools.iter().find(|tool| tool.name == "lmstudio"));
    let running_process = processes.iter().find(|process| {
        process_matches(process, "lmstudio") || process_matches(process, "lm studio")
    });
    let provider = providers.iter().find(|provider| provider.name == "LM Studio");
    let server_available = provider.is_some_and(|provider| provider.available);
    let config_count = configs
        .iter()
        .filter(|config| config.exists && config.kind == "LM Studio")
        .count();
    let mut metadata = BTreeMap::new();
    metadata.insert("configCount".to_owned(), config_count.to_string());
    metadata.insert("providerId".to_owned(), "lmstudio".to_owned());
    metadata.insert("adapterId".to_owned(), "lm-studio".to_owned());
    if let Some(provider) = provider {
        metadata.insert("endpoint".to_owned(), provider.endpoint.clone());
        metadata.insert("healthDetail".to_owned(), provider.detail.clone());
    }
    if let Some(process) = running_process {
        metadata.insert("pid".to_owned(), process.pid.to_string());
    }
    insert_some(
        &mut metadata,
        "command",
        tool.map(|tool| tool.name.as_str()),
    );
    insert_some(
        &mut metadata,
        "version",
        tool.and_then(|tool| tool.version.as_deref()),
    );

    let status = if running_process.is_some() {
        "running"
    } else if server_available || tool.is_some_and(|tool| tool.available) {
        "available"
    } else if config_count > 0 {
        "configured"
    } else {
        "unavailable"
    };

    DiscoveredEntity {
        id: "agent:lm-studio".to_owned(),
        entity_type: "agent".to_owned(),
        name: "LM Studio".to_owned(),
        status: status.to_owned(),
        source: "agent-discovery".to_owned(),
        metadata,
    }
}

fn provider_entity_id(provider: &ProviderHealth) -> String {
    if provider.name == "LM Studio" && provider.endpoint == "http://localhost:1234/v1/models" {
        return "provider:lmstudio:http-localhost-1234-v1".to_owned();
    }

    format!(
        "provider:{}:{:016x}",
        provider_identifier(provider),
        storage::stable_hash(&provider.endpoint)
    )
}

fn provider_identifier(provider: &ProviderHealth) -> String {
    match provider.name.as_str() {
        "LM Studio" => "lmstudio".to_owned(),
        "xAI" => "xai".to_owned(),
        other => slugify(other),
    }
}

fn slugify(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_owned()
}

fn process_matches(process: &DetectedProcess, marker: &str) -> bool {
    let marker = marker.to_lowercase();
    [&process.name, process.executable.as_deref().unwrap_or("")]
        .iter()
        .any(|value| value.to_lowercase().contains(&marker))
}

fn insert_some(metadata: &mut BTreeMap<String, String>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        metadata.insert(key.to_owned(), value.to_owned());
    }
}



fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

fn required_tools_available(tools: &[ToolStatus]) -> bool {
    ["node", "pnpm", "rustc", "cargo"].iter().all(|required| {
        tools
            .iter()
            .any(|tool| tool.name == *required && tool.available)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_tool_is_reported_as_unavailable() {
        let status = inspect_tool("agentdeck-tool-that-does-not-exist");
        assert!(!status.available);
        assert_eq!(status.error.as_deref(), Some("unavailable"));
    }

    #[test]
    fn readiness_requires_the_core_toolchain() {
        let tools = ["node", "pnpm", "rustc", "cargo"]
            .map(|name| ToolStatus {
                name: name.to_owned(),
                available: true,
                version: None,
                path: None,
                error: None,
            })
            .to_vec();

        assert!(required_tools_available(&tools));
    }

    #[test]
    fn config_status_only_reports_metadata() {
        let status = inspect_config("Test", Path::new("/not/a/real/config.json"), true);
        assert!(!status.exists);
        assert!(status.id.starts_with("config:"));
        assert_eq!(status.format.as_deref(), Some("json"));
    }

    #[test]
    fn json_parser_redacts_secret_like_keys() {
        let keys = parse_json_keys(r#"{"mcpServers": {}, "api_token": "not-returned"}"#).unwrap();
        assert_eq!(keys, vec!["[redacted-key]", "mcpServers"]);
    }

    #[test]
    fn stable_ids_are_repeatable() {
        assert_eq!(
            storage::stable_hash("/tmp/config.json"),
            storage::stable_hash("/tmp/config.json")
        );
        assert_ne!(
            storage::stable_hash("/tmp/config.json"),
            storage::stable_hash("/tmp/other.json")
        );
    }

    #[test]
    fn running_agent_includes_pid_metadata() {
        let tools = vec![ToolStatus {
            name: "codex".to_owned(),
            available: true,
            version: Some("codex-cli test".to_owned()),
            path: Some("/usr/local/bin/codex".to_owned()),
            error: None,
        }];
        let processes = vec![DetectedProcess {
            id: "process:4242".to_owned(),
            pid: 4242,
            name: "codex".to_owned(),
            executable: Some("/usr/local/bin/codex".to_owned()),
            command: Some("codex".to_owned()),
            category: "agent".to_owned(),
        }];

        let agents = normalize_agents(&tools, &processes, &[], &[]);
        let codex = agents
            .iter()
            .find(|entity| entity.id == "agent:codex")
            .expect("codex agent entity");

        assert_eq!(codex.status, "running");
        assert_eq!(codex.metadata.get("pid").map(String::as_str), Some("4242"));
    }

    #[test]
    fn normalized_agents_include_status_from_tools() {
        let tools = vec![ToolStatus {
            name: "codex".to_owned(),
            available: true,
            version: Some("codex-cli test".to_owned()),
            path: Some("/usr/local/bin/codex".to_owned()),
            error: None,
        }];

        let agents = normalize_agents(&tools, &[], &[], &[]);
        let codex = agents
            .iter()
            .find(|entity| entity.id == "agent:codex")
            .expect("codex agent entity");

        assert_eq!(codex.status, "available");
        assert_eq!(
            codex.metadata.get("version").map(String::as_str),
            Some("codex-cli test")
        );
    }

    #[test]
    fn lm_studio_agent_reflects_cli_and_server_health() {
        let tools = vec![ToolStatus {
            name: "lms".to_owned(),
            available: true,
            version: Some("LM Studio CLI 0.3.0".to_owned()),
            path: Some("/Users/test/.lmstudio/bin/lms".to_owned()),
            error: None,
        }];
        let providers = vec![ProviderHealth {
            name: "LM Studio".to_owned(),
            endpoint: "http://localhost:1234/v1/models".to_owned(),
            available: true,
            detail: "HTTP 200".to_owned(),
        }];

        let agents = normalize_agents(&tools, &[], &[], &providers);
        let lm_studio = agents
            .iter()
            .find(|entity| entity.id == "agent:lm-studio")
            .expect("lm studio agent entity");

        assert_eq!(lm_studio.status, "available");
        assert_eq!(
            lm_studio.metadata.get("adapterId").map(String::as_str),
            Some("lm-studio")
        );
        assert_eq!(
            lm_studio.metadata.get("healthDetail").map(String::as_str),
            Some("HTTP 200")
        );
    }

    #[test]
    fn normalized_entities_include_grok_source_agent() {
        let grok = providers::grok_source_agent(&providers::XaiReadiness {
            credential_status: "stored".to_owned(),
            subscription_active: true,
            health: ProviderHealth {
                name: "xAI".to_owned(),
                endpoint: "https://api.x.ai/v1/models".to_owned(),
                available: true,
                detail: "HTTP 200".to_owned(),
            },
        });

        let entities = normalize_entities(&[], &[], &[], &[], grok);
        let grok = entities
            .iter()
            .find(|entity| entity.id == "agent:grok")
            .expect("grok agent entity");

        assert_eq!(grok.status, "available");
        assert_eq!(grok.source, "xai");
        assert_eq!(
            grok.metadata.get("providerId").map(String::as_str),
            Some("xai")
        );
    }

    #[test]
    fn xai_readiness_completes_quickly() {
        let started = std::time::Instant::now();
        let _ = providers::xai_readiness();
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "xAI readiness exceeded 8 seconds"
        );
    }

    #[test]
    fn detect_known_configs_completes_quickly() {
        let started = std::time::Instant::now();
        let configs = detect_known_configs();
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "config detection exceeded 8 seconds, found {} configs",
            configs.len()
        );
    }

    #[test]
    fn scan_agent_processes_completes_quickly() {
        let started = std::time::Instant::now();
        let processes = scan_agent_processes();
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "process scan exceeded 8 seconds, found {} processes",
            processes.len()
        );
    }

    #[test]
    fn inspect_tools_completes_quickly() {
        let started = std::time::Instant::now();
        let tools = inspect_tools(&TOOL_NAMES);
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "tool inspection exceeded 8 seconds"
        );
        assert_eq!(tools.len(), TOOL_NAMES.len());
    }

    #[test]
    fn environment_scan_completes() {
        let started = std::time::Instant::now();
        let scan = scan_environment();
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "environment scan exceeded 15 seconds"
        );
        assert!(!scan.scanned_at.is_empty());
        assert_eq!(scan.tools.len(), TOOL_NAMES.len());
    }
}
