use std::env;

use crate::tunnel_control;

const LOOPBACK_RESOURCE_URL: &str = "http://127.0.0.1:7823/mcp";

pub fn mcp_public_resource_url() -> String {
    for key in ["MCP_PUBLIC_RESOURCE_URL", "AGENTDECK_MCP_URL"] {
        if let Some(url) = read_https_env(key) {
            return normalize_mcp_resource_url(&url);
        }
        if let Some(url) = tunnel_control::tunnel_config_value(key).filter(|url| is_https_url(url)) {
            return normalize_mcp_resource_url(&url);
        }
    }

    LOOPBACK_RESOURCE_URL.to_owned()
}

fn read_https_env(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .filter(|value| is_https_url(value))
}

fn is_https_url(value: &str) -> bool {
    value.starts_with("https://")
}

pub fn normalize_mcp_resource_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return LOOPBACK_RESOURCE_URL.to_owned();
    }
    if trimmed.ends_with("/mcp/") {
        return trimmed.trim_end_matches('/').to_owned();
    }
    if trimmed.ends_with('/') && !trimmed.ends_with("/mcp") {
        return trimmed.trim_end_matches('/').to_owned();
    }
    trimmed.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_trailing_slash() {
        assert_eq!(
            normalize_mcp_resource_url("https://mcp.example.com/mcp/"),
            "https://mcp.example.com/mcp"
        );
    }

    #[test]
    fn falls_back_to_loopback_without_public_config() {
        let previous_public = env::var("MCP_PUBLIC_RESOURCE_URL").ok();
        let previous_upstream = env::var("AGENTDECK_MCP_URL").ok();
        let previous_tunnel_env = env::var("AGENTDECK_TUNNEL_ENV").ok();
        let temp_config = std::env::temp_dir().join(format!(
            "agentdeck-empty-tunnel-{}.env",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::write(
            &temp_config,
            "export OPENAI_TUNNEL_ID=\"tunnel_test\"\nexport OPENAI_API_KEY=\"sk-test\"\n",
        )
        .expect("temp tunnel config");
        env::remove_var("MCP_PUBLIC_RESOURCE_URL");
        env::remove_var("AGENTDECK_MCP_URL");
        env::set_var("AGENTDECK_TUNNEL_ENV", &temp_config);

        assert_eq!(mcp_public_resource_url(), LOOPBACK_RESOURCE_URL);

        restore_env("MCP_PUBLIC_RESOURCE_URL", previous_public);
        restore_env("AGENTDECK_MCP_URL", previous_upstream);
        restore_env("AGENTDECK_TUNNEL_ENV", previous_tunnel_env);
        let _ = std::fs::remove_file(temp_config);
    }

    #[test]
    fn prefers_explicit_public_resource_env() {
        let previous_public = env::var("MCP_PUBLIC_RESOURCE_URL").ok();
        let previous_upstream = env::var("AGENTDECK_MCP_URL").ok();
        env::set_var(
            "MCP_PUBLIC_RESOURCE_URL",
            "https://api.openai.com/v1/tunnel/tunnel_test",
        );
        env::set_var("AGENTDECK_MCP_URL", "https://mcp.example.com/mcp");

        assert_eq!(
            mcp_public_resource_url(),
            "https://api.openai.com/v1/tunnel/tunnel_test"
        );

        restore_env("MCP_PUBLIC_RESOURCE_URL", previous_public);
        restore_env("AGENTDECK_MCP_URL", previous_upstream);
    }

    fn restore_env(key: &str, value: Option<String>) {
        match value {
            Some(value) => env::set_var(key, value),
            None => env::remove_var(key),
        }
    }
}