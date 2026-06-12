---
title: AgentDeck Privacy Policy
permalink: /privacy-policy/
---

# AgentDeck Privacy Policy

Effective date: 2026-06-12

AgentDeck is a local-first macOS app for inspecting and coordinating AI agents, LLM providers, MCP servers, plugins, skills, projects, and handoffs. AgentDeck is designed to keep user data on the user's Mac. AgentDeck does not operate a hosted cloud service or AgentDeck-owned servers.

## Information AgentDeck stores locally

AgentDeck may store the following information on the user's Mac in local app storage, including a local SQLite database:

- Discovered local agents, tools, processes, and provider readiness information.
- MCP server inventory, configuration metadata, risk labels, and connection status.
- Project, plugin, skill, routing, and settings metadata created or discovered by the app.
- Handoff requests, run records, chat messages created in AgentDeck, and audit events.
- Provider credentials or tokens the user chooses to save.

Credentials are encrypted at rest in AgentDeck's local secret store. AgentDeck does not read macOS Keychain entries at runtime. Any legacy Keychain import must be explicitly initiated by the user.

## Information that may leave the Mac

AgentDeck itself does not upload local app data to AgentDeck-owned servers. AgentDeck has no AgentDeck cloud backend.

When the user connects AgentDeck to ChatGPT through a Secure MCP Tunnel or developer-mode connector, ChatGPT may receive only the MCP tool responses that the user requests during an active tunnel session. Those responses can include read-only summaries of local agents, tools, provider status, MCP server inventory, graph relationships, stored handoff runs, and audit events.

When the user sends chat requests from AgentDeck to an external model provider such as xAI, Anthropic, OpenAI-compatible services, or another configured provider, the prompt and related request content are sent to that provider according to the provider selected by the user. Local LM Studio requests stay on the user's local LM Studio server.

## What AgentDeck does not do

AgentDeck does not:

- Sell user data.
- Operate third-party advertising or analytics inside AgentDeck.
- Upload credentials to AgentDeck-owned servers.
- Modify third-party agent or MCP configuration files without user action and approval.
- Send data to ChatGPT unless the user has an active connector/tunnel session and requests an MCP tool response.

## User control and deletion

AgentDeck data is stored locally on the user's Mac. The user can delete AgentDeck app data by removing the app's local Application Support data, normally under:

```text
~/Library/Application Support/com.agentdeck.desktop/
```

Users can also remove saved provider credentials from AgentDeck's Providers settings before uninstalling the app.

## Security notes

AgentDeck is intended for local development and observability workflows. Users should review MCP tool responses before sharing them externally, especially if local project names, paths, tool configurations, handoff notes, or audit events may contain sensitive information.

## Contact

For privacy or support questions, open an issue at:

<https://github.com/ameratom/AgentDeck/issues>
