# AgentDeck — Codex Handoff

Build folder:

```text
/Users/claudemccready/Desktop/Scripts/Codex/AgentDeck
```

Remote: `https://github.com/ameratom/AgentDeck` (`main`)

Latest release: [v0.1.6](https://github.com/ameratom/AgentDeck/releases/tag/v0.1.6) (tag pending — local `0.1.6` ready, notarized DMG not built yet)

## Current Status (June 2026)

Phases **0–13** are complete. ChatGPT app **v1.0.0** is **in review** (not published yet).

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
| Project-aware onboarding + Claude Code MCP serve export | ✔ |
| Secure MCP Tunnel controls (MCP view) | ✔ |
| ChatGPT OAuth PRM + Streamable HTTP MCP fixes | ✔ |
| Read-only ChatGPT MCP profile (`read_only_v1_1`, 10 tools) | ✔ |
| MCP `outputSchema` + `structuredContent` on tool results | ✔ |
| Hermes overnight autonomy (CLI + policy) | ✔ |
| Signed + notarized macOS DMG | ✔ v0.1.5 (v0.1.6 bump local; notarize pending) |

### Project MCP config (`.mcp.json`)

Registered servers: `agentdeck`, `grok-mcp`, `filesystem`, `git`, `agentdeck-xai-research-mcp` — connect in Claude Code as needed.

Grok auth flows through `~/Library/Application Support/com.agentdeck.desktop/grok-mcp.env` (synced from encrypted xAI credentials).

### Dev commands

```bash
pnpm install
pnpm tauri dev
pnpm verify
pnpm test:xai-research-mcp
```

### Release

```bash
source scripts/notarize.local.env
pnpm tauri build
./scripts/notarize-macos.sh
./scripts/create-github-release.sh v0.1.6
```

### ChatGPT tunnel

```bash
./scripts/smoke-chatgpt-tunnel.sh
```

Set `MCP_PUBLIC_RESOURCE_URL` in `chatgpt-mcp-tunnel.env` to the HTTPS URL ChatGPT uses. Guide: `docs/chatgpt-app-submission.md`

## ChatGPT app submission (v1.0.0 — in review)

| Field | Value |
|-------|-------|
| Platform status | **REVIEW** (submitted, awaiting OpenAI approval) |
| App version | `1.0.0` |
| MCP URL | `https://mcp.thedeckisstacked.win/mcp` |
| Tools in snapshot | 10 read-only (no write tools) |
| Safety scan | `SCANNED_OK` |
| Local export | `agentdeck-1-0-0.json` (platform dump; gitignored) |

Canonical metadata: `chatgpt-app-submission.json` (read-only v1.1 profile)

**Publishing checklist (human):**

1. **While in REVIEW:** keep AgentDeck.app running and the Secure MCP Tunnel connected so reviewers can reach `:7823` via the public URL.
2. **When status → Approved:** open [platform.openai.com/apps-manage](https://platform.openai.com/apps-manage) and click **Publish** on version `1.0.0`.
3. **After publish:** confirm the app is findable by exact name; directory placement is optional/enhanced distribution only.
4. **Do not change** the published MCP tool contract (names, schemas, URLs) until a new draft version is submitted and approved.

Write tools (`dispatch_handoff`, `execute_skill`, `toggle_mcp_server`) remain in developer mode for a future submission.

## Overnight autonomy (Hermes)

- Policy: `docs/autonomy-policy.md`
- Operator guide: `HERMES.md`
- Verification: `docs/verification.md` (`pnpm verify`, §18 enablement)
- CLI: `pnpm hermes:guard`, `pnpm hermes:overnight`, `pnpm hermes:overnight:dry-run`
- Composer bridge: `cursor agent --print --mode plan` (requires `cursor agent login` or `CURSOR_API_KEY`)
- Queue: `tasks/overnight.queue.json` → reports in `tasks/reports/`

First supervised live overnight run recommended before unattended use. Dry-run report `tasks/reports/overnight-20260613.md` blocked on Composer diff parsing.

## Mission

Build **AgentDeck**, a macOS-first local control plane for AI agents, local LLMs, MCP servers, IDE integrations, skills, plugins, webhooks, and project-specific automations.

AgentDeck should make one thing obvious:

> What is running, what is connected, what is allowed, what changed, and which agent handled which part of the work.

The first release is **observability + controlled chat routing**, not full autonomous orchestration.

## Next candidates

1. **Wait for ChatGPT review** → Publish v1.0.0 when Approved
2. **Notarize v0.1.6** — `pnpm tauri build` + `./scripts/notarize-macos.sh` + GitHub release
3. **Hermes** — supervised overnight run with live `CURSOR_API_KEY` / `cursor agent login`
4. **ChatGPT write-tools follow-up** — second submission with deferred MCP tools
5. **Core product** — handoff router automation, webhooks