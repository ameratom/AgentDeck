use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

use chrono::Utc;
use reqwest::blocking::Client;
use serde::Serialize;
use serde_json::{json, Value};

use crate::mcp_http::MCP_HTTP_PORT;
use crate::mcp_public_url;
use crate::mcp_server;
use crate::tunnel_control;

const PLATFORM_STATUS_REVIEW: &str = "REVIEW";
const PUBLISH_BLOCKED_REASON: &str =
    "OpenAI has not approved this app version yet. Publishing unlocks after status becomes Approved.";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewCheck {
    pub id: String,
    pub label: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatgptReviewHealth {
    pub checked_at: String,
    pub platform_status: String,
    pub publish_allowed: bool,
    pub publish_blocked_reason: String,
    pub ready_for_reviewers: bool,
    pub submission_tool_count: usize,
    pub public_mcp_url: Option<String>,
    pub checks: Vec<ReviewCheck>,
}

pub fn evaluate_default_review_health() -> Result<ChatgptReviewHealth, String> {
    let database_path = crate::storage::resolve_database_path(None)?;
    evaluate_review_health(&database_path)
}

pub fn evaluate_review_health(database_path: &Path) -> Result<ChatgptReviewHealth, String> {
    let checked_at = Utc::now().to_rfc3339();
    let profile = mcp_server::evaluate_submission_profile();
    let tunnel = tunnel_control::status(database_path)?;
    let public_mcp_url = public_mcp_url_if_configured();
    let local_mcp_listening = localhost_mcp_listening();
    let local_tools_reachable = if local_mcp_listening {
        probe_tools_list_count(&format!("http://127.0.0.1:{MCP_HTTP_PORT}/mcp"))
    } else {
        Err("local MCP HTTP server is not listening".to_owned())
    };
    let public_tools_reachable = public_mcp_url
        .as_ref()
        .map(|url| probe_tools_list_count(url))
        .unwrap_or(Err(
            "public MCP URL is not configured (set MCP_PUBLIC_RESOURCE_URL in tunnel env)"
                .to_owned(),
        ));

    let expected_count = profile.tool_count.max(10);
    let mut checks = vec![
        ReviewCheck {
            id: "platform-review".to_owned(),
            label: "Platform status".to_owned(),
            passed: true,
            detail: format!(
                "App version is in {PLATFORM_STATUS_REVIEW}. Keep AgentDeck and the tunnel running until OpenAI finishes review."
            ),
        },
        ReviewCheck {
            id: "publish-gate".to_owned(),
            label: "Publish gate".to_owned(),
            passed: false,
            detail: PUBLISH_BLOCKED_REASON.to_owned(),
        },
        ReviewCheck {
            id: "local-mcp-listening".to_owned(),
            label: "Local MCP server".to_owned(),
            passed: local_mcp_listening,
            detail: if local_mcp_listening {
                format!("Listening on http://127.0.0.1:{MCP_HTTP_PORT}/mcp")
            } else {
                "AgentDeck MCP HTTP is not reachable. Launch AgentDeck or restart the app.".to_owned()
            },
        },
        ReviewCheck {
            id: "tunnel-ready".to_owned(),
            label: "Secure MCP Tunnel".to_owned(),
            passed: tunnel.ready,
            detail: tunnel.detail,
        },
        ReviewCheck {
            id: "submission-tool-count".to_owned(),
            label: "Submission tool count".to_owned(),
            passed: profile.tool_count == expected_count
                && profile.missing_from_manifest.is_empty()
                && profile.unexpected_tools.is_empty(),
            detail: if profile.missing_from_manifest.is_empty() && profile.unexpected_tools.is_empty() {
                format!(
                    "HTTP profile exposes {} tools matching chatgpt-app-submission.json",
                    profile.tool_count
                )
            } else {
                format!(
                    "manifest mismatch (missing: {:?}, unexpected: {:?})",
                    profile.missing_from_manifest, profile.unexpected_tools
                )
            },
        },
        ReviewCheck {
            id: "no-write-tools".to_owned(),
            label: "Write tools excluded".to_owned(),
            passed: profile.deferred_tools_exposed.is_empty(),
            detail: if profile.deferred_tools_exposed.is_empty() {
                "dispatch_handoff, execute_skill, and toggle_mcp_server are not in the HTTP profile"
                    .to_owned()
            } else {
                format!(
                    "deferred write tools exposed: {}",
                    profile.deferred_tools_exposed.join(", ")
                )
            },
        },
        ReviewCheck {
            id: "output-schemas".to_owned(),
            label: "Output schemas".to_owned(),
            passed: profile.missing_output_schema.is_empty(),
            detail: if profile.missing_output_schema.is_empty() {
                "All submission tools include outputSchema".to_owned()
            } else {
                format!(
                    "missing outputSchema: {}",
                    profile.missing_output_schema.join(", ")
                )
            },
        },
        ReviewCheck {
            id: "read-only-hints".to_owned(),
            label: "Read-only annotations".to_owned(),
            passed: profile.non_read_only_tools.is_empty(),
            detail: if profile.non_read_only_tools.is_empty() {
                "All submission tools set readOnlyHint=true".to_owned()
            } else {
                format!(
                    "non-read-only tools: {}",
                    profile.non_read_only_tools.join(", ")
                )
            },
        },
        ReviewCheck {
            id: "local-tools-list".to_owned(),
            label: "Local tools/list probe".to_owned(),
            passed: local_tools_reachable
                .as_ref()
                .ok()
                .copied()
                .is_some_and(|count| count == profile.tool_count),
            detail: match local_tools_reachable {
                Ok(count) => format!("tools/list returned {count} tools over loopback HTTP"),
                Err(error) => error,
            },
        },
    ];

    if public_mcp_url.is_some() {
        checks.push(ReviewCheck {
            id: "public-tools-list".to_owned(),
            label: "Public tools/list probe".to_owned(),
            passed: public_tools_reachable
                .as_ref()
                .ok()
                .copied()
                .is_some_and(|count| count == profile.tool_count),
            detail: match public_tools_reachable {
                Ok(count) => format!(
                    "Public endpoint returned {count} tools (reviewers use this URL)"
                ),
                Err(error) => error,
            },
        });
    }

    let operational_checks = checks
        .iter()
        .filter(|check| {
            !matches!(
                check.id.as_str(),
                "platform-review" | "publish-gate" | "public-tools-list"
            )
        })
        .all(|check| check.passed);
    let public_probe_ok = public_mcp_url
        .as_ref()
        .map(|_| {
            checks
                .iter()
                .find(|check| check.id == "public-tools-list")
                .is_some_and(|check| check.passed)
        })
        .unwrap_or(true);

    Ok(ChatgptReviewHealth {
        checked_at,
        platform_status: PLATFORM_STATUS_REVIEW.to_owned(),
        publish_allowed: false,
        publish_blocked_reason: PUBLISH_BLOCKED_REASON.to_owned(),
        ready_for_reviewers: operational_checks && public_probe_ok,
        submission_tool_count: profile.tool_count,
        public_mcp_url,
        checks,
    })
}

fn public_mcp_url_if_configured() -> Option<String> {
    let url = mcp_public_url::mcp_public_resource_url();
    if url.starts_with("https://") {
        Some(url)
    } else {
        None
    }
}

fn localhost_mcp_listening() -> bool {
    let address = SocketAddr::from(([127, 0, 0, 1], MCP_HTTP_PORT));
    TcpStream::connect_timeout(&address, Duration::from_millis(500)).is_ok()
}

fn probe_tools_list_count(url: &str) -> Result<usize, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|error| format!("failed to build HTTP client: {error}"))?;
    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        }))
        .send()
        .map_err(|error| format!("tools/list request failed for {url}: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "tools/list returned HTTP {} for {url}",
            response.status()
        ));
    }

    let body: Value = response
        .json()
        .map_err(|error| format!("tools/list response was not JSON for {url}: {error}"))?;
    if body.get("error").is_some() {
        return Err(format!(
            "tools/list JSON-RPC error for {url}: {}",
            body.get("error")
                .and_then(|value| value.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("unknown error")
        ));
    }

    let count = body
        .get("result")
        .and_then(|value| value.get("tools"))
        .and_then(Value::as_array)
        .map(|tools| tools.len())
        .unwrap_or(0);
    if count == 0 {
        return Err(format!("tools/list returned zero tools for {url}"));
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submission_profile_report_matches_manifest() {
        let profile = mcp_server::evaluate_submission_profile();
        assert_eq!(profile.tool_count, 10);
        assert!(profile.missing_from_manifest.is_empty());
        assert!(profile.unexpected_tools.is_empty());
        assert!(profile.deferred_tools_exposed.is_empty());
        assert!(profile.missing_output_schema.is_empty());
        assert!(profile.non_read_only_tools.is_empty());
    }

    #[test]
    fn review_health_includes_publish_gate() {
        let path = std::env::temp_dir().join(format!(
            "agentdeck-review-health-{}.sqlite3",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let connection = crate::storage::open_database(&path).expect("open database");
        drop(connection);
        let health = evaluate_review_health(&path).expect("review health");
        assert_eq!(health.platform_status, "REVIEW");
        assert!(!health.publish_allowed);
        assert!(health
            .checks
            .iter()
            .any(|check| check.id == "publish-gate" && !check.passed));
        let _ = std::fs::remove_file(path);
    }
}