use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use super::composer::{ComposerError, ComposerRequest, ComposerResponse};

const DEFAULT_MODEL: &str = "composer-2.5-fast";
const DEFAULT_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposerBridgeKind {
    CursorAgent,
    DryRun,
}

pub fn bridge_kind_from_env() -> ComposerBridgeKind {
    match std::env::var("AGENTDECK_COMPOSER_BRIDGE")
        .unwrap_or_else(|_| "cursor-agent".to_owned())
        .to_ascii_lowercase()
        .as_str()
    {
        "dry-run" => ComposerBridgeKind::DryRun,
        _ => ComposerBridgeKind::CursorAgent,
    }
}

pub fn invoke_with_bridge(request: &ComposerRequest) -> Result<ComposerResponse, ComposerError> {
    invoke_with_bridge_kind(request, bridge_kind_from_env())
}

pub fn invoke_with_bridge_kind(
    request: &ComposerRequest,
    kind: ComposerBridgeKind,
) -> Result<ComposerResponse, ComposerError> {
    match kind {
        ComposerBridgeKind::DryRun => invoke_dry_run(request),
        ComposerBridgeKind::CursorAgent => invoke_cursor_agent(request),
    }
}

fn invoke_dry_run(request: &ComposerRequest) -> Result<ComposerResponse, ComposerError> {
    Ok(ComposerResponse {
        patch_text: String::new(),
        summary: format!(
            "dry-run bridge acknowledged task `{}` without calling Cursor",
            request.task_id
        ),
        suggested_tests: vec!["pnpm verify".to_owned()],
        suggested_commands: vec!["pnpm verify".to_owned()],
    })
}

fn invoke_cursor_agent(request: &ComposerRequest) -> Result<ComposerResponse, ComposerError> {
    ensure_cursor_available()?;
    ensure_cursor_auth()?;

    let model = std::env::var("AGENTDECK_COMPOSER_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_owned());
    let timeout_secs = std::env::var("AGENTDECK_COMPOSER_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_TIMEOUT_SECS);

    let prompt = build_prompt(request);
    let repo_root = Path::new(&request.repo_root);
    if !repo_root.is_dir() {
        return Err(ComposerError::InvalidRequest(format!(
            "repo_root does not exist: {}",
            request.repo_root
        )));
    }

    let mut command = Command::new("cursor");
    command
        .arg("agent")
        .arg("--print")
        .arg("--trust")
        .arg("--mode")
        .arg("plan")
        .arg("--workspace")
        .arg(repo_root)
        .arg("--model")
        .arg(&model)
        .arg("--output-format")
        .arg("text")
        .arg(&prompt);

    if let Ok(api_key) = std::env::var("CURSOR_API_KEY") {
        if !api_key.trim().is_empty() {
            command.arg("--api-key").arg(api_key);
        }
    }

    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let child = command
        .spawn()
        .map_err(|error| ComposerError::InvocationFailed(format!("failed to spawn cursor agent: {error}")))?;

    let output = wait_with_timeout(child, Duration::from_secs(timeout_secs))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else {
            stdout.trim().to_string()
        };
        if detail.to_ascii_lowercase().contains("not logged in")
            || detail.to_ascii_lowercase().contains("authentication")
        {
            return Err(ComposerError::AuthRequired);
        }
        return Err(ComposerError::InvocationFailed(format!(
            "cursor agent exited with {}: {detail}",
            output.status.code().unwrap_or(-1)
        )));
    }

    let raw = String::from_utf8_lossy(&output.stdout).to_string();
    parse_cursor_response(&raw)
}

fn ensure_cursor_available() -> Result<(), ComposerError> {
    let output = Command::new("cursor")
        .arg("--version")
        .output()
        .map_err(|_| ComposerError::BridgeDisabled("cursor CLI not found on PATH".to_owned()))?;
    if !output.status.success() {
        return Err(ComposerError::BridgeDisabled(
            "cursor CLI failed version check".to_owned(),
        ));
    }
    Ok(())
}

fn ensure_cursor_auth() -> Result<(), ComposerError> {
    if std::env::var("CURSOR_API_KEY")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
    {
        return Ok(());
    }

    let output = Command::new("cursor")
        .args(["agent", "status"])
        .output()
        .map_err(|error| ComposerError::InvocationFailed(format!("cursor agent status failed: {error}")))?;

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if combined.to_ascii_lowercase().contains("not logged in") {
        return Err(ComposerError::AuthRequired);
    }
    Ok(())
}

fn wait_with_timeout(
    mut child: std::process::Child,
    timeout: Duration,
) -> Result<std::process::Output, ComposerError> {
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child.wait_with_output().map_err(|error| {
                    ComposerError::InvocationFailed(format!("cursor agent wait failed: {error}"))
                });
            }
            Ok(None) => {
                if started.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(ComposerError::Timeout);
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(error) => {
                return Err(ComposerError::InvocationFailed(format!(
                    "cursor agent process error: {error}"
                )));
            }
        }
    }
}

pub fn build_prompt(request: &ComposerRequest) -> String {
    let allowed_files = if request.allowed_files.is_empty() {
        "(none specified — propose minimal diff only)".to_owned()
    } else {
        request.allowed_files.join(", ")
    };

    let constraints = if request.constraints.is_empty() {
        "no dependency changes; no signing/notarization changes; repo-local edits only".to_owned()
    } else {
        request.constraints.join("; ")
    };

    let context = format_readonly_context(&request.readonly_context);

    format!(
        "You are Composer 2.5 operating as a bounded coding worker for AgentDeck overnight autonomy.\n\
         \n\
         MANDATORY RULES:\n\
         - Return patch text only. Do not execute shell commands or modify files directly.\n\
         - Output a unified diff patch and metadata in the exact format below.\n\
         - Touch only the allowed files list.\n\
         - Respect all constraints.\n\
         \n\
         Task ID: {task_id}\n\
         Instructions: {instructions}\n\
         Allowed files: {allowed_files}\n\
         Constraints: {constraints}\n\
         Read-only context:\n{context}\n\
         \n\
         Respond using this exact structure:\n\
         SUMMARY: <one line>\n\
         SUGGESTED_TESTS: <comma-separated list or none>\n\
         SUGGESTED_COMMANDS: <comma-separated list or none>\n\
         PATCH:\n\
         ```diff\n\
         <unified diff>\n\
         ```\n",
        task_id = request.task_id,
        instructions = request.instructions,
    )
}

fn format_readonly_context(context: &BTreeMap<String, String>) -> String {
    if context.is_empty() {
        return "  (none)".to_owned();
    }

    context
        .iter()
        .map(|(path, body)| format!("  --- {path} ---\n{body}"))
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn parse_cursor_response(raw: &str) -> Result<ComposerResponse, ComposerError> {
    let summary = extract_field(raw, "SUMMARY:")
        .unwrap_or_else(|| "Composer completed without explicit summary".to_owned());
    let suggested_tests = extract_list_field(raw, "SUGGESTED_TESTS:");
    let suggested_commands = extract_list_field(raw, "SUGGESTED_COMMANDS:");
    let patch_text = extract_patch(raw)?;

    if patch_text.trim().is_empty() {
        return Err(ComposerError::InvalidResponse(
            "cursor agent response did not include a unified diff patch".to_owned(),
        ));
    }

    Ok(ComposerResponse {
        patch_text,
        summary,
        suggested_tests,
        suggested_commands,
    })
}

fn extract_field(raw: &str, label: &str) -> Option<String> {
    raw.lines()
        .find_map(|line| line.strip_prefix(label).map(str::trim).map(str::to_owned))
}

fn extract_list_field(raw: &str, label: &str) -> Vec<String> {
    let Some(value) = extract_field(raw, label) else {
        return Vec::new();
    };
    if value.eq_ignore_ascii_case("none") {
        return Vec::new();
    }
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

pub fn extract_patch(raw: &str) -> Result<String, ComposerError> {
    if let Some(fenced) = extract_fenced_diff(raw) {
        return Ok(fenced);
    }

    if raw.contains("\n--- ") && raw.contains("\n+++ ") {
        return Ok(raw.trim().to_owned());
    }

    if let Some(patch_section) = raw.split("PATCH:").nth(1) {
        let trimmed = patch_section.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_owned());
        }
    }

    Err(ComposerError::InvalidResponse(
        "unable to locate unified diff in cursor agent response".to_owned(),
    ))
}

fn extract_fenced_diff(raw: &str) -> Option<String> {
    let mut lines = raw.lines();
    let mut captured = Vec::new();
    let mut in_fence = false;

    while let Some(line) = lines.next() {
        if line.trim().starts_with("```diff") {
            in_fence = true;
            continue;
        }
        if in_fence && line.trim() == "```" {
            break;
        }
        if in_fence {
            captured.push(line);
        }
    }

    if captured.is_empty() {
        None
    } else {
        Some(captured.join("\n"))
    }
}

pub fn apply_unified_patch(repo_root: &Path, patch_text: &str) -> Result<(), ComposerError> {
    let mut child = Command::new("git")
        .args(["apply", "--whitespace=nowarn", "-"])
        .current_dir(repo_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ComposerError::PatchApplyFailed(format!("git apply spawn failed: {error}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(patch_text.as_bytes())
            .map_err(|error| ComposerError::PatchApplyFailed(format!("git apply stdin failed: {error}")))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| ComposerError::PatchApplyFailed(format!("git apply wait failed: {error}")))?;

    if output.status.success() {
        return Ok(());
    }

    Err(ComposerError::PatchApplyFailed(
        String::from_utf8_lossy(&output.stderr).trim().to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autonomy::composer::PatchFormat;

    fn sample_request() -> ComposerRequest {
        ComposerRequest {
            task_id: "task-001".to_owned(),
            repo_root: "/tmp/agentdeck".to_owned(),
            instructions: "Add a comment".to_owned(),
            allowed_files: vec!["src-tauri/src/autonomy/policy.rs".to_owned()],
            readonly_context: BTreeMap::from([(
                "src-tauri/src/autonomy/policy.rs".to_owned(),
                "// sample".to_owned(),
            )]),
            constraints: vec!["no dependency changes".to_owned()],
            expected_patch_format: PatchFormat::UnifiedDiff,
        }
    }

    #[test]
    fn build_prompt_includes_allowed_files_and_constraints() {
        let prompt = build_prompt(&sample_request());
        assert!(prompt.contains("task-001"));
        assert!(prompt.contains("src-tauri/src/autonomy/policy.rs"));
        assert!(prompt.contains("no dependency changes"));
        assert!(prompt.contains("Return patch text only"));
    }

    #[test]
    fn parse_cursor_response_extracts_patch_and_metadata() {
        let raw = r#"SUMMARY: Added comment
SUGGESTED_TESTS: pnpm verify
SUGGESTED_COMMANDS: none
PATCH:
```diff
--- a/file.rs
+++ b/file.rs
@@
+// test
```"#;

        let response = parse_cursor_response(raw).expect("parse response");
        assert_eq!(response.summary, "Added comment");
        assert_eq!(response.suggested_tests, vec!["pnpm verify"]);
        assert!(response.patch_text.contains("--- a/file.rs"));
    }

    #[test]
    fn dry_run_bridge_returns_without_cursor() {
        let response =
            invoke_with_bridge_kind(&sample_request(), ComposerBridgeKind::DryRun).expect("dry-run ok");
        assert!(response.summary.contains("dry-run"));
    }

    #[test]
    fn extract_patch_requires_diff_content() {
        let error = extract_patch("SUMMARY: nothing here").expect_err("missing patch");
        assert!(matches!(error, ComposerError::InvalidResponse(_)));
    }
}