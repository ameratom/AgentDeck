# AgentDeck Data Model

## Entities

### Agent

Examples:

- Codex
- Claude Code
- Hermes
- OpenClaw
- Grok

Grok is an xAI-backed source agent. AgentDeck keeps it visible when xAI credentials are present, and marks it degraded when xAI is configured but the live health check fails.

Fields:

- id
- name
- type
- status
- command
- version
- config paths
- capabilities
- metadata

### Provider

Examples:

- LM Studio
- OpenAI-compatible
- xAI
- Anthropic

Fields:

- id
- name
- base URL
- auth mode
- health
- models
- metadata

### Model

Examples:

- local Gemma model returned by LM Studio
- Grok model
- Claude model
- OpenAI model

Fields:

- id
- provider id
- model id
- display name
- context window if known
- capabilities if known

### MCP server

Fields:

- id
- name
- transport
- command
- args
- url
- env keys
- config source
- enabled
- health
- declared tools

### Skill

Fields:

- id
- name
- description
- file path
- version
- allowed capabilities

### Plugin

Fields:

- id
- name
- version
- enabled
- status
- capabilities
- config

### Project

Fields:

- id
- name
- path
- git root
- active branch
- detected configs
- agent rules

### Run

Fields:

- id
- thread
- source agent
- target agent/provider
- status
- input
- output
- approvals
- audit refs

## Graph edges

Use edges to represent both configuration and runtime relationships.

Examples:

```text
Codex -> AGENTS.md configured_by
Codex -> filesystem MCP uses
LM Studio -> Gemma model hosts
Claude Code -> .mcp.json configured_by
OpenClaw -> gateway process runs_in
AgentDeck Chat -> LM Studio calls
Grok -> Web Search uses
Hermes -> skills owns
```

## Deterministic IDs

Suggested ID format:

```text
agent:codex
agent:claude-code
agent:grok
provider:lmstudio:http-localhost-1234-v1
model:lmstudio:<model-id>
mcp:<config-path-hash>:<server-name>
project:<path-hash>
process:<pid>
config:<path-hash>
```
