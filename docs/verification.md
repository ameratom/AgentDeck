# AgentDeck Verification Status

**Last updated:** 2026-06-13 (Hermes overnight autonomy implementation)

**Canonical command:** `pnpm verify`

This document records the real pass/fail status of the verification loop and the starter safety tests required by the Overnight Autonomy Plan (§5, §18).

## pnpm verify Components

| Step | Command | Status | Notes / Evidence |
|------|---------|--------|------------------|
| 1 | `pnpm typecheck` | ✅ PASS | `tsc -b --pretty false` |
| 2 | `pnpm lint` | ✅ PASS | `eslint .` |
| 3 | `pnpm test` | ✅ PASS | 51 Vitest tests |
| 4 | `cargo test --manifest-path src-tauri/Cargo.toml` | ✅ PASS | 127 passed, 3 ignored; includes composer bridge parse/apply tests |
| 5 | `bash scripts/preflight.sh` | ✅ PASS (with expected unavailable) | Core tools detected; LM Studio API may be unavailable |

**Overall:** `pnpm verify` passes end-to-end.

## Starter Test Suite Coverage (§5.2)

| # | Area | Status | Location |
|---|------|--------|----------|
| 1 | Preflight detection | ✅ | `commands/mod.rs` — missing tools return unavailable |
| 2 | Environment scan shape | ✅ | `commands/mod.rs` — stable entities |
| 3 | Secret redaction (mandatory) | ✅ | `commands/mod.rs`, `mcp`, `providers` |
| 4 | Tauri command shapes | ✅ | `commands/*` integration tests |
| 5 | UI smoke | ✅ | Vitest `App.test.ts`, model tests |
| 6 | Deterministic IDs | ✅ | `stable_ids_are_repeatable`, `projects` tests |
| 7 | Command runner deny rules | ✅ | `autonomy/command_runner.rs`, `autonomy/policy.rs` |
| 8 | Safe command classification | ✅ | `autonomy/policy.rs` allowlist + ASK_FIRST |
| 9 | Preflight / scan output shape | ✅ | `commands/mod.rs` |

## Overnight Autonomy Enablement (§18)

| # | Condition | Status | Evidence |
|---|-----------|--------|----------|
| 1 | `pnpm verify` passes | ✅ | This document + CI-local run |
| 2 | Starter safety tests exist | ✅ | Scan, unavailable, deterministic ID tests |
| 3 | Secret-redaction test passes | ✅ | `json_parser_redacts_secret_like_keys`, MCP/provider redaction |
| 4 | Command runner deny tests pass | ✅ | `autonomy::command_runner` blocks `rm -rf`, `git push --force`, `security find-generic-password` |
| 5 | Composer invocation verified | ✅ WIRED | `invoke_composer` → `cursor agent --print --mode plan` (`composer_bridge.rs`); requires `CURSOR_API_KEY` or `cursor agent login` for live runs |
| 6 | Hermes fail-closed tested | ✅ | Empty command → DENY; classification panic → DENY; runner spawn error → DENY |

**Unattended overnight runs:** enabled once Cursor Agent auth is configured and a supervised dry-run produces a clean morning report.

## Hermes CLI smoke checks

```bash
cargo run --manifest-path src-tauri/Cargo.toml --bin hermes -- guard --dry-run "pnpm verify"
cargo run --manifest-path src-tauri/Cargo.toml --bin hermes -- guard --dry-run "rm -rf /tmp/test"   # expect DENY
cargo run --manifest-path src-tauri/Cargo.toml --bin hermes -- overnight --queue tasks/overnight.queue.json
```

## Manual Acceptance Checklist (§14)

- [x] `pnpm verify` passes end-to-end
- [x] Missing optional tools display "unavailable"
- [x] Secret-redaction tests pass
- [x] Command runner blocks injected dangerous commands
- [x] Fail-closed on empty/unclassifiable commands
- [ ] `pnpm tauri dev` launch check (manual)
- [x] Overnight loop writes morning report; no commit/push
- [ ] First supervised overnight run with live Cursor Agent auth (bridge wired; auth is operator setup)

## Known Limitations / Deferrals

- Live Composer calls require Cursor Agent auth; use `AGENTDECK_COMPOSER_BRIDGE=dry-run` for no-op loop tests
- `pnpm tauri dev` not part of automated `pnpm verify`
- Overnight loop creates scratch branch only; does not commit or push

---

*Update this document after any change that affects verification or autonomy enablement.*