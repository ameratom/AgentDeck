//! JSON Schemas for AgentDeck MCP tool structured outputs (tools/list `outputSchema`).

use serde_json::{json, Value};

fn tool_status_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "available": { "type": "boolean" },
            "version": { "type": ["string", "null"] },
            "path": { "type": ["string", "null"] },
            "pathSource": { "type": ["string", "null"] },
            "error": { "type": ["string", "null"] }
        },
        "required": ["name", "available"]
    })
}

fn provider_health_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "endpoint": { "type": "string" },
            "available": { "type": "boolean" },
            "detail": { "type": "string" }
        },
        "required": ["name", "endpoint", "available", "detail"]
    })
}

fn discovered_entity_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": { "type": "string" },
            "entityType": { "type": "string" },
            "name": { "type": "string" },
            "status": { "type": "string" },
            "source": { "type": "string" },
            "metadata": {
                "type": "object",
                "additionalProperties": { "type": "string" }
            }
        },
        "required": ["id", "entityType", "name", "status", "source", "metadata"]
    })
}

fn handoff_run_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": { "type": "string" },
            "projectId": { "type": ["string", "null"] },
            "threadId": { "type": "string" },
            "sourceAgentId": { "type": "string" },
            "sourceAgentName": { "type": "string" },
            "targetProviderId": { "type": "string" },
            "targetProviderName": { "type": "string" },
            "targetModelId": { "type": "string" },
            "title": { "type": "string" },
            "task": { "type": "string" },
            "context": { "type": "string" },
            "status": { "type": "string" },
            "output": { "type": "string" },
            "error": { "type": ["string", "null"] },
            "approvals": { "type": "array", "items": { "type": "string" } },
            "auditRef": { "type": ["string", "null"] },
            "createdAt": { "type": "string" },
            "updatedAt": { "type": "string" }
        },
        "required": [
            "id",
            "threadId",
            "sourceAgentId",
            "sourceAgentName",
            "targetProviderId",
            "targetProviderName",
            "targetModelId",
            "title",
            "task",
            "context",
            "status",
            "output",
            "approvals",
            "createdAt",
            "updatedAt"
        ]
    })
}

fn audit_event_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": { "type": "string" },
            "action": { "type": "string" },
            "status": { "type": "string" },
            "model": { "type": "string" },
            "conversationId": { "type": "string" },
            "runId": { "type": ["string", "null"] },
            "durationMs": { "type": "integer" },
            "createdAt": { "type": "string" }
        },
        "required": [
            "id",
            "action",
            "status",
            "model",
            "conversationId",
            "durationMs",
            "createdAt"
        ]
    })
}

fn xai_research_result_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "answer": {
                "type": "string",
                "description": "Evidence-backed summary or answer with inline markdown citations."
            },
            "sources": {
                "type": "array",
                "items": { "type": "string", "format": "uri" },
                "description": "Source URLs cited in the answer."
            },
            "model": { "type": "string" },
            "costUsd": { "type": ["number", "null"] }
        },
        "required": ["answer", "sources", "model"]
    })
}

pub fn scan_environment() -> Value {
    json!({
        "type": "object",
        "description": "Local environment inventory from a read-only scan.",
        "properties": {
            "scannedAt": { "type": "string", "format": "date-time" },
            "project": {
                "type": ["object", "null"],
                "properties": {
                    "id": { "type": "string" },
                    "name": { "type": "string" },
                    "path": { "type": "string" }
                }
            },
            "tools": { "type": "array", "items": tool_status_schema() },
            "providers": { "type": "array", "items": provider_health_schema() },
            "processes": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "pid": { "type": "integer" },
                        "name": { "type": "string" },
                        "executable": { "type": ["string", "null"] },
                        "command": { "type": ["string", "null"] },
                        "category": { "type": "string" }
                    },
                    "required": ["id", "pid", "name", "category"]
                }
            },
            "configs": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "kind": { "type": "string" },
                        "path": { "type": "string" },
                        "exists": { "type": "boolean" },
                        "format": { "type": ["string", "null"] },
                        "valid": { "type": ["boolean", "null"] },
                        "topLevelKeys": { "type": "array", "items": { "type": "string" } },
                        "error": { "type": ["string", "null"] }
                    },
                    "required": ["id", "kind", "path", "exists", "topLevelKeys"]
                }
            },
            "entities": { "type": "array", "items": discovered_entity_schema() }
        },
        "required": [
            "scannedAt",
            "tools",
            "providers",
            "processes",
            "configs",
            "entities"
        ]
    })
}

pub fn get_graph() -> Value {
    json!({
        "type": "object",
        "description": "Relationship graph derived from the current environment scan.",
        "properties": {
            "scannedAt": { "type": "string", "format": "date-time" },
            "nodes": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "label": { "type": "string" },
                        "entityType": { "type": "string" },
                        "status": { "type": "string" },
                        "source": { "type": "string" }
                    },
                    "required": ["id", "label", "entityType", "status", "source"]
                }
            },
            "edges": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "source": { "type": "string" },
                        "target": { "type": "string" },
                        "label": { "type": "string" }
                    },
                    "required": ["id", "source", "target", "label"]
                }
            }
        },
        "required": ["scannedAt", "nodes", "edges"]
    })
}

pub fn list_agents() -> Value {
    json!({
        "type": "object",
        "properties": {
            "agents": {
                "type": "array",
                "items": discovered_entity_schema()
            },
            "count": { "type": "integer", "minimum": 0 }
        },
        "required": ["agents", "count"]
    })
}

pub fn list_mcp_servers() -> Value {
    json!({
        "type": "object",
        "description": "Read-only MCP configuration inventory.",
        "properties": {
            "scannedAt": { "type": "string", "format": "date-time" },
            "sources": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "client": { "type": "string" },
                        "path": { "type": "string" },
                        "exists": { "type": "boolean" },
                        "parsed": { "type": "boolean" },
                        "serverCount": { "type": "integer" },
                        "error": { "type": ["string", "null"] }
                    },
                    "required": ["id", "client", "path", "exists", "parsed", "serverCount"]
                }
            },
            "servers": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "name": { "type": "string" },
                        "client": { "type": "string" },
                        "transport": { "type": "string" },
                        "command": { "type": ["string", "null"] },
                        "args": { "type": "array", "items": { "type": "string" } },
                        "cwd": { "type": ["string", "null"] },
                        "url": { "type": ["string", "null"] },
                        "envKeys": { "type": "array", "items": { "type": "string" } },
                        "source": { "type": "string" },
                        "enabled": { "type": "boolean" },
                        "commandAvailable": { "type": ["boolean", "null"] },
                        "declaredTools": { "type": "array", "items": { "type": "string" } },
                        "riskLevel": { "type": "string" },
                        "riskReasons": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": [
                        "id",
                        "name",
                        "client",
                        "transport",
                        "args",
                        "envKeys",
                        "source",
                        "enabled",
                        "declaredTools",
                        "riskLevel",
                        "riskReasons"
                    ]
                }
            }
        },
        "required": ["scannedAt", "sources", "servers"]
    })
}

pub fn health_check() -> Value {
    json!({
        "type": "object",
        "description": "Local preflight readiness report.",
        "properties": {
            "checkedAt": { "type": "string", "format": "date-time" },
            "tools": { "type": "array", "items": tool_status_schema() },
            "providers": { "type": "array", "items": provider_health_schema() },
            "ready": { "type": "boolean" }
        },
        "required": ["checkedAt", "tools", "providers", "ready"]
    })
}

pub fn get_run() -> Value {
    json!({
        "type": "object",
        "description": "Stored handoff run when found, or a not-found message.",
        "properties": {
            "run": {
                "description": "Handoff run payload when resolved.",
                "oneOf": [
                    handoff_run_schema(),
                    { "type": "null" }
                ]
            },
            "runId": {
                "type": "string",
                "description": "Resolved run identifier when found."
            },
            "message": {
                "type": "string",
                "description": "Human-readable message when the run was not found."
            }
        }
    })
}

pub fn search_audit_log() -> Value {
    json!({
        "type": "object",
        "properties": {
            "records": { "type": "array", "items": audit_event_schema() },
            "count": { "type": "integer", "minimum": 0 }
        },
        "required": ["records", "count"]
    })
}

pub fn xai_research() -> Value {
    xai_research_result_schema()
}

pub fn dispatch_handoff() -> Value {
    handoff_run_schema()
}

pub fn execute_skill() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": { "type": "string" },
            "skillId": { "type": "string" },
            "skillName": { "type": "string" },
            "status": { "type": "string" },
            "auditRef": { "type": "string" },
            "createdAt": { "type": "string" },
            "output": { "type": "string" }
        },
        "required": ["id", "skillId", "skillName", "status", "auditRef", "createdAt", "output"]
    })
}

pub fn toggle_mcp_server() -> Value {
    json!({
        "type": "object",
        "properties": {
            "serverId": { "type": "string" },
            "serverName": { "type": "string" },
            "enabled": { "type": "boolean" },
            "configPath": { "type": "string" },
            "backupPath": { "type": "string" }
        },
        "required": ["serverId", "serverName", "enabled", "configPath", "backupPath"]
    })
}