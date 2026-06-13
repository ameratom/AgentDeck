use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::composer_bridge;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComposerRequest {
    pub task_id: String,
    pub repo_root: String,
    pub instructions: String,
    pub allowed_files: Vec<String>,
    pub readonly_context: BTreeMap<String, String>,
    pub constraints: Vec<String>,
    pub expected_patch_format: PatchFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchFormat {
    UnifiedDiff,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComposerResponse {
    pub patch_text: String,
    pub summary: String,
    pub suggested_tests: Vec<String>,
    pub suggested_commands: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposerError {
    InvalidRequest(String),
    BridgeDisabled(String),
    AuthRequired,
    InvocationFailed(String),
    InvalidResponse(String),
    Timeout,
    PatchApplyFailed(String),
}

impl std::fmt::Display for ComposerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRequest(message) => write!(formatter, "invalid composer request: {message}"),
            Self::BridgeDisabled(message) => write!(formatter, "composer bridge disabled: {message}"),
            Self::AuthRequired => write!(
                formatter,
                "cursor agent authentication required; set CURSOR_API_KEY or run `cursor agent login`"
            ),
            Self::InvocationFailed(message) => write!(formatter, "composer invocation failed: {message}"),
            Self::InvalidResponse(message) => write!(formatter, "invalid composer response: {message}"),
            Self::Timeout => write!(formatter, "composer invocation timed out"),
            Self::PatchApplyFailed(message) => write!(formatter, "patch apply failed: {message}"),
        }
    }
}

impl std::error::Error for ComposerError {}

/// Bounded Hermes → Composer seam via the Cursor Agent CLI (`cursor agent --print --mode plan`).
pub fn invoke_composer(request: &ComposerRequest) -> Result<ComposerResponse, ComposerError> {
    if request.task_id.trim().is_empty() {
        return Err(ComposerError::InvalidRequest("task_id is required".to_owned()));
    }
    if request.instructions.trim().is_empty() {
        return Err(ComposerError::InvalidRequest(
            "instructions are required".to_owned(),
        ));
    }

    composer_bridge::invoke_with_bridge(request)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invoke_composer_delegates_to_bridge() {
        let request = ComposerRequest {
            task_id: "task-001".to_owned(),
            repo_root: "/tmp/agentdeck".to_owned(),
            instructions: "Add a test".to_owned(),
            allowed_files: vec!["src-tauri/src/autonomy/policy.rs".to_owned()],
            readonly_context: Default::default(),
            constraints: vec!["no dependency changes".to_owned()],
            expected_patch_format: PatchFormat::UnifiedDiff,
        };

        let response = composer_bridge::invoke_with_bridge_kind(
            &request,
            composer_bridge::ComposerBridgeKind::DryRun,
        )
        .expect("dry-run composer");
        assert!(response.summary.contains("dry-run"));
    }
}