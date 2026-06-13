# AgentDeck — Codex Handoff

Build folder:

```text
/Users/claudemccready/Desktop/Scripts/Codex/AgentDeck
```

Remote: `https://github.com/ameratom/AgentDeck` (`main`)

Latest release: [v0.1.4](https://github.com/ameratom/AgentDeck/releases/tag/v0.1.4)

## Current Status (June 2026)

Phases **0–11** are complete. v0.1.4 ships ChatGPT MCP connector fixes.

### Shipped

| Area | Status |
|------|--------|
| Environment discovery + orbital graph | ✔ |
| Multi-provider chat (LM Studio, xAI, Anthropic, Codex, Claude Code) | ✔ |
| Encrypted credential store + legacy Keychain import | ✔ |
| MCP inventory + HTTP server (`:7823`) | ✔ |
| External MCP connectors (Grok bridge, filesystem, git) | ✔ |
| xAI Research MCP connector (read-only web research) | ✔ |
| Plugins/skills registry + audit log | ✔ |
| Manual handoffs with approval gate | ✔ |
| Handoff router rules (Settings + suggestions) | ✔ |
| Local project registry + active workspace selection | ✔ |
| Project-scoped config discovery, graph context, chat, and handoffs | ✔ |
| Per-project filesystem/Git MCP profiles with validated exports | ✔ |
| Secure MCP Tunnel controls (MCP view) | ✔ |
| ChatGPT OAuth PRM + Streamable HTTP MCP fixes | ✔ v0.1.4 |
| Signed + notarized macOS DMG | ✔ v0.1.4 |

### Project MCP config (`.mcp.json`)

Registered servers: `agentdeck`, `grok-mcp`, `filesystem`, `git`, `agentdeck-xai-research-mcp` — connect in Claude Code as needed.

Grok auth flows through `~/Library/Application Support/com.agentdeck.desktop/grok-mcp.env` (synced from encrypted xAI credentials).

### Dev commands

```bash
pnpm install
pnpm tauri dev
pnpm test
pnpm test:xai-research-mcp
cd src-tauri && cargo test
```

### Release

```bash
source scripts/notarize.local.env
pnpm tauri build
./scripts/notarize-macos.sh
./scripts/create-github-release.sh v0.1.4
```

### ChatGPT tunnel

```bash
./scripts/smoke-chatgpt-tunnel.sh
```

Set `MCP_PUBLIC_RESOURCE_URL` in `chatgpt-mcp-tunnel.env` to the HTTPS URL ChatGPT uses. Guide: `docs/chatgpt-app-submission.md`

## Mission

Build **AgentDeck**, a macOS-first local control plane for AI agents, local LLMs, MCP servers, IDE integrations, skills, plugins, webhooks, and project-specific automations.

AgentDeck should make one thing obvious:

> What is running, what is connected, what is allowed, what changed, and which agent handled which part of the work.

The first release is **observability + controlled chat routing**, not full autonomous orchestration.

## ChatGPT submission (ready to test)

- Import file: `chatgpt-app-submission.json` (read-only v1 profile, 7 tools)
- Guide: `docs/chatgpt-app-submission.md`
- Test prompts: `docs/chatgpt-test-prompts.md`
- Validate: `./scripts/validate-chatgpt-submission.sh`
- Tunnel smoke: `./scripts/smoke-chatgpt-tunnel.sh` (19 checks; requires AgentDeck + tunnel running)
- Tunnel helper: `scripts/run-chatgpt-mcp-tunnel.sh`
- Tunnel UI: MCP view → Start tunnel → Open operator UI

**Operator checklist (needs human in ChatGPT):**

1. Launch AgentDeck.app
2. Confirm tunnel connected (smoke test green)
3. Run positive prompts from `docs/chatgpt-test-prompts.md` (health → agents → MCP → audit → graph → combined → scan → handoff)
4. Run negative prompts; confirm AgentDeck is **not** invoked
5. Submit via [platform.openai.com/apps](https://platform.openai.com/apps) using tunnel URL + `chatgpt-app-submission.json`

## Overnight autonomy (Hermes — implemented, uncommitted)

- Policy: `docs/autonomy-policy.md`
- Operator guide: `HERMES.md`
- Verification: `docs/verification.md` (`pnpm verify`, §18 enablement)
- CLI: `pnpm hermes:guard`, `pnpm hermes:overnight`, `pnpm hermes:overnight:dry-run`
- Composer bridge: `cursor agent --print --mode plan` (requires `cursor agent login` or `CURSOR_API_KEY`)
- Queue: `tasks/overnight.queue.json` → reports in `tasks/reports/`

First supervised live overnight run recommended before unattended use.

## Next candidates

1. **You:** ChatGPT manual prompt test + Platform dashboard submission
2. **You:** Review + commit Hermes/overnight autonomy work (on `main`, uncommitted)
3. Phase 12 complete — project-aware onboarding + Claude Code MCP serve export (uncommitted with Hermes work)