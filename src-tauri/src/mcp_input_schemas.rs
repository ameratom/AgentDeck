//! JSON Schemas for AgentDeck MCP write-tool inputs (developer / full profile).

use serde_json::{json, Value};

const AGENT_ID_PATTERN: &str = "^agent:[a-z0-9][a-z0-9-]*$";
const PROVIDER_ID_PATTERN: &str = "^provider:[a-z0-9][a-z0-9-:]*$";
const MCP_SERVER_ID_PATTERN: &str = "^mcp-server:[0-9a-f]{16}$";
const SKILL_ID_PATTERN: &str = "^[a-z0-9][a-z0-9-]*$";

fn agent_id_property(description: &str, required_for_permission: bool) -> Value {
    let mut schema = json!({
        "type": "string",
        "pattern": AGENT_ID_PATTERN,
        "description": description,
        "examples": [
            "agent:agentdeck",
            "agent:codex",
            "agent:claude-code",
            "agent:grok"
        ]
    });
    if required_for_permission {
        schema.as_object_mut().expect("agent id schema").insert(
            "description".to_owned(),
            json!(format!(
                "{description} Must match an AgentDeck permission row and include the `agent:` prefix."
            )),
        );
    }
    schema
}

fn approval_tokens_property() -> Value {
    json!({
        "type": "array",
        "description": "Explicit approval tokens recorded with the handoff. Include at least one non-empty string such as `user-approved` after the user confirms dispatch in AgentDeck.",
        "items": {
            "type": "string",
            "minLength": 1,
            "maxLength": 128
        },
        "minItems": 1,
        "examples": [["user-approved"], ["desktop-approval", "user-confirmed"]]
    })
}

pub fn dispatch_handoff() -> Value {
    json!({
        "type": "object",
        "properties": {
            "callerAgentId": agent_id_property(
                "Optional MCP caller identity checked against AgentDeck permissions. Defaults to agent:agentdeck when omitted.",
                true,
            ),
            "sourceAgentId": {
                "type": "string",
                "pattern": AGENT_ID_PATTERN,
                "description": "Deterministic discovered-agent ID from scan_environment entities (entityType=agent), e.g. agent:codex.",
                "examples": ["agent:codex", "agent:claude-code"]
            },
            "sourceAgentName": {
                "type": "string",
                "minLength": 1,
                "maxLength": 128,
                "description": "Human-readable source agent label shown in the handoff record."
            },
            "targetProviderId": {
                "type": "string",
                "pattern": PROVIDER_ID_PATTERN,
                "description": "Registered provider adapter ID from AgentDeck providers inventory, e.g. provider:lmstudio:http-localhost-1234-v1.",
                "examples": ["provider:lmstudio:http-localhost-1234-v1", "provider:xai:grok"]
            },
            "targetProviderName": {
                "type": "string",
                "minLength": 1,
                "maxLength": 128,
                "description": "Human-readable provider label matching targetProviderId."
            },
            "targetModelId": {
                "type": "string",
                "minLength": 1,
                "maxLength": 256,
                "description": "Provider model identifier returned by the provider adapter models list."
            },
            "title": {
                "type": "string",
                "minLength": 1,
                "maxLength": 4096,
                "description": "Short handoff title stored on the run record."
            },
            "task": {
                "type": "string",
                "minLength": 1,
                "maxLength": 4096,
                "description": "Primary task instructions sent to the target provider."
            },
            "context": {
                "type": "string",
                "maxLength": 4096,
                "description": "Optional supporting context for the target provider."
            },
            "approvals": approval_tokens_property()
        },
        "required": [
            "sourceAgentId",
            "sourceAgentName",
            "targetProviderId",
            "targetProviderName",
            "targetModelId",
            "title",
            "task",
            "context",
            "approvals"
        ],
        "additionalProperties": false
    })
}

pub fn execute_skill() -> Value {
    json!({
        "type": "object",
        "properties": {
            "callerAgentId": agent_id_property(
                "Optional MCP caller identity checked against execute-skill permissions. Defaults to agent:agentdeck when omitted.",
                true,
            ),
            "skillId": {
                "type": "string",
                "pattern": SKILL_ID_PATTERN,
                "description": "Skill ID from AgentDeck plugin inventory (plugins/skills registry), matching the skill frontmatter id.",
                "examples": ["test-skill", "xlsx"]
            }
        },
        "required": ["skillId"],
        "additionalProperties": false
    })
}

pub fn toggle_mcp_server() -> Value {
    json!({
        "type": "object",
        "properties": {
            "callerAgentId": agent_id_property(
                "Optional MCP caller identity checked against write-config permissions. Defaults to agent:agentdeck when omitted.",
                true,
            ),
            "serverId": {
                "type": "string",
                "pattern": MCP_SERVER_ID_PATTERN,
                "description": "Deterministic MCP server ID from list_mcp_servers / scan_environment (format mcp-server:<16 lowercase hex>).",
                "examples": ["mcp-server:0123456789abcdef"]
            },
            "enabled": {
                "type": "boolean",
                "description": "true to enable the server entry in the local JSON config; false to disable it with backup/restore safety."
            }
        },
        "required": ["serverId", "enabled"],
        "additionalProperties": false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_tool_schemas_include_descriptions_and_patterns() {
        for (name, schema) in [
            ("dispatch_handoff", dispatch_handoff()),
            ("execute_skill", execute_skill()),
            ("toggle_mcp_server", toggle_mcp_server()),
        ] {
            let properties = schema
                .get("properties")
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("{name} missing properties"));
            assert!(
                properties.values().all(|field| {
                    field.get("description").is_some() || field.get("type") == Some(&json!("boolean"))
                }),
                "{name} has fields without descriptions"
            );
            assert!(
                properties
                    .get("sourceAgentId")
                    .or_else(|| properties.get("skillId"))
                    .or_else(|| properties.get("serverId"))
                    .and_then(|field| field.get("pattern"))
                    .is_some(),
                "{name} missing key pattern"
            );
        }
    }
}