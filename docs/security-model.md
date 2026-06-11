# AgentDeck Security Model

## Default posture

AgentDeck must be safe before it is powerful.

Default mode:

```text
read-only observability
```

The app may scan configs, show statuses, and call local chat endpoints. It must not modify agent configs, start unknown MCP servers, or run shell commands without approval.

## Capability model

```ts
type Capability =
  | "read_files"
  | "write_files"
  | "run_shell"
  | "access_network"
  | "use_browser"
  | "send_messages"
  | "modify_git"
  | "manage_processes"
  | "call_mcp_tools"
  | "store_memory"
  | "deploy";
```

## Risk scoring

### Low

- read local config metadata
- read process names
- list models
- chat with local LM Studio model

### Medium

- access network
- call remote provider
- read project files
- call read-only MCP tools

### High

- shell command execution
- write files
- modify git
- send messages to external systems
- start/stop agents

### Critical

- deploy
- run arbitrary commands
- write shell config/profile files
- modify credential stores
- expose local filesystem to remote MCP
- run untrusted MCP server commands

## Approval rules

Any action containing these capabilities requires explicit user approval:

- `write_files`
- `run_shell`
- `modify_git`
- `manage_processes`
- `deploy`
- `send_messages`
- `use_browser`

Approval preview must show:

- actor
- target
- command or endpoint
- working directory
- environment variables with secrets redacted
- affected files
- risk level
- rollback info where possible.

## Redaction

Redact values matching:

- API keys
- Bearer tokens
- `.env`
- private keys
- OAuth tokens
- session cookies
- passwords
- SSH keys

Never store full command output if it may include secrets. Provide truncation and redaction.

## MCP safety

Before starting or using an MCP server, show:

- server name
- transport
- command
- args
- cwd
- env keys only, not values
- remote URL if applicable
- declared tools, if available.

Do not auto-start MCP servers during scanning.

## Provider key storage

Use macOS Keychain through Rust `keyring`.

Allowed fallback:

- environment variables for local dev only
- never persist plaintext API keys in SQLite.

## Audit log

Log:

- scans
- provider calls metadata
- handoffs
- command approvals
- command results
- config validation errors
- plugin enable/disable
- MCP server invocations.

Do not log:

- raw API keys
- full private file contents
- secrets
- private key material.

## Privacy defaults

1. Local model first.
2. Cloud model only when user selects or approves it.
3. Project context is not sent externally unless previewed.
4. Chat threads stored locally.
5. Export/delete controls required before adding cloud sync.
