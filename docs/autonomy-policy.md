# AgentDeck Autonomy Policy

Single source of truth for Hermes overnight autonomy: ALLOW / ASK_FIRST / DENY classification and the command-runner deny list.

Mirrored in code: `src-tauri/src/autonomy/policy.rs` and `src-tauri/src/autonomy/command_runner.rs`.

## Authority order

1. User explicit approval
2. AgentDeck project rules (local-first, no secret leakage, audit logging)
3. Hermes policy engine (`classify_shell_command`, `classify_branch_name`)
4. Command runner (`run_guarded`) — mechanical deny patterns
5. Composer suggestions — never final authority

## ALLOW — Hermes may do without asking

### Reading / inspection

- Read any file inside the AgentDeck repo
- `git status`, `git diff`, `git branch --show-current`, `git log --oneline`
- Non-destructive discovery: `which pnpm`, `node --version`, `cargo --version`

### Source edits (repo only)

- Edit application source, tests, docs, and non-destructive scripts
- Add fixtures, test helpers, verification scripts

### Safe commands

- `pnpm typecheck`, `pnpm lint`, `pnpm test`, `pnpm verify`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml`
- `bash scripts/preflight.sh`

### Scratch branches

Hermes may create and work on branches matching:

- `hermes/*`
- `agentdeck/hermes/*`
- `scratch/hermes/*`

No commit, push, merge, rebase, or history rewrite on these branches during overnight runs.

## ASK_FIRST — stop and request approval

| Category | Examples |
|----------|----------|
| Dependency changes | `pnpm install`, `pnpm add`, `cargo add`, lockfile changes |
| Architecture / product | New provider abstractions, schema redesign, cloud sync |
| Tauri capability | Permissions, capabilities, entitlements |
| Network behavior | `curl`, `wget`, external APIs, tunnels, telemetry |
| Config outside repo | `~/.codex/config.toml`, `~/.claude.json`, shell profiles |
| Git write operations | `git commit`, `git merge`, `git rebase`, `git tag` |
| Unknown commands | Anything not on the overnight allowlist |

## DENY — never autonomously

### Credentials / identity

- Keychain read/write (`security find-generic-password`, etc.)
- `.env`, `.npmrc`, SSH/GPG keys, signing identities, API keys
- Hard-coded secrets or secret values in logs

### Destructive filesystem

- `rm -rf`, `rmdir`, `unlink`, `shred`, `find ... -delete`
- `chmod -R`, `chown -R`
- Redirects to `~/.zshrc`, `~/.bashrc`, `~/.profile`

### Dangerous git

- `git push`, `git push --force`, `git push --mirror`
- `git reset --hard`, `git clean -fd`, `git remote add/set-url/remove`

### System / release

- `sudo`, `launchctl`, `killall`, `pkill`
- Signing, notarization, entitlements, bundle ID changes

## Command runner deny patterns

Every shell command in the overnight loop must pass through `run_guarded`. Blocked shapes (non-exhaustive):

| Pattern | Reason |
|---------|--------|
| `rm -rf` | Destructive filesystem |
| `git reset --hard` | Dangerous git |
| `git clean -fd` | Dangerous git |
| `git push` / `git push --force` | No overnight pushes |
| `security find-generic-password` | Credential access |
| `sudo` | System control |
| `launchctl` | System control |
| `chmod -R` / `chown -R` | Destructive filesystem |
| `shred` | Destructive filesystem |
| `find ... -delete` | Destructive filesystem |
| `> ~/.zshrc` / `> ~/.bashrc` | Config outside repo |

On block: do not execute, write audit entry (command shape only), return DENY.

On classification error: fail closed (DENY).

## Completion rule

Hermes marks a task complete only when:

- Implementation matches the bounded plan
- `pnpm verify` passes
- Changed behavior has test coverage
- No ASK_FIRST/DENY action ran without stopping
- No secret-looking values appear in logs or audit records

See also: [verification.md](verification.md), [HERMES.md](../HERMES.md).