# Hermes — Overnight Autonomy Operator

Hermes is the local orchestrator for bounded overnight coding tasks on AgentDeck. Composer 2.5 is a patch-only worker behind `invoke_composer`. The command runner (`run_guarded`) is the mechanical backstop.

**Unattended overnight runs require all [enablement conditions](docs/verification.md#overnight-autonomy-enablement-18) to pass**, including Cursor Agent CLI auth (`CURSOR_API_KEY` or `cursor agent login`).

## Execution chain

```
Planner → Hermes → Composer (patch text) → run_guarded → repo
```

Authority: user approval > AgentDeck policy > Hermes policy > command runner > Composer suggestions.

Policy details: [docs/autonomy-policy.md](docs/autonomy-policy.md)

## CLI

From the repo root:

```bash
# Classify or execute a single command through the runner
cargo run --manifest-path src-tauri/Cargo.toml --bin hermes -- guard --dry-run "pnpm verify"
cargo run --manifest-path src-tauri/Cargo.toml --bin hermes -- guard --execute "pnpm typecheck"

# Overnight loop (dry-run bridge — no Cursor call, no patch)
AGENTDECK_COMPOSER_BRIDGE=dry-run cargo run --manifest-path src-tauri/Cargo.toml --bin hermes -- overnight --queue tasks/overnight.queue.json

# Overnight loop with Cursor Agent CLI bridge
source scripts/composer-bridge.local.env  # set CURSOR_API_KEY, model, timeout
cargo run --manifest-path src-tauri/Cargo.toml --bin hermes -- overnight --queue tasks/overnight.queue.json --execute-verify
```

## Task queue format

`tasks/overnight.queue.json` — ordered list of pre-classified **ALLOW** tasks only.

```json
[
  {
    "id": "task-001",
    "title": "Short task description",
    "scope": "ALLOW",
    "files_hint": ["src-tauri/src/autonomy/"],
    "acceptance": ["pnpm verify passes", "new test covers behavior"],
    "deferred_if": "requires schema change"
  }
]
```

ASK_FIRST and DENY tasks belong in a morning review list, not the overnight queue.

## Run loop

1. Create scratch branch `hermes/overnight-YYYYMMDD` (no commit, no push)
2. For each queued task:
   - Validate `scope: ALLOW`
   - Build bounded `ComposerRequest`
   - `invoke_composer` → `cursor agent --print --mode plan` → patch text only
   - Apply allowed edits on scratch branch
   - Run `pnpm verify` through `run_guarded`
3. Retry cap: **2** per task
4. On ASK_FIRST/DENY or verify failure after retries: stop task, log blocker, continue to next independent task
5. Write morning report to `tasks/reports/overnight-YYYYMMDD.md`

## Stop conditions

- Cursor Agent auth missing → coding tasks blocked (`AuthRequired`)
- Command runner DENY
- Policy ASK_FIRST without user approval
- `pnpm verify` fails after retry cap
- Stop signal file (manual kill after current verify step)

## Morning report fields

- Branch, commit status (always uncommitted)
- Tasks attempted / completed / blocked
- Files changed, commands run
- Verification result, test failures
- Denied actions, approval-needed actions
- Composer calls, patch summaries
- Known limitations, recommended human review

## invoke_composer contract

```typescript
type ComposerRequest = {
  taskId: string;
  repoRoot: string;
  instructions: string;
  allowedFiles: string[];
  readonlyContext: Record<string, string>;
  constraints: string[];
  expectedPatchFormat: "unified_diff";
};

type ComposerResponse = {
  patchText: string;
  summary: string;
  suggestedTests: string[];
  suggestedCommands: string[]; // suggestions only; runner vets each
};
```

Implementation:

- `src-tauri/src/autonomy/composer.rs` — public `invoke_composer` seam
- `src-tauri/src/autonomy/composer_bridge.rs` — Cursor Agent CLI bridge (`cursor agent --print --trust --mode plan`)
- `scripts/composer-bridge.example.env` — auth/model configuration template

Bridge rules:

- Plan mode only (read-only); Composer returns unified diff text, Hermes applies via `git apply`
- `suggested_commands` are logged as approval-needed; never auto-executed
- Set `AGENTDECK_COMPOSER_BRIDGE=dry-run` for tests or supervised no-op runs