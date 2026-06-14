use std::collections::BTreeMap;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use chrono::Utc;
use rusqlite::params;
use serde::Serialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::storage;
use crate::tool_path;

const CONFIG_FILE_NAME: &str = "chatgpt-mcp-tunnel.env";
const DEFAULT_MCP_URL: &str = "http://127.0.0.1:7823/mcp";
const DEFAULT_HEALTH_URL: &str = "http://127.0.0.1:8081";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelStatus {
    pub configured: bool,
    pub running: bool,
    pub ready: bool,
    pub pid: Option<u32>,
    pub config_path: String,
    pub admin_url: Option<String>,
    pub log_path: String,
    pub detail: String,
}

struct TunnelPaths {
    config: PathBuf,
    runtime_dir: PathBuf,
    pid: PathBuf,
    health_url: PathBuf,
    log: PathBuf,
}

pub fn status(database_path: &Path) -> Result<TunnelStatus, String> {
    let paths = tunnel_paths(database_path)?;
    let config = load_config(&paths.config).unwrap_or_default();
    let configured = config_is_ready(&config);
    let managed_pid = read_pid(&paths.pid).filter(|pid| tunnel_process_running(*pid));
    let admin_base =
        read_health_url(&paths.health_url).unwrap_or_else(|| DEFAULT_HEALTH_URL.to_owned());
    let health_ready = endpoint_ready(&format!("{admin_base}/readyz"));
    let managed = managed_pid.is_some();
    let ready = health_ready;
    let detail = if ready && managed {
        "OpenAI Secure MCP Tunnel is connected and ready.".to_owned()
    } else if ready {
        "Tunnel is ready on 127.0.0.1:8081 via an external tunnel-client process. AgentDeck does not manage that PID. Stop the external tunnel, then use Start tunnel here to manage it from the app.".to_owned()
    } else if managed {
        "Tunnel process is running but is not ready yet. Check the operator UI or log.".to_owned()
    } else if configured {
        "Tunnel configuration is ready. Start it when ChatGPT needs local MCP access.".to_owned()
    } else {
        format!(
            "Tunnel credentials are missing. Configure {} first.",
            paths.config.display()
        )
    };

    Ok(TunnelStatus {
        configured,
        running: managed,
        ready,
        pid: managed_pid,
        config_path: paths.config.display().to_string(),
        admin_url: if ready {
            Some(format!("{admin_base}/ui"))
        } else {
            managed_pid.map(|_| format!("{admin_base}/ui"))
        },
        log_path: paths.log.display().to_string(),
        detail,
    })
}

pub fn start(database_path: &Path) -> Result<TunnelStatus, String> {
    let paths = tunnel_paths(database_path)?;
    let current = status(database_path)?;
    if current.ready {
        return Ok(current);
    }
    if endpoint_ready(&format!("{DEFAULT_HEALTH_URL}/readyz")) {
        return Err(
            "Port 127.0.0.1:8081 is already in use by another tunnel-client process. Stop the external tunnel first, then click Start tunnel.".to_owned(),
        );
    }

    let config = load_config(&paths.config)?;
    validate_config(&config, &paths.config)?;
    ensure_local_mcp_available()?;
    fs::create_dir_all(&paths.runtime_dir)
        .map_err(|error| format!("failed to create tunnel runtime directory: {error}"))?;
    remove_if_exists(&paths.pid)?;
    remove_if_exists(&paths.health_url)?;

    let tunnel_id = required_config(&config, "OPENAI_TUNNEL_ID")?;
    let api_key = required_config(&config, "OPENAI_API_KEY")?;
    let mcp_url = config
        .get("AGENTDECK_MCP_URL")
        .cloned()
        .unwrap_or_else(|| DEFAULT_MCP_URL.to_owned());
    let binary = resolve_tunnel_client_binary(&config, &paths.config)?;

    let mut command = Command::new(&binary);
    command
        .arg("run")
        .arg("--control-plane.tunnel-id")
        .arg(tunnel_id)
        .arg("--control-plane.api-key")
        .arg("env:OPENAI_API_KEY")
        .arg("--mcp.server-url")
        .arg(format!("url={mcp_url},channel=main"))
        .arg("--health.listen-addr")
        .arg("127.0.0.1:8081")
        .arg("--health.url-file")
        .arg(&paths.health_url)
        .arg("--pid.file")
        .arg(&paths.pid)
        .arg("--log.file")
        .arg(&paths.log)
        .env("OPENAI_API_KEY", api_key)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let child = command
        .spawn()
        .map_err(|error| {
            format!(
                "failed to start {}: {error}",
                binary.display()
            )
        })?;
    fs::write(&paths.pid, child.id().to_string())
        .map_err(|error| format!("failed to record tunnel PID: {error}"))?;

    let deadline = Instant::now() + Duration::from_secs(6);
    while Instant::now() < deadline {
        let next = status(database_path)?;
        if next.ready {
            audit(database_path, "tunnel-start", "success")?;
            return Ok(next);
        }
        if !next.running {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    let next = status(database_path)?;
    audit(
        database_path,
        "tunnel-start",
        if next.running { "pending" } else { "failed" },
    )?;
    if next.running {
        Ok(next)
    } else {
        Err(format!(
            "tunnel-client exited during startup. Check {}",
            paths.log.display()
        ))
    }
}

pub fn stop(database_path: &Path) -> Result<TunnelStatus, String> {
    let paths = tunnel_paths(database_path)?;
    let Some(pid) = read_pid(&paths.pid) else {
        return status(database_path);
    };
    if !tunnel_process_running(pid) {
        remove_if_exists(&paths.pid)?;
        remove_if_exists(&paths.health_url)?;
        return status(database_path);
    }

    let result = Command::new("/bin/kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status()
        .map_err(|error| format!("failed to stop tunnel process {pid}: {error}"))?;
    if !result.success() {
        return Err(format!("failed to stop tunnel process {pid}"));
    }

    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline && tunnel_process_running(pid) {
        std::thread::sleep(Duration::from_millis(100));
    }
    if tunnel_process_running(pid) {
        return Err(format!(
            "tunnel process {pid} did not stop after SIGTERM; see {}",
            paths.log.display()
        ));
    }
    remove_if_exists(&paths.pid)?;
    remove_if_exists(&paths.health_url)?;
    audit(database_path, "tunnel-stop", "success")?;
    status(database_path)
}

pub fn open_operator_ui(database_path: &Path) -> Result<TunnelStatus, String> {
    let current = status(database_path)?;
    let url = current
        .admin_url
        .as_deref()
        .ok_or_else(|| "start the tunnel before opening its operator UI".to_owned())?;
    let result = Command::new("/usr/bin/open")
        .arg(url)
        .status()
        .map_err(|error| format!("failed to open tunnel operator UI: {error}"))?;
    if !result.success() {
        return Err("macOS could not open the tunnel operator UI".to_owned());
    }
    Ok(current)
}

pub fn openai_apps_challenge_token() -> Option<String> {
    tunnel_config_value("OPENAI_APPS_CHALLENGE_TOKEN")
}

pub fn tunnel_config_value(key: &str) -> Option<String> {
    let database_path = storage::home_database_path().ok()?;
    let paths = tunnel_paths(&database_path).ok()?;
    load_config(&paths.config)
        .ok()?
        .get(key)
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
}

fn tunnel_paths(database_path: &Path) -> Result<TunnelPaths, String> {
    let app_data = database_path
        .parent()
        .ok_or_else(|| "database path has no parent directory".to_owned())?;
    let config = std::env::var_os("AGENTDECK_TUNNEL_ENV")
        .map(PathBuf::from)
        .unwrap_or_else(|| app_data.join(CONFIG_FILE_NAME));
    let runtime_dir = app_data.join("tunnel-client");
    Ok(TunnelPaths {
        config,
        pid: runtime_dir.join("tunnel-client.pid"),
        health_url: runtime_dir.join("health-url.txt"),
        log: runtime_dir.join("tunnel-client.log"),
        runtime_dir,
    })
}

fn load_config(path: &Path) -> Result<BTreeMap<String, String>, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read tunnel config {}: {error}", path.display()))?;
    let mut config = BTreeMap::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !key
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            continue;
        }
        config.insert(key.to_owned(), unquote(raw_value.trim()));
    }
    Ok(config)
}

fn unquote(value: &str) -> String {
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].to_owned()
    } else {
        value.to_owned()
    }
}

fn config_is_ready(config: &BTreeMap<String, String>) -> bool {
    validate_config(config, Path::new("tunnel config")).is_ok()
}

fn validate_config(config: &BTreeMap<String, String>, path: &Path) -> Result<(), String> {
    let tunnel_id = required_config(config, "OPENAI_TUNNEL_ID")?;
    let api_key = required_config(config, "OPENAI_API_KEY")?;
    if tunnel_id == "tunnel_..." || tunnel_id.contains("YOUR_TUNNEL_ID_HERE") {
        return Err(format!(
            "replace the placeholder OPENAI_TUNNEL_ID in {}",
            path.display()
        ));
    }
    if api_key == "sk-..." || api_key.contains("YOUR_OPENAI_API_KEY_HERE") {
        return Err(format!(
            "replace the placeholder OPENAI_API_KEY in {}",
            path.display()
        ));
    }
    Ok(())
}

fn required_config<'a>(config: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, String> {
    config
        .get(key)
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{key} is missing from tunnel configuration"))
}

fn resolve_tunnel_client_binary(
    config: &BTreeMap<String, String>,
    config_path: &Path,
) -> Result<PathBuf, String> {
    let configured = config
        .get("TUNNEL_CLIENT_BIN")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("tunnel-client");
    let candidate = PathBuf::from(configured);
    if candidate.is_absolute() {
        if candidate.is_file() {
            return Ok(candidate);
        }
        return Err(format!(
            "TUNNEL_CLIENT_BIN points to a missing file: {}",
            candidate.display()
        ));
    }

    let name = candidate
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(configured);
    tool_path::find_executable(name, &[])
        .map(|resolved| resolved.path)
        .ok_or_else(|| {
            format!(
                "tunnel-client not found for GUI launch. Install OpenAI tunnel-client or set TUNNEL_CLIENT_BIN to its full path (for example /usr/local/bin/tunnel-client) in {}",
                config_path.display()
            )
        })
}

fn ensure_local_mcp_available() -> Result<(), String> {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 7823);
    TcpStream::connect_timeout(&address, Duration::from_secs(1))
        .map(|_| ())
        .map_err(|_| "AgentDeck MCP is not listening on 127.0.0.1:7823".to_owned())
}

fn read_pid(path: &Path) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn tunnel_process_running(pid: u32) -> bool {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[Pid::from_u32(pid)]),
        true,
        ProcessRefreshKind::nothing()
            .with_exe(UpdateKind::OnlyIfNotSet)
            .with_cmd(UpdateKind::OnlyIfNotSet),
    );
    let Some(process) = system.process(Pid::from_u32(pid)) else {
        return false;
    };
    let searchable = format!(
        "{} {}",
        process.name().to_string_lossy(),
        process
            .cmd()
            .iter()
            .map(|part| part.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ")
    )
    .to_lowercase();
    searchable.contains("tunnel-client")
}

fn read_health_url(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_owned())
        .filter(|value| !value.is_empty())
}

fn endpoint_ready(url: &str) -> bool {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(700))
        .build()
        .and_then(|client| client.get(url).send())
        .is_ok_and(|response| response.status().is_success())
}

fn remove_if_exists(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("failed to remove {}: {error}", path.display())),
    }
}

fn audit(database_path: &Path, action: &str, status: &str) -> Result<(), String> {
    let connection = storage::open_database(database_path)?;
    let created_at = Utc::now();
    let id = format!(
        "audit:{:016x}",
        storage::stable_hash(&format!("{action}:{created_at}"))
    );
    connection
        .execute(
            "INSERT INTO audit_events
                (id, action, status, model, conversation_id, duration_ms, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
            params![
                id,
                action,
                status,
                "tunnel-client",
                "secure-mcp-tunnel",
                created_at.to_rfc3339()
            ],
        )
        .map_err(|error| format!("failed to store tunnel audit event: {error}"))?;
    storage::append_log_event(
        database_path,
        "audit_event",
        serde_json::json!({
            "id": id,
            "action": action,
            "status": status,
            "model": "tunnel-client",
            "conversationId": "secure-mcp-tunnel",
            "createdAt": created_at.to_rfc3339()
        }),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exported_tunnel_config_without_exposing_values() {
        let path = std::env::temp_dir().join(format!(
            "agentdeck-tunnel-config-{}.env",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        fs::write(
            &path,
            "export OPENAI_TUNNEL_ID=\"tunnel_test\"\nexport OPENAI_API_KEY='sk-test'\n",
        )
        .unwrap();
        let config = load_config(&path).unwrap();
        assert_eq!(
            config.get("OPENAI_TUNNEL_ID").map(String::as_str),
            Some("tunnel_test")
        );
        assert!(config_is_ready(&config));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_placeholder_tunnel_config() {
        let config = BTreeMap::from([
            ("OPENAI_TUNNEL_ID".to_owned(), "tunnel_...".to_owned()),
            ("OPENAI_API_KEY".to_owned(), "sk-...".to_owned()),
        ]);
        assert!(!config_is_ready(&config));
    }

    #[test]
    #[ignore = "live smoke: starts the configured OpenAI tunnel and stops it"]
    fn live_smoke_start_and_stop_home_tunnel() {
        let database_path = storage::home_database_path().expect("home database path");
        let started = start(&database_path).expect("start secure tunnel");
        assert!(started.running);
        let stopped = stop(&database_path).expect("stop secure tunnel");
        assert!(!stopped.running);
    }
}
