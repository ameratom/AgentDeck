use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::policy::{self, PolicyVerdict};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp_ms: u64,
    pub command_shape: String,
    pub verdict: String,
    pub reason: String,
    pub executed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuardOutcome {
    pub verdict: PolicyVerdict,
    pub audit: AuditEntry,
    pub output: Option<CommandOutput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn command_shape(command: &str) -> String {
    let trimmed = command.trim();
    if trimmed.len() <= 240 {
        return trimmed.to_owned();
    }
    format!("{}…", &trimmed[..240])
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn build_audit(command: &str, verdict: &PolicyVerdict, executed: bool) -> AuditEntry {
    let reason = match verdict {
        PolicyVerdict::Allow => "allowed".to_owned(),
        PolicyVerdict::AskFirst { reason } | PolicyVerdict::Deny { reason } => reason.clone(),
    };

    AuditEntry {
        timestamp_ms: now_ms(),
        command_shape: command_shape(command),
        verdict: verdict.label().to_owned(),
        reason,
        executed,
    }
}

fn execute_allowed(command: &str) -> Result<CommandOutput, String> {
    let output = if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", command]).output()
    } else {
        Command::new("sh").arg("-lc").arg(command).output()
    }
    .map_err(|error| format!("command runner failed to spawn process: {error}"))?;

    Ok(map_output(output))
}

fn map_output(output: Output) -> CommandOutput {
    CommandOutput {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    }
}

/// Single chokepoint for shell execution in the overnight loop.
/// Fails closed: DENY and ASK_FIRST never execute. Runner errors become DENY.
pub fn run_guarded(command: &str, execute: bool) -> GuardOutcome {
    let verdict = match std::panic::catch_unwind(|| policy::classify_shell_command(command)) {
        Ok(verdict) => verdict,
        Err(_) => PolicyVerdict::Deny {
            reason: "classification panic; fail closed".to_owned(),
        },
    };

    match &verdict {
        PolicyVerdict::Allow if execute => match execute_allowed(command) {
            Ok(output) => GuardOutcome {
                verdict: PolicyVerdict::Allow,
                audit: build_audit(command, &PolicyVerdict::Allow, true),
                output: Some(output),
            },
            Err(error) => {
                let deny = PolicyVerdict::Deny { reason: error };
                GuardOutcome {
                    verdict: deny.clone(),
                    audit: build_audit(command, &deny, false),
                    output: None,
                }
            }
        },
        PolicyVerdict::Allow => GuardOutcome {
            verdict: PolicyVerdict::Allow,
            audit: build_audit(command, &PolicyVerdict::Allow, false),
            output: None,
        },
        PolicyVerdict::AskFirst { .. } | PolicyVerdict::Deny { .. } => GuardOutcome {
            verdict: verdict.clone(),
            audit: build_audit(command, &verdict, false),
            output: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_rm_rf_without_execution() {
        let outcome = run_guarded("rm -rf /tmp/agentdeck-deny-test", true);
        assert!(matches!(outcome.verdict, PolicyVerdict::Deny { .. }));
        assert!(!outcome.audit.executed);
        assert_eq!(outcome.audit.verdict, "DENY");
    }

    #[test]
    fn blocks_git_push_force_without_execution() {
        let outcome = run_guarded("git push --force origin main", true);
        assert!(matches!(outcome.verdict, PolicyVerdict::Deny { .. }));
        assert!(!outcome.audit.executed);
    }

    #[test]
    fn blocks_security_find_generic_password() {
        let outcome = run_guarded("security find-generic-password -s test", true);
        assert!(matches!(outcome.verdict, PolicyVerdict::Deny { .. }));
        assert!(!outcome.audit.executed);
    }

    #[test]
    fn dry_run_allow_does_not_execute() {
        let outcome = run_guarded("pnpm typecheck", false);
        assert!(matches!(outcome.verdict, PolicyVerdict::Allow));
        assert!(!outcome.audit.executed);
        assert!(outcome.output.is_none());
    }

    #[test]
    fn audit_entry_uses_command_shape_not_secrets() {
        let outcome = run_guarded("git status", false);
        assert_eq!(outcome.audit.command_shape, "git status");
    }

    #[test]
    fn runner_errors_fail_closed_as_deny() {
        let outcome = run_guarded("", true);
        assert!(matches!(outcome.verdict, PolicyVerdict::Deny { .. }));
        assert!(!outcome.audit.executed);
    }
}