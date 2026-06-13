# AgentDeck v0.1.3 Release Checklist

Phase 11 ships **project workspaces**, **scoped discovery/chat/handoffs**, **per-project MCP exports**, **Secure MCP Tunnel controls**, and the **xAI Research MCP** connector.

Use this checklist in order. Do not skip validation gates before signing.

---

## 0. Preconditions

- [ ] Working tree is clean or intentionally staged (no stray `tunnel.log`, `.DS_Store`, or local secrets)
- [ ] `scripts/notarize.local.env` exists locally (not committed) and passes preflight
- [ ] `gh auth status` succeeds for `ameratom/AgentDeck`
- [ ] Apple Developer ID + notarization credentials are current

```bash
./scripts/notarize-preflight.sh   # after a release build exists
gh auth status
```

---

## 1. Land Phase 11 code

Commit the full Phase 11 slice in logical commits (or one release commit if you prefer).

**Modified (34 files)**

- Project scoping: `chat.rs`, `handoffs.rs`, `mcp.rs`, `storage.rs`, `models.rs`
- Tunnel control: `connectors.rs`, `tunnel_control.rs`, `McpView.tsx`
- Projects UI: `src/features/projects/*`, `App.tsx`, `invoke.ts`, `types.ts`
- Docs: `CODEX_HANDOFF.md`, `phase-plan.md`, `mcp-connectors.md`, `chatgpt-app-submission.md`
- Tunnel scripts: `run-chatgpt-mcp-tunnel.sh`, `chatgpt-mcp-tunnel.example.env`

**New (untracked — must be added)**

- [ ] `src-tauri/src/commands/projects.rs`
- [ ] `src-tauri/src/tunnel_control.rs`
- [ ] `src/features/projects/` (view, model, tests)
- [ ] `scripts/xai-research-mcp.mjs`
- [ ] `scripts/xai-research-mcp-launcher.sh`
- [ ] `scripts/xai-research-mcp.node-test.mjs`
- [ ] `data/connectors/xai-research-mcp.claude.json`
- [ ] `data/connectors/xai-research-mcp.codex.toml`

**Suggested commit message**

```text
Prepare v0.1.3 with project workspaces, tunnel controls, and xAI research MCP
```

- [ ] `git status` shows no unintended files staged
- [ ] Push to `origin/main` (or release branch) before tagging

---

## 2. Version bump (all must match `0.1.3`)

| File | Field |
|------|-------|
| `package.json` | `"version"` |
| `src-tauri/Cargo.toml` | `version` |
| `src-tauri/tauri.conf.json` | `"version"` |
| `chatgpt-app-submission.json` | `submission_profile.agentdeck_version` |

After editing `Cargo.toml`:

```bash
cd src-tauri && cargo check && cd ..
```

- [ ] All four files read `0.1.3`
- [ ] `cargo check` updates `Cargo.lock` for the app crate only (commit lockfile if changed)

**Docs to update (not version-critical, but should reflect latest release)**

- [ ] `README.md` — DMG artifact path (`AgentDeck_0.1.3_aarch64.dmg`)
- [ ] `CODEX_HANDOFF.md` — latest release link, status table, release command
- [ ] `docs/phase-plan.md` — mark Phase 11 complete; check off v0.1.3 item
- [ ] `docs/chatgpt-app-submission.md` — submission profile header version
- [ ] `scripts/create-github-release.sh` — default `TAG` (optional; pass arg explicitly instead)

---

## 3. Release notes

- [ ] `RELEASE_NOTES_v0.1.3.md` exists (required by `create-github-release.sh`)
- [ ] Highlights cover: project workspaces, active-project scoping, tunnel UI, xAI research connector
- [ ] Install steps reference `AgentDeck_0.1.3_aarch64.dmg`
- [ ] Requirements unchanged unless new runtime deps were added (Node for filesystem MCP, uv for Grok/Git)

---

## 4. Validation gate

Run from repo root:

```bash
pnpm install
pnpm typecheck
pnpm lint
pnpm test
pnpm test:xai-research-mcp
cd src-tauri && cargo test && cd ..
./scripts/validate-chatgpt-submission.sh
```

- [ ] `pnpm typecheck` — pass
- [ ] `pnpm lint` — pass
- [ ] `pnpm test` — 51+ frontend tests pass
- [ ] `pnpm test:xai-research-mcp` — pass
- [ ] `cargo test` — 98+ Rust tests pass
- [ ] ChatGPT submission validator — pass

---

## 5. Manual smoke test (desktop)

```bash
pnpm tauri dev
```

**Projects**

- [ ] Register a project path; first project becomes active
- [ ] Switch active project; graph/chat/handoffs rescope
- [ ] Remove a non-active project; audit log records mutation
- [ ] Export local data; project entries are redacted appropriately

**Scoped features**

- [ ] Graph scan shows project boundary (not full-machine bleed)
- [ ] Chat injects active project root context
- [ ] Handoff preview/run attaches `project_id`
- [ ] MCP view shows per-project connector export profiles

**Secure MCP Tunnel (MCP view)**

- [ ] Status loads (`configured` / `running` / `ready`)
- [ ] Start tunnel when `chatgpt-mcp-tunnel.env` is configured
- [ ] Operator UI opens; public HTTPS URL visible
- [ ] Stop tunnel cleanly

**xAI Research MCP (optional, requires xAI credentials)**

```bash
node --test scripts/xai-research-mcp.node-test.mjs
# or register launcher in Codex/Claude and call one tool
```

- [ ] Launcher reads `grok-mcp.env` bridge
- [ ] `xai_research.search_web` returns sources (live key only)

---

## 6. Release build

```bash
source scripts/notarize.local.env
pnpm tauri build
```

Artifacts:

- `src-tauri/target/release/bundle/macos/AgentDeck.app`
- `src-tauri/target/release/bundle/dmg/AgentDeck_0.1.3_aarch64.dmg`

- [ ] Build completes without errors
- [ ] App launches from `.app` bundle (not only `tauri dev`)
- [ ] MCP HTTP responds on `http://127.0.0.1:7823/mcp`

```bash
curl -s -X POST http://127.0.0.1:7823/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | jq '.result.tools | length'
```

- [ ] `tools/list` returns expected read-only tool count

---

## 7. Sign and notarize

```bash
source scripts/notarize.local.env
./scripts/notarize-preflight.sh
./scripts/notarize-macos.sh
```

- [ ] `codesign --verify --deep --strict` passes on `.app`
- [ ] Notarization succeeds (no stapler errors)
- [ ] `spctl --assess --type execute` passes on release `.app`

Re-open the notarized `.app` and repeat the **Projects** and **tunnel start** smoke checks.

- [ ] Notarized app passes Gatekeeper on a clean double-click launch

---

## 8. Publish GitHub release

```bash
./scripts/create-github-release.sh v0.1.3
```

- [ ] Tag `v0.1.3` created on GitHub
- [ ] DMG attached to release
- [ ] Release notes render correctly

Verify: https://github.com/ameratom/AgentDeck/releases/tag/v0.1.3

---

## 9. Post-release documentation

- [ ] `CODEX_HANDOFF.md` — Phase 11 complete; latest release `v0.1.3`
- [ ] `docs/phase-plan.md` — Phase 11 checkbox complete; optional Phase 12 stub
- [ ] Commit doc updates: `Post-release: update handoff for v0.1.3`

---

## 10. ChatGPT submission (follow-up, same sprint)

Not blocking the DMG release, but next after v0.1.3 is live.

- [ ] Bump `chatgpt-app-submission.json` `agentdeck_version` to `0.1.3` (step 2)
- [ ] Configure `~/Library/Application Support/com.agentdeck.desktop/chatgpt-mcp-tunnel.env`
- [ ] Start tunnel from notarized app; copy HTTPS MCP URL from operator UI
- [ ] Developer-mode smoke: run all `test_cases` and `negative_test_cases`
- [ ] Dashboard submission: metadata, screenshots, privacy/terms URLs
- [ ] `./scripts/validate-chatgpt-submission.sh` passes against release build

Guide: `docs/chatgpt-app-submission.md`

---

## Quick reference

| Step | Command |
|------|---------|
| Validate | `pnpm typecheck && pnpm lint && pnpm test && cargo test --manifest-path src-tauri/Cargo.toml` |
| Build | `source scripts/notarize.local.env && pnpm tauri build` |
| Notarize | `./scripts/notarize-macos.sh` |
| Publish | `./scripts/create-github-release.sh v0.1.3` |

---

## Rollback

If notarization or Gatekeeper fails after tag:

1. Do **not** delete the tag until a fixed build is ready.
2. Fix signing/entitlements or code issue.
3. Rebuild, re-notarize, and `gh release upload v0.1.3 <new-dmg> --clobber`.
4. Document the fix in release notes.