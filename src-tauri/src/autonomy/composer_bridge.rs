use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::Value;

use super::composer::{ComposerError, ComposerRequest, ComposerResponse};

const DEFAULT_MODEL: &str = "composer-2.5-fast";
const DEFAULT_TIMEOUT_SECS: u64 = 600;
const DEFAULT_CURSOR_MODE: &str = "plan";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposerBridgeKind {
    CursorAgent,
    DryRun,
}

pub fn cursor_mode_from_env() -> String {
    let mode = std::env::var("AGENTDECK_COMPOSER_CURSOR_MODE")
        .unwrap_or_else(|_| DEFAULT_CURSOR_MODE.to_owned())
        .trim()
        .to_ascii_lowercase();

    match mode.as_str() {
        "plan" | "agent" => mode,
        other => {
            eprintln!(
                "AGENTDECK_COMPOSER_CURSOR_MODE={other} is unsupported; using {DEFAULT_CURSOR_MODE}"
            );
            DEFAULT_CURSOR_MODE.to_owned()
        }
    }
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

    let cursor_mode = cursor_mode_from_env();
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
        .arg(&cursor_mode)
        .arg("--workspace")
        .arg(repo_root)
        .arg("--model")
        .arg(&model)
        .arg("--output-format")
        .arg("json")
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

pub fn unwrap_cursor_payload(raw: &str) -> String {
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') {
        return trimmed.to_owned();
    }

    let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
        return trimmed.to_owned();
    };

    value
        .get("result")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| trimmed.to_owned())
}

pub fn parse_cursor_response(raw: &str) -> Result<ComposerResponse, ComposerError> {
    let payload = unwrap_cursor_payload(raw);
    let summary = extract_field(&payload, "SUMMARY:")
        .unwrap_or_else(|| "Composer completed without explicit summary".to_owned());
    let suggested_tests = extract_list_field(&payload, "SUGGESTED_TESTS:");
    let suggested_commands = extract_list_field(&payload, "SUGGESTED_COMMANDS:");
    let patch_text = extract_patch(&payload)?;

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

    if let Some(fenced) = extract_fenced_patch_blocks(raw).into_iter().find(|block| looks_like_unified_diff(block)) {
        return Ok(fenced);
    }

    if let Some(patch) = extract_inline_unified_diff(raw) {
        return Ok(patch);
    }

    if let Some(patch_section) = raw.split("PATCH:").nth(1) {
        let trimmed = strip_code_fence(patch_section.trim());
        if looks_like_unified_diff(&trimmed) {
            return Ok(trimmed);
        }
    }

    Err(ComposerError::InvalidResponse(
        "unable to locate unified diff in cursor agent response".to_owned(),
    ))
}

pub fn looks_like_unified_diff(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with("diff --git ") {
        return true;
    }
    let has_old = trimmed.lines().any(|line| line.starts_with("--- "));
    let has_new = trimmed.lines().any(|line| line.starts_with("+++ "));
    let has_hunk = trimmed.lines().any(|line| line.starts_with("@@ "));
    (has_old && has_new) || (has_old && has_hunk) || (has_new && has_hunk)
}

fn extract_inline_unified_diff(raw: &str) -> Option<String> {
    let lines: Vec<&str> = raw.lines().collect();
    let start = lines.iter().position(|line| {
        line.starts_with("--- ")
            || line.starts_with("+++ ")
            || line.starts_with("diff --git ")
            || line.starts_with("@@ ")
    })?;

    let mut end = lines.len();
    for index in (start + 1)..lines.len() {
        let line = lines[index];
        if line.trim().is_empty() {
            continue;
        }
        if line.starts_with("SUMMARY:")
            || line.starts_with("SUGGESTED_TESTS:")
            || line.starts_with("SUGGESTED_COMMANDS:")
            || line.starts_with("```")
        {
            end = index;
            break;
        }
    }

    let patch = lines[start..end].join("\n").trim().to_owned();
    if looks_like_unified_diff(&patch) {
        Some(patch)
    } else {
        None
    }
}

fn extract_fenced_diff(raw: &str) -> Option<String> {
    extract_fenced_patch_blocks(raw)
        .into_iter()
        .find(|block| looks_like_unified_diff(block))
}

fn extract_fenced_patch_blocks(raw: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut captured = Vec::new();
    let mut in_fence = false;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            if in_fence {
                let block = captured.join("\n");
                if !block.trim().is_empty() {
                    blocks.push(block);
                }
                captured.clear();
                in_fence = false;
                continue;
            }
            in_fence = true;
            continue;
        }
        if in_fence {
            captured.push(line);
        }
    }

    if in_fence && !captured.is_empty() {
        blocks.push(captured.join("\n"));
    }

    blocks
}

fn strip_code_fence(text: &str) -> String {
    let mut trimmed = text.trim();
    if let Some(inner) = trimmed.strip_prefix("```") {
        trimmed = inner.trim_start();
        if let Some(rest) = trimmed.strip_prefix("diff") {
            trimmed = rest.trim_start();
        }
        if let Some(rest) = trimmed.strip_suffix("```") {
            trimmed = rest.trim();
        }
    }
    trimmed.to_owned()
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
    fn cursor_mode_from_env_defaults_to_plan() {
        let previous = std::env::var("AGENTDECK_COMPOSER_CURSOR_MODE").ok();
        std::env::remove_var("AGENTDECK_COMPOSER_CURSOR_MODE");
        assert_eq!(cursor_mode_from_env(), "plan");
        std::env::set_var("AGENTDECK_COMPOSER_CURSOR_MODE", "agent");
        assert_eq!(cursor_mode_from_env(), "agent");
        match previous {
            Some(value) => std::env::set_var("AGENTDECK_COMPOSER_CURSOR_MODE", value),
            None => std::env::remove_var("AGENTDECK_COMPOSER_CURSOR_MODE"),
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

    #[test]
    fn extract_patch_accepts_plain_fence_without_diff_language() {
        let raw = r#"Here is the patch:
```
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1 +1 @@
-old
+new
```"#;
        let patch = extract_patch(raw).expect("plain fence patch");
        assert!(patch.contains("--- a/src/lib.rs"));
        assert!(looks_like_unified_diff(&patch));
    }

    #[test]
    fn extract_patch_accepts_inline_unified_diff_without_patch_header() {
        let raw = r#"SUMMARY: inline diff
--- a/file.rs
+++ b/file.rs
@@ -0,0 +1 @@
+// added
SUGGESTED_TESTS: none"#;
        let patch = extract_patch(raw).expect("inline patch");
        assert!(patch.contains("+++ b/file.rs"));
    }

    #[test]
    fn extract_patch_accepts_diff_git_format() {
        let raw = r#"PATCH:
```diff
diff --git a/foo.rs b/foo.rs
index 111..222 100644
--- a/foo.rs
+++ b/foo.rs
@@ -1 +1 @@
-old
+new
```"#;
        let patch = extract_patch(raw).expect("git diff patch");
        assert!(patch.starts_with("diff --git"));
    }

    #[test]
    fn looks_like_unified_diff_rejects_prose() {
        assert!(!looks_like_unified_diff("SUMMARY: no diff here"));
    }

    #[test]
    fn unwrap_cursor_payload_extracts_result_field_from_json_envelope() {
        let raw = r#"{"type":"result","subtype":"success","is_error":false,"result":"SUMMARY: Added test\nPATCH:\n```diff\n--- a/file.rs\n+++ b/file.rs\n@@\n+// test\n```"}"#;
        let payload = unwrap_cursor_payload(raw);
        assert!(payload.contains("SUMMARY: Added test"));
        let response = parse_cursor_response(raw).expect("json envelope response");
        assert_eq!(response.summary, "Added test");
        assert!(response.patch_text.contains("--- a/file.rs"));
    }

    #[test]
    fn extract_patch_accepts_patch_fence_language_tag() {
        let raw = r#"PATCH:
```patch
--- a/foo.rs
+++ b/foo.rs
@@ -1 +1 @@
-old
+new
```"#;
        let patch = extract_patch(raw).expect("patch fence");
        assert!(patch.contains("+++ b/foo.rs"));
    }
}
