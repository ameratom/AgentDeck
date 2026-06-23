use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Local;
use serde::{Deserialize, Serialize};

use super::command_runner::{run_guarded, AuditEntry, GuardOutcome};
use super::composer::{invoke_composer, ComposerRequest, ComposerResponse, PatchFormat};
use super::composer_bridge::apply_unified_patch;
use super::policy::{self, PolicyVerdict};

pub const DEFAULT_RETRY_CAP: u32 = 2;
const MAX_CONTEXT_FILE_BYTES: usize = 64 * 1024;
const MAX_CONTEXT_TOTAL_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OvernightTask {
    pub id: String,
    pub title: String,
    pub scope: String,
    pub files_hint: Vec<String>,
    pub acceptance: Vec<String>,
    #[serde(default)]
    pub deferred_if: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub title: String,
    pub status: TaskStatus,
    pub retries_used: u32,
    pub composer_blocked: bool,
    pub verify_passed: Option<bool>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Completed,
    Blocked,
    Deferred,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OvernightReport {
    pub branch: String,
    pub commit_status: String,
    pub tasks_attempted: usize,
    pub tasks_completed: usize,
    pub tasks_blocked: usize,
    pub files_changed: Vec<String>,
    pub commands_run: Vec<String>,
    pub verification_result: String,
    pub test_failures: Vec<String>,
    pub denied_actions: Vec<String>,
    pub approval_needed_actions: Vec<String>,
    pub composer_calls: Vec<String>,
    pub patch_summaries: Vec<String>,
    pub known_limitations: Vec<String>,
    pub recommended_next_human_review: Vec<String>,
    pub audit_entries: Vec<AuditEntry>,
    pub task_results: Vec<TaskResult>,
}

pub fn overnight_branch_name() -> String {
    let date = Local::now().format("%Y%m%d");
    format!("hermes/overnight-{date}")
}

pub fn load_queue(path: &Path) -> Result<Vec<OvernightTask>, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("failed to read queue {}: {error}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|error| format!("failed to parse queue {}: {error}", path.display()))
}

pub fn validate_task_scope(task: &OvernightTask) -> PolicyVerdict {
    match task.scope.to_ascii_uppercase().as_str() {
        "ALLOW" => PolicyVerdict::Allow,
        "ASK_FIRST" => PolicyVerdict::AskFirst {
            reason: format!("task `{}` is ASK_FIRST and cannot run overnight", task.id),
        },
        "DENY" => PolicyVerdict::Deny {
            reason: format!("task `{}` is DENY and cannot run overnight", task.id),
        },
        _ => PolicyVerdict::AskFirst {
            reason: format!("task `{}` has unknown scope `{}`", task.id, task.scope),
        },
    }
}

fn run_git(repo_root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .map_err(|error| format!("git command failed: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn ensure_scratch_branch(repo_root: &Path, branch: &str) -> Result<(), String> {
    match policy::classify_branch_name(branch) {
        PolicyVerdict::Allow => {}
        other => return Err(format!("branch not allowed: {other:?}")),
    }

    let current = run_git(repo_root, &["branch", "--show-current"])?;
    if current == branch {
        return Ok(());
    }

    if run_git(repo_root, &["branch", "--list", branch])?
        .lines()
        .any(|line| line.trim() == branch)
    {
        run_git(repo_root, &["checkout", branch])?;
        return Ok(());
    }

    run_git(repo_root, &["checkout", "-b", branch])?;
    Ok(())
}

pub fn list_changed_files(repo_root: &Path) -> Result<Vec<String>, String> {
    let output = run_git(repo_root, &["status", "--porcelain"])?;
    Ok(output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.len() < 4 {
                return None;
            }
            Some(trimmed[3..].trim().to_string())
        })
        .collect())
}

fn guarded(repo_root: &Path, command: &str, execute: bool, audit: &mut Vec<AuditEntry>) -> GuardOutcome {
    let _ = repo_root;
    let outcome = run_guarded(command, execute);
    audit.push(outcome.audit.clone());
    outcome
}

pub fn run_overnight(repo_root: &Path, queue_path: &Path, execute_verify: bool) -> OvernightReport {
    let branch = overnight_branch_name();
    let mut audit_entries = Vec::new();
    let mut commands_run = Vec::new();
    let mut denied_actions = Vec::new();
    let mut approval_needed_actions = Vec::new();
    let mut composer_calls = Vec::new();
    let mut patch_summaries = Vec::new();
    let mut task_results = Vec::new();
    let mut test_failures = Vec::new();
    let known_limitations = vec![
        "Composer bridge uses `cursor agent --print` with AGENTDECK_COMPOSER_CURSOR_MODE (default plan; agent omits --mode for patch output); suggested commands are never auto-executed.".to_owned(),
        "Overnight loop does not commit or push.".to_owned(),
    ];

    let queue = match load_queue(queue_path) {
        Ok(tasks) => tasks,
        Err(error) => {
            return empty_report_with_error(branch, error, audit_entries);
        }
    };

    if let Err(error) = ensure_scratch_branch(repo_root, &branch) {
        return empty_report_with_error(branch, error, audit_entries);
    }

    for task in &queue {
        let scope = validate_task_scope(task);
        if !scope.is_allow() {
            let message = format!("{scope:?}");
            if matches!(scope, PolicyVerdict::Deny { .. }) {
                denied_actions.push(message.clone());
            } else {
                approval_needed_actions.push(message.clone());
            }
            task_results.push(TaskResult {
                task_id: task.id.clone(),
                title: task.title.clone(),
                status: TaskStatus::Blocked,
                retries_used: 0,
                composer_blocked: false,
                verify_passed: None,
                notes: vec![message],
            });
            continue;
        }

        let readonly_context = load_readonly_context(repo_root, &task.files_hint);

        let request = ComposerRequest {
            task_id: task.id.clone(),
            repo_root: repo_root.display().to_string(),
            instructions: task.title.clone(),
            allowed_files: task.files_hint.clone(),
            readonly_context,
            constraints: task
                .deferred_if
                .clone()
                .map(|value| vec![value])
                .unwrap_or_default(),
            expected_patch_format: PatchFormat::UnifiedDiff,
        };

        let composer_result = invoke_composer(&request);
        let mut retries = 0_u32;
        let mut notes = Vec::new();

        let composer_response = match composer_result {
            Ok(response) => {
                composer_calls.push(format!("{} -> ok: {}", task.id, response.summary));
                response
            }
            Err(error) => {
                composer_calls.push(format!("{} -> error: {error}", task.id));
                notes.push(format!("Composer invocation failed: {error}"));
                task_results.push(TaskResult {
                    task_id: task.id.clone(),
                    title: task.title.clone(),
                    status: TaskStatus::Blocked,
                    retries_used: retries,
                    composer_blocked: true,
                    verify_passed: None,
                    notes,
                });
                continue;
            }
        };

        if let Err(error) = apply_composer_patch(repo_root, &composer_response, &mut notes, &mut patch_summaries, &task.id) {
            notes.push(error);
            task_results.push(TaskResult {
                task_id: task.id.clone(),
                title: task.title.clone(),
                status: TaskStatus::Blocked,
                retries_used: retries,
                composer_blocked: false,
                verify_passed: None,
                notes,
            });
            continue;
        }

        for suggested in &composer_response.suggested_commands {
            approval_needed_actions.push(format!("{} suggested command: {suggested}", task.id));
        }

        let mut verify_passed = None;
        while retries <= DEFAULT_RETRY_CAP {
            let outcome = guarded(repo_root, "pnpm verify", execute_verify, &mut audit_entries);
            commands_run.push(outcome.audit.command_shape.clone());

            match outcome.verdict {
                PolicyVerdict::Allow => {
                    if execute_verify {
                        let success = outcome
                            .output
                            .as_ref()
                            .map(|output| output.exit_code == 0)
                            .unwrap_or(false);
                        verify_passed = Some(success);
                        if success {
                            patch_summaries.push(format!("{}: verify passed", task.id));
                            break;
                        }
                        test_failures.push(format!("{}: pnpm verify failed", task.id));
                    } else {
                        verify_passed = Some(true);
                        patch_summaries.push(format!("{}: dry-run verify allowed", task.id));
                        break;
                    }
                }
                PolicyVerdict::Deny { reason } => {
                    denied_actions.push(reason.clone());
                    notes.push(reason);
                    break;
                }
                PolicyVerdict::AskFirst { reason } => {
                    approval_needed_actions.push(reason.clone());
                    notes.push(reason);
                    break;
                }
            }

            retries += 1;
        }

        let status = if verify_passed == Some(true) {
            TaskStatus::Completed
        } else if retries > DEFAULT_RETRY_CAP {
            TaskStatus::Failed
        } else {
            TaskStatus::Blocked
        };

        task_results.push(TaskResult {
            task_id: task.id.clone(),
            title: task.title.clone(),
            status,
            retries_used: retries,
            composer_blocked: false,
            verify_passed,
            notes,
        });
    }

    let files_changed = list_changed_files(repo_root).unwrap_or_default();
    let tasks_completed = task_results
        .iter()
        .filter(|result| result.status == TaskStatus::Completed)
        .count();
    let tasks_blocked = task_results
        .iter()
        .filter(|result| matches!(result.status, TaskStatus::Blocked | TaskStatus::Failed))
        .count();

    let verification_result = if test_failures.is_empty() {
        "pnpm verify allowed or passed for attempted tasks".to_owned()
    } else {
        format!("verify failures: {}", test_failures.join("; "))
    };

    OvernightReport {
        branch,
        commit_status: "no commits created by overnight loop".to_owned(),
        tasks_attempted: task_results.len(),
        tasks_completed,
        tasks_blocked,
        files_changed,
        commands_run,
        verification_result,
        test_failures,
        denied_actions,
        approval_needed_actions,
        composer_calls,
        patch_summaries,
        known_limitations,
        recommended_next_human_review: vec![
            "Review scratch branch diff and run pnpm verify manually before merge.".to_owned(),
            "Approve or reject any Composer suggested commands before running them.".to_owned(),
        ],
        audit_entries,
        task_results,
    }
}

fn empty_report_with_error(
    branch: String,
    error: String,
    audit_entries: Vec<AuditEntry>,
) -> OvernightReport {
    OvernightReport {
        branch,
        commit_status: "no commits created by overnight loop".to_owned(),
        tasks_attempted: 0,
        tasks_completed: 0,
        tasks_blocked: 0,
        files_changed: Vec::new(),
        commands_run: Vec::new(),
        verification_result: error.clone(),
        test_failures: vec![error.clone()],
        denied_actions: Vec::new(),
        approval_needed_actions: Vec::new(),
        composer_calls: Vec::new(),
        patch_summaries: Vec::new(),
        known_limitations: vec!["Overnight loop aborted before task execution.".to_owned()],
        recommended_next_human_review: vec!["Fix queue/branch setup and re-run.".to_owned()],
        audit_entries,
        task_results: Vec::new(),
    }
}

pub fn render_report_markdown(report: &OvernightReport) -> String {
    let mut lines = vec![
        "# AgentDeck Overnight Report".to_owned(),
        String::new(),
        format!("Branch: {}", report.branch),
        format!("Commit status: {}", report.commit_status),
        format!("Tasks attempted: {}", report.tasks_attempted),
        format!("Tasks completed: {}", report.tasks_completed),
        format!("Tasks blocked: {}", report.tasks_blocked),
        String::new(),
        "## Files changed".to_owned(),
    ];

    if report.files_changed.is_empty() {
        lines.push("- (none)".to_owned());
    } else {
        for file in &report.files_changed {
            lines.push(format!("- {file}"));
        }
    }

    lines.push(String::new());
    lines.push("## Commands run".to_owned());
    if report.commands_run.is_empty() {
        lines.push("- (none)".to_owned());
    } else {
        for command in &report.commands_run {
            lines.push(format!("- `{command}`"));
        }
    }

    lines.push(String::new());
    lines.push(format!(
        "## Verification result\n\n{}",
        report.verification_result
    ));

    append_list_section(&mut lines, "Test failures", &report.test_failures);
    append_list_section(&mut lines, "Denied actions", &report.denied_actions);
    append_list_section(
        &mut lines,
        "Approval-needed actions",
        &report.approval_needed_actions,
    );
    append_list_section(&mut lines, "Composer calls", &report.composer_calls);
    append_list_section(&mut lines, "Patch summaries", &report.patch_summaries);
    append_list_section(&mut lines, "Known limitations", &report.known_limitations);
    append_list_section(
        &mut lines,
        "Recommended next human review",
        &report.recommended_next_human_review,
    );

    lines.push(String::new());
    lines.push("## Task results".to_owned());
    for result in &report.task_results {
        lines.push(format!(
            "- {} ({:?}) retries={} composer_blocked={} verify={:?}",
            result.task_id, result.status, result.retries_used, result.composer_blocked, result.verify_passed
        ));
    }

    lines.join("\n")
}

pub fn load_readonly_context(repo_root: &Path, files_hint: &[String]) -> BTreeMap<String, String> {
    let mut context = BTreeMap::new();
    let Ok(repo_root) = repo_root.canonicalize() else {
        return context;
    };

    let mut total_bytes = 0_usize;
    for hint in files_hint {
        let candidate = repo_root.join(hint);
        let Ok(canonical) = candidate.canonicalize() else {
            context.insert(hint.clone(), "(unavailable)".to_owned());
            continue;
        };
        if !canonical.starts_with(&repo_root) {
            context.insert(hint.clone(), "(outside repo)".to_owned());
            continue;
        }

        if canonical.is_file() {
            if let Some(body) = read_bounded_file(&canonical, &mut total_bytes) {
                context.insert(hint.clone(), body);
            }
            continue;
        }

        if canonical.is_dir() {
            context.insert(hint.clone(), summarize_directory(&canonical));
        }
    }

    context
}

fn read_bounded_file(path: &Path, total_bytes: &mut usize) -> Option<String> {
    let metadata = fs::metadata(path).ok()?;
    if metadata.len() as usize > MAX_CONTEXT_FILE_BYTES {
        return Some(format!(
            "(file too large: {} bytes, cap {MAX_CONTEXT_FILE_BYTES})",
            metadata.len()
        ));
    }
    if *total_bytes >= MAX_CONTEXT_TOTAL_BYTES {
        return Some("(context budget exceeded)".to_owned());
    }

    let bytes = fs::read(path).ok()?;
    let take = bytes
        .len()
        .min(MAX_CONTEXT_FILE_BYTES)
        .min(MAX_CONTEXT_TOTAL_BYTES.saturating_sub(*total_bytes));
    *total_bytes += take;
    Some(String::from_utf8_lossy(&bytes[..take]).to_string())
}

fn summarize_directory(path: &Path) -> String {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) => return format!("(directory unreadable: {error})"),
    };

    let names = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .take(20)
        .collect::<Vec<_>>();

    if names.is_empty() {
        "(empty directory)".to_owned()
    } else {
        format!("(directory listing) {}", names.join(", "))
    }
}

fn apply_composer_patch(
    repo_root: &Path,
    response: &ComposerResponse,
    notes: &mut Vec<String>,
    patch_summaries: &mut Vec<String>,
    task_id: &str,
) -> Result<(), String> {
    patch_summaries.push(format!("{task_id}: {}", response.summary));

    if response.patch_text.trim().is_empty() {
        notes.push("Composer returned no patch; continuing without file changes.".to_owned());
        return Ok(());
    }

    apply_unified_patch(repo_root, &response.patch_text)
        .map_err(|error| format!("patch apply failed: {error}"))?;
    notes.push("Composer patch applied via git apply.".to_owned());
    Ok(())
}

fn append_list_section(lines: &mut Vec<String>, title: &str, items: &[String]) {
    lines.push(String::new());
    lines.push(format!("## {title}"));
    if items.is_empty() {
        lines.push("- (none)".to_owned());
    } else {
        for item in items {
            lines.push(format!("- {item}"));
        }
    }
}

pub fn write_report(repo_root: &Path, report: &OvernightReport) -> Result<PathBuf, String> {
    let reports_dir = repo_root.join("tasks/reports");
    fs::create_dir_all(&reports_dir)
        .map_err(|error| format!("failed to create reports dir: {error}"))?;
    let date = Local::now().format("%Y%m%d");
    let path = reports_dir.join(format!("overnight-{date}.md"));
    fs::write(&path, render_report_markdown(report))
        .map_err(|error| format!("failed to write report {}: {error}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_allow_task_scope() {
        let task = OvernightTask {
            id: "task-deny".to_owned(),
            title: "Dangerous".to_owned(),
            scope: "DENY".to_owned(),
            files_hint: vec![],
            acceptance: vec![],
            deferred_if: None,
        };
        assert!(!validate_task_scope(&task).is_allow());
    }

    #[test]
    fn overnight_branch_matches_policy() {
        let branch = overnight_branch_name();
        assert!(policy::classify_branch_name(&branch).is_allow());
        assert!(branch.starts_with("hermes/overnight-"));
    }

    #[test]
    fn load_readonly_context_reads_repo_files() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("repo root");
        let context = load_readonly_context(repo_root, &["AGENTS.md".to_owned()]);
        assert!(context.get("AGENTS.md").unwrap().contains("AgentDeck"));
    }

    #[test]
    fn load_readonly_context_rejects_paths_outside_repo() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("repo root");
        let context = load_readonly_context(repo_root, &["/etc/passwd".to_owned()]);
        assert_eq!(
            context.get("/etc/passwd").map(String::as_str),
            Some("(outside repo)")
        );
    }
}