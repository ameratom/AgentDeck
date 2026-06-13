use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PolicyVerdict {
    Allow,
    AskFirst { reason: String },
    Deny { reason: String },
}

impl PolicyVerdict {
    pub fn is_allow(&self) -> bool {
        matches!(self, Self::Allow)
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Allow => "ALLOW",
            Self::AskFirst { .. } => "ASK_FIRST",
            Self::Deny { .. } => "DENY",
        }
    }
}

const ALLOW_COMMAND_PREFIXES: &[&str] = &[
    "pnpm typecheck",
    "pnpm lint",
    "pnpm test",
    "pnpm verify",
    "cargo check --manifest-path src-tauri/cargo.toml",
    "cargo test --manifest-path src-tauri/cargo.toml",
    "bash scripts/preflight.sh",
    "git status",
    "git diff",
    "git branch --show-current",
    "git log --oneline",
    "node --version",
    "pnpm --version",
    "cargo --version",
    "which ",
];

const DENY_PATTERNS: &[(&str, &str)] = &[
    ("rm -rf", "destructive filesystem: rm -rf"),
    ("rm -fr", "destructive filesystem: rm -fr"),
    ("git reset --hard", "dangerous git: reset --hard"),
    ("git clean -fd", "dangerous git: clean -fd"),
    ("git push --force", "dangerous git: push --force"),
    ("git push --mirror", "dangerous git: push --mirror"),
    ("git push", "overnight git push denied"),
    ("security find-generic-password", "credential access denied"),
    ("sudo ", "system control: sudo"),
    ("launchctl ", "system control: launchctl"),
    ("chmod -R", "destructive filesystem: chmod -R"),
    ("chown -R", "destructive filesystem: chown -R"),
    ("shred ", "destructive filesystem: shred"),
    ("find ", "-delete"),
    ("> ~/.zshrc", "config outside repo"),
    ("> ~/.bashrc", "config outside repo"),
    ("> ~/.profile", "config outside repo"),
    ("git remote add", "dangerous git: remote add"),
    ("git remote set-url", "dangerous git: remote set-url"),
    ("git remote remove", "dangerous git: remote remove"),
    ("killall ", "system control: killall"),
    ("pkill ", "system control: pkill"),
];

const ASK_FIRST_GIT_PREFIXES: &[&str] = &[
    "git commit",
    "git merge",
    "git rebase",
    "git cherry-pick",
    "git stash",
    "git reset",
    "git clean",
    "git tag",
];

const ASK_FIRST_COMMAND_HINTS: &[(&str, &str)] = &[
    ("pnpm install", "dependency change may modify lockfile"),
    ("pnpm add ", "dependency change"),
    ("pnpm remove ", "dependency change"),
    ("cargo add ", "dependency change"),
    ("curl ", "network behavior"),
    ("wget ", "network behavior"),
    ("npm install", "dependency change"),
];

fn normalize_command(command: &str) -> String {
    command.trim().to_ascii_lowercase()
}

fn matches_allow_prefix(command: &str) -> bool {
    ALLOW_COMMAND_PREFIXES
        .iter()
        .any(|prefix| command.starts_with(prefix))
}

fn matches_deny_pattern(command: &str) -> Option<(&'static str, &'static str)> {
    for (pattern, reason) in DENY_PATTERNS {
        if *pattern == "find " && command.contains("-delete") && command.contains("find ") {
            return Some((pattern, reason));
        }
        if command.contains(pattern) {
            return Some((pattern, reason));
        }
    }
    None
}

pub fn classify_shell_command(command: &str) -> PolicyVerdict {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return PolicyVerdict::Deny {
            reason: "empty command".to_owned(),
        };
    }

    let normalized = normalize_command(trimmed);

    if let Some((pattern, reason)) = matches_deny_pattern(&normalized) {
        return PolicyVerdict::Deny {
            reason: format!("blocked pattern `{pattern}`: {reason}"),
        };
    }

    for prefix in ASK_FIRST_GIT_PREFIXES {
        if normalized.starts_with(prefix) {
            return PolicyVerdict::AskFirst {
                reason: format!("git operation requires approval: {prefix}"),
            };
        }
    }

    for (hint, reason) in ASK_FIRST_COMMAND_HINTS {
        if normalized.contains(hint) {
            return PolicyVerdict::AskFirst {
                reason: (*reason).to_owned(),
            };
        }
    }

    if matches_allow_prefix(&normalized) {
        return PolicyVerdict::Allow;
    }

    PolicyVerdict::AskFirst {
        reason: "command not on overnight allowlist".to_owned(),
    }
}

pub fn classify_branch_name(branch: &str) -> PolicyVerdict {
    let allowed_prefixes = ["hermes/", "agentdeck/hermes/", "scratch/hermes/"];
    if allowed_prefixes.iter().any(|prefix| branch.starts_with(prefix)) {
        PolicyVerdict::Allow
    } else {
        PolicyVerdict::AskFirst {
            reason: format!("branch `{branch}` is not a hermes scratch branch"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_safe_verify_commands() {
        assert_eq!(
            classify_shell_command("pnpm typecheck"),
            PolicyVerdict::Allow
        );
        assert_eq!(
            classify_shell_command("cargo test --manifest-path src-tauri/Cargo.toml"),
            PolicyVerdict::Allow
        );
        assert_eq!(
            classify_shell_command("bash scripts/preflight.sh"),
            PolicyVerdict::Allow
        );
    }

    #[test]
    fn denies_rm_rf() {
        let verdict = classify_shell_command("rm -rf /tmp/agentdeck-test");
        assert!(matches!(verdict, PolicyVerdict::Deny { .. }));
    }

    #[test]
    fn denies_git_push_force() {
        let verdict = classify_shell_command("git push --force origin main");
        assert!(matches!(verdict, PolicyVerdict::Deny { .. }));
    }

    #[test]
    fn denies_security_keychain_lookup() {
        let verdict =
            classify_shell_command("security find-generic-password -s agentdeck -w");
        assert!(matches!(verdict, PolicyVerdict::Deny { .. }));
    }

    #[test]
    fn asks_first_for_dependency_install() {
        let verdict = classify_shell_command("pnpm install");
        assert!(matches!(verdict, PolicyVerdict::AskFirst { .. }));
    }

    #[test]
    fn asks_first_for_unknown_commands() {
        let verdict = classify_shell_command("make release");
        assert!(matches!(verdict, PolicyVerdict::AskFirst { .. }));
    }

    #[test]
    fn denies_empty_command_fail_closed() {
        let verdict = classify_shell_command("   ");
        assert!(matches!(verdict, PolicyVerdict::Deny { .. }));
    }

    #[test]
    fn allows_hermes_scratch_branches() {
        assert_eq!(
            classify_branch_name("hermes/overnight-20260613"),
            PolicyVerdict::Allow
        );
    }
}