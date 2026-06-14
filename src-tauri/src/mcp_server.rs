use std::collections::BTreeSet;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::commands;
use crate::commands::{handoffs, mcp, plugins};
use crate::models::{
    AuditEventRecord, DiscoveredEntity, EnvironmentScan, GraphEdge, GraphNode, GraphSnapshot,
    HandoffRequest, HandoffRun, McpToggleResult, SkillExecutionRecord,
};
use crate::permissions;
use crate::storage;
use crate::xai_research;

const SERVER_NAME: &str = "AgentDeck";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_PROTOCOL_VERSION: &str = "2025-06-18";
const LEGACY_PROTOCOL_VERSION: &str = "2025-03-26";
const MAX_SEARCH_LIMIT: usize = 100;
const CHATGPT_SUBMISSION_MANIFEST: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../chatgpt-app-submission.json"));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpToolProfile {
    Full,
    ReadOnlyV1_1,
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[serde(default)]
    #[serde(rename = "jsonrpc")]
    _jsonrpc: Option<String>,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

pub fn run_stdio() -> Result<(), String> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let lock = stdin.lock();

    for line in lock.lines() {
        let line = line.map_err(|error| format!("failed to read MCP request: {error}"))?;
        if line.trim().is_empty() {
            continue;
        }

        match process_request_line(&line) {
            Ok(Some(response)) => {
                let payload = serde_json::to_string(&response)
                    .map_err(|error| format!("failed to encode MCP response: {error}"))?;
                stdout
                    .write_all(payload.as_bytes())
                    .and_then(|_| stdout.write_all(b"\n"))
                    .and_then(|_| stdout.flush())
                    .map_err(|error| format!("failed to write MCP response: {error}"))?;
            }
            Ok(None) => {}
            Err(error) => {
                let response = jsonrpc_error(
                    None,
                    -32603,
                    "Internal error",
                    Some(json!({ "message": error })),
                );
                let payload = serde_json::to_string(&response).map_err(|encode_error| {
                    format!("failed to encode MCP error: {encode_error}")
                })?;
                stdout
                    .write_all(payload.as_bytes())
                    .and_then(|_| stdout.write_all(b"\n"))
                    .and_then(|_| stdout.flush())
                    .map_err(|write_error| format!("failed to write MCP error: {write_error}"))?;
            }
        }
    }

    Ok(())
}

pub fn process_request_line(line: &str) -> Result<Option<Value>, String> {
    process_request_line_with_profile(line, McpToolProfile::Full)
}

pub fn process_request_line_with_profile(
    line: &str,
    profile: McpToolProfile,
) -> Result<Option<Value>, String> {
    let value: Value =
        serde_json::from_str(line).map_err(|error| format!("invalid JSON-RPC payload: {error}"))?;
    process_request_value_with_profile(value, profile)
}

pub fn process_request_value(value: Value) -> Result<Option<Value>, String> {
    process_request_value_with_profile(value, McpToolProfile::Full)
}

pub fn process_request_value_with_profile(
    value: Value,
    profile: McpToolProfile,
) -> Result<Option<Value>, String> {
    match value {
        Value::Array(items) => {
            let responses = items
                .into_iter()
                .filter_map(|item| handle_request(item, profile))
                .collect::<Vec<_>>();
            if responses.is_empty() {
                Ok(None)
            } else {
                Ok(Some(Value::Array(responses)))
            }
        }
        other => Ok(handle_request(other, profile)),
    }
}

fn handle_request(value: Value, profile: McpToolProfile) -> Option<Value> {
    let request: JsonRpcRequest = match serde_json::from_value(value) {
        Ok(request) => request,
        Err(error) => {
            return Some(jsonrpc_error(
                None,
                -32600,
                "Invalid Request",
                Some(json!({ "message": error.to_string() })),
            ));
        }
    };

    let request_id = request.id.clone();
    let result = match request.method.as_str() {
        "initialize" => handle_initialize(request.params, profile),
        "notifications/initialized" => return None,
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools_list(profile) })),
        "tools/call" => handle_tool_call(request.params, request_id.clone(), profile),
        other => Err(jsonrpc_error(
            request_id.clone(),
            -32601,
            "Method not found",
            Some(json!({ "method": other })),
        )),
    };

    Some(match result {
        Ok(result) => jsonrpc_result(request_id.unwrap_or(Value::Null), result),
        Err(response) => response,
    })
}

fn handle_initialize(params: Option<Value>, profile: McpToolProfile) -> Result<Value, Value> {
    let client_version = params
        .as_ref()
        .and_then(|value| value.get("protocolVersion"))
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_PROTOCOL_VERSION);
    let negotiated_version = if matches!(
        client_version,
        DEFAULT_PROTOCOL_VERSION | LEGACY_PROTOCOL_VERSION
    ) {
        client_version
    } else {
        DEFAULT_PROTOCOL_VERSION
    };

    let instructions = match profile {
        McpToolProfile::Full => {
            "AgentDeck exposes local control plane tools. Write tools require callerAgentId and respect per-agent permissions."
        }
        McpToolProfile::ReadOnlyV1_1 => {
            "AgentDeck exposes read-only local inspection tools plus xAI research helpers: agentdeck.xai_research_search_web, agentdeck.xai_research_answer_with_sources, and agentdeck.xai_research_summarize_url. Research tools require an xAI API key saved in AgentDeck Settings. Write tools are excluded from this ChatGPT connector profile."
        }
    };

    Ok(json!({
        "protocolVersion": negotiated_version,
        "capabilities": {
            "tools": {},
        },
        "serverInfo": {
            "name": SERVER_NAME,
            "version": SERVER_VERSION,
        },
        "instructions": instructions
    }))
}

fn handle_tool_call(
    params: Option<Value>,
    request_id: Option<Value>,
    profile: McpToolProfile,
) -> Result<Value, Value> {
    let Some(params) = params else {
        return Err(jsonrpc_error(
            request_id,
            -32602,
            "Invalid params",
            Some(json!({ "message": "missing tool arguments" })),
        ));
    };

    let tool_name = params.get("name").and_then(Value::as_str).ok_or_else(|| {
        jsonrpc_error(
            request_id.clone(),
            -32602,
            "Invalid params",
            Some(json!({ "message": "missing tool name" })),
        )
    })?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    if !tool_allowed_for_profile(profile, tool_name) {
        return Err(jsonrpc_error(
            request_id,
            -32602,
            "Invalid params",
            Some(json!({
                "message": format!(
                    "tool {tool_name} is not exposed in the ChatGPT read-only connector profile; refresh the AgentDeck connector in ChatGPT Apps settings"
                )
            })),
        ));
    }

    match execute_agentdeck_tool(tool_name, arguments) {
        Ok((text, is_error)) => Ok(tool_content_from_text(&text, is_error)),
        Err(message) => Err(jsonrpc_error(
            request_id,
            -32603,
            "Internal error",
            Some(json!({ "message": message })),
        )),
    }
}

pub fn request_is_notification_only(body: &str) -> bool {
    let value: Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => return false,
    };

    match value {
        Value::Array(items) => !items.is_empty() && items.iter().all(message_is_notification),
        other => message_is_notification(&other),
    }
}

fn message_is_notification(value: &Value) -> bool {
    value.get("method").and_then(Value::as_str).is_some() && value.get("id").is_none()
}

pub fn truncate_tool_response(value: Value, notice: &str) -> Value {
    let Some(result) = value.get("result").cloned() else {
        return value;
    };
    let Some(text) = result
        .get("content")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("text"))
        .and_then(Value::as_str)
    else {
        return value;
    };

    let mut truncated = text.chars().take(60_000).collect::<String>();
    truncated.push_str("\n\n");
    truncated.push_str(notice);

    let mut next = value;
    next["result"] = tool_content_from_text(&truncated, false);
    next
}

pub fn execute_agentdeck_tool(tool_name: &str, arguments: Value) -> Result<(String, bool), String> {
    let caller = caller_agent_id(&arguments);
    let database_path = database_path()?;
    let connection = storage::open_database(&database_path)?;
    permissions::require_permission(&connection, &caller, "call-mcp-tool")?;

    let value = match tool_name {
        "agentdeck.scan_environment" => serde_json::to_value(commands::scan_environment())
            .map_err(|error| format!("failed to serialize scan result: {error}"))?,
        "agentdeck.get_graph" => serde_json::to_value(get_graph_snapshot()?)
            .map_err(|error| format!("failed to serialize graph result: {error}"))?,
        "agentdeck.list_agents" => serde_json::to_value(list_agents()?)
            .map_err(|error| format!("failed to serialize agent list: {error}"))?,
        "agentdeck.list_mcp_servers" => {
            serde_json::to_value(commands::mcp::scan_inventory())
                .map_err(|error| format!("failed to serialize MCP inventory: {error}"))?
        }
        "agentdeck.health_check" => serde_json::to_value(commands::run_preflight())
            .map_err(|error| format!("failed to serialize health check: {error}"))?,
        "agentdeck.get_run" => match get_run(arguments) {
            Ok(value) => value,
            Err(message) if message.contains("not found") => json!({
                "run": null,
                "message": message,
            }),
            Err(message) => return Err(message),
        },
        "agentdeck.search_audit_log" => serde_json::to_value(search_audit_log(arguments)?)
            .map_err(|error| format!("failed to serialize audit search: {error}"))?,
        "agentdeck.xai_research_search_web"
        | "agentdeck.xai_research_answer_with_sources"
        | "agentdeck.xai_research_summarize_url" => {
            xai_research::execute_tool(&database_path, tool_name, &arguments)?
        }
        "agentdeck.dispatch_handoff" => serde_json::to_value(dispatch_handoff_tool(
            &database_path,
            arguments,
        )?)
        .map_err(|error| format!("failed to serialize handoff result: {error}"))?,
        "agentdeck.execute_skill" => {
            permissions::require_permission(&connection, &caller, "execute-skill")?;
            serde_json::to_value(execute_skill_tool(&database_path, arguments)?)
                .map_err(|error| format!("failed to serialize skill result: {error}"))?
        }
        "agentdeck.toggle_mcp_server" => {
            permissions::require_permission(&connection, &caller, "write-config")?;
            serde_json::to_value(toggle_mcp_server_tool(arguments)?)
                .map_err(|error| format!("failed to serialize MCP toggle result: {error}"))?
        }
        other => return Err(format!("unknown AgentDeck tool: {other}")),
    };
    let is_error = tool_name == "agentdeck.get_run"
        && value.get("run").is_some_and(Value::is_null)
        && value.get("message").is_some();
    let text = serde_json::to_string_pretty(&value)
        .map_err(|error| format!("failed to encode tool result: {error}"))?;
    Ok((text, is_error))
}

fn caller_agent_id(arguments: &Value) -> String {
    arguments
        .get("callerAgentId")
        .and_then(Value::as_str)
        .unwrap_or("agent:agentdeck")
        .to_owned()
}

fn dispatch_handoff_tool(path: &Path, arguments: Value) -> Result<HandoffRun, String> {
    let request: HandoffRequest = serde_json::from_value(arguments)
        .map_err(|error| format!("invalid handoff arguments: {error}"))?;
    handoffs::dispatch_handoff(path, request)
}

fn execute_skill_tool(path: &Path, arguments: Value) -> Result<SkillExecutionRecord, String> {
    let skill_id = arguments
        .get("skillId")
        .and_then(Value::as_str)
        .ok_or_else(|| "skillId is required".to_owned())?;
    storage::validate_identifier("skill ID", skill_id)?;
    plugins::execute_skill_pipeline(path, skill_id)
}

fn toggle_mcp_server_tool(arguments: Value) -> Result<McpToggleResult, String> {
    let server_id = arguments
        .get("serverId")
        .and_then(Value::as_str)
        .ok_or_else(|| "serverId is required".to_owned())?;
    let enabled = arguments
        .get("enabled")
        .and_then(Value::as_bool)
        .ok_or_else(|| "enabled is required".to_owned())?;
    storage::validate_identifier("server ID", server_id)?;
    mcp::toggle_server_config(server_id, enabled)
}

fn tool_content_from_text(text: &str, is_error: bool) -> Value {
    json!({
        "content": [
            {
                "type": "text",
                "text": text,
            }
        ],
        "isError": is_error,
    })
}



fn chatgpt_submission_tool_names() -> &'static BTreeSet<String> {
    static NAMES: OnceLock<BTreeSet<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let manifest: Value = serde_json::from_str(CHATGPT_SUBMISSION_MANIFEST)
            .expect("chatgpt-app-submission.json must be valid JSON");
        manifest
            .get("tools")
            .and_then(Value::as_object)
            .expect("chatgpt-app-submission.json must include tools")
            .keys()
            .cloned()
            .collect()
    })
}

fn tool_allowed_for_profile(profile: McpToolProfile, tool_name: &str) -> bool {
    match profile {
        McpToolProfile::Full => true,
        McpToolProfile::ReadOnlyV1_1 => chatgpt_submission_tool_names().contains(tool_name),
    }
}

fn tools_list(profile: McpToolProfile) -> Vec<Value> {
    tools_list_all()
        .into_iter()
        .filter(|tool| {
            tool.get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| tool_allowed_for_profile(profile, name))
        })
        .collect()
}

fn tools_list_all() -> Vec<Value> {
    vec![
        tool_definition(
            "agentdeck.scan_environment",
            "Return the current local environment scan, including tools, providers, processes, configs, and entities.",
            json!({"type":"object","properties":{}}),
            true,
        ),
        tool_definition(
            "agentdeck.get_graph",
            "Return a graph snapshot derived from the current environment scan.",
            json!({"type":"object","properties":{}}),
            true,
        ),
        tool_definition(
            "agentdeck.list_agents",
            "Return the discovered local agents from the current environment scan.",
            json!({"type":"object","properties":{}}),
            true,
        ),
        tool_definition(
            "agentdeck.list_mcp_servers",
            "Return the read-only MCP inventory for local config files.",
            json!({"type":"object","properties":{}}),
            true,
        ),
        tool_definition(
            "agentdeck.health_check",
            "Run the local preflight checks and return readiness status.",
            json!({"type":"object","properties":{}}),
            true,
        ),
        tool_definition(
            "agentdeck.get_run",
            "Fetch a stored handoff run by runId, auditId, or conversationId. Audit search rows for handoff.dispatch include runId when available.",
            json!({
                "type": "object",
                "properties": {
                    "runId": { "type": "string" },
                    "auditId": { "type": "string" },
                    "auditRef": { "type": "string" },
                    "conversationId": { "type": "string" },
                    "threadId": { "type": "string" }
                },
                "additionalProperties": false
            }),
            true,
        ),
        tool_definition(
            "agentdeck.search_audit_log",
            "Search stored audit events. Multiple terms and OR are supported, for example \"handoff OR provider\" or \"handoff provider\".",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 100 }
                },
                "additionalProperties": false
            }),
            true,
        ),
        tool_definition_open_world(
            "agentdeck.xai_research_search_web",
            "Search the current public web and return a concise evidence summary with source URLs.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "maxSources": { "type": "integer", "minimum": 1, "maximum": 20 }
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        ),
        tool_definition_open_world(
            "agentdeck.xai_research_answer_with_sources",
            "Answer a question using current web research and return the answer with source URLs.",
            json!({
                "type": "object",
                "properties": {
                    "question": { "type": "string" },
                    "maxSources": { "type": "integer", "minimum": 1, "maximum": 20 }
                },
                "required": ["question"],
                "additionalProperties": false
            }),
        ),
        tool_definition_open_world(
            "agentdeck.xai_research_summarize_url",
            "Read and summarize a public HTTP or HTTPS URL, preserving the source URL and relevant citations.",
            json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string" },
                    "focus": { "type": "string" },
                    "maxSources": { "type": "integer", "minimum": 1, "maximum": 20 }
                },
                "required": ["url"],
                "additionalProperties": false
            }),
        ),
        tool_definition(
            "agentdeck.dispatch_handoff",
            "Dispatch an approved handoff to a target provider.",
            json!({
                "type": "object",
                "properties": {
                    "callerAgentId": { "type": "string" },
                    "sourceAgentId": { "type": "string" },
                    "sourceAgentName": { "type": "string" },
                    "targetProviderId": { "type": "string" },
                    "targetProviderName": { "type": "string" },
                    "targetModelId": { "type": "string" },
                    "title": { "type": "string" },
                    "task": { "type": "string" },
                    "context": { "type": "string" },
                    "approvals": {
                        "type": "array",
                        "items": { "type": "string" },
                        "minItems": 1
                    }
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
            }),
            false,
        ),
        tool_definition(
            "agentdeck.execute_skill",
            "Execute a registered AgentDeck skill pipeline.",
            json!({
                "type": "object",
                "properties": {
                    "callerAgentId": { "type": "string" },
                    "skillId": { "type": "string" }
                },
                "required": ["skillId"],
                "additionalProperties": false
            }),
            false,
        ),
        tool_definition(
            "agentdeck.toggle_mcp_server",
            "Enable or disable an MCP server in a local JSON config file with backup/restore safety.",
            json!({
                "type": "object",
                "properties": {
                    "callerAgentId": { "type": "string" },
                    "serverId": { "type": "string" },
                    "enabled": { "type": "boolean" }
                },
                "required": ["serverId", "enabled"],
                "additionalProperties": false
            }),
            false,
        ),
    ]
}

fn tool_definition(
    name: &str,
    description: &str,
    input_schema: Value,
    read_only: bool,
) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
        "annotations": {
            "readOnlyHint": read_only,
            "openWorldHint": false,
            "destructiveHint": !read_only
        }
    })
}

fn tool_definition_open_world(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
        "annotations": {
            "readOnlyHint": true,
            "openWorldHint": true,
            "destructiveHint": false
        }
    })
}

fn get_graph_snapshot() -> Result<GraphSnapshot, String> {
    let scan = commands::scan_environment();
    Ok(build_graph_snapshot(&scan))
}

fn build_graph_snapshot(scan: &EnvironmentScan) -> GraphSnapshot {
    let nodes = scan
        .entities
        .iter()
        .map(|entity| GraphNode {
            id: entity.id.clone(),
            label: entity.name.clone(),
            entity_type: entity.entity_type.clone(),
            status: entity.status.clone(),
            source: entity.source.clone(),
        })
        .collect::<Vec<_>>();
    let edges = build_graph_edges(&scan.entities);

    GraphSnapshot {
        scanned_at: scan.scanned_at.clone(),
        nodes,
        edges,
    }
}

fn build_graph_edges(entities: &[DiscoveredEntity]) -> Vec<GraphEdge> {
    let entity_ids = entities
        .iter()
        .map(|entity| entity.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let agents = entities
        .iter()
        .filter(|entity| entity.entity_type == "agent")
        .collect::<Vec<_>>();
    let configs = entities
        .iter()
        .filter(|entity| entity.entity_type == "config")
        .collect::<Vec<_>>();
    let processes = entities
        .iter()
        .filter(|entity| entity.entity_type == "process")
        .collect::<Vec<_>>();

    let mut edges = Vec::new();
    for agent in agents {
        if let Some(command) = agent.metadata.get("command") {
            let tool_id = format!("tool:{command}");
            if entity_ids.contains(tool_id.as_str()) {
                edges.push(edge(&agent.id, &tool_id, "uses"));
            }
        }

        for config in configs.iter().filter(|config| config.name == agent.name) {
            edges.push(edge(&agent.id, &config.id, "configured by"));
        }

        let aliases = agent_aliases(&agent.id);
        for process in processes
            .iter()
            .filter(|process| matches_aliases(process, &aliases))
        {
            edges.push(edge(&agent.id, &process.id, "runs as"));
        }
    }

    let provider_id = "provider:lmstudio:http-localhost-1234-v1";
    if entity_ids.contains(provider_id) {
        for tool_id in ["tool:lms", "tool:lmstudio"] {
            if entity_ids.contains(tool_id) {
                edges.push(edge(provider_id, tool_id, "managed by"));
            }
        }
        let aliases = provider_aliases(provider_id);
        for process in processes
            .iter()
            .filter(|process| matches_aliases(process, &aliases))
        {
            edges.push(edge(provider_id, &process.id, "runs as"));
        }
    }

    unique_edges(edges)
}

fn agent_aliases(agent_id: &str) -> Vec<&'static str> {
    match agent_id {
        "agent:codex" => vec!["codex"],
        "agent:claude-code" => vec!["claude"],
        "agent:hermes" => vec!["hermes"],
        "agent:openclaw" => vec!["openclaw"],
        "agent:lm-studio" => vec!["lm studio", "lmstudio", "lms"],
        "agent:grok" => vec!["grok", "xai"],
        _ => Vec::new(),
    }
}

fn provider_aliases(provider_id: &str) -> Vec<&'static str> {
    match provider_id {
        "provider:lmstudio:http-localhost-1234-v1" => vec!["lm studio", "lmstudio", "lms"],
        _ => Vec::new(),
    }
}

fn matches_aliases(entity: &DiscoveredEntity, aliases: &[&str]) -> bool {
    let searchable = std::iter::once(entity.name.as_str())
        .chain(std::iter::once(entity.source.as_str()))
        .chain(entity.metadata.values().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    aliases.iter().any(|alias| searchable.contains(alias))
}

fn edge(source: &str, target: &str, label: &str) -> GraphEdge {
    GraphEdge {
        id: format!("{source}->{target}:{label}"),
        source: source.to_owned(),
        target: target.to_owned(),
        label: label.to_owned(),
    }
}

fn unique_edges(edges: Vec<GraphEdge>) -> Vec<GraphEdge> {
    let mut seen = std::collections::BTreeMap::new();
    for edge in edges {
        seen.insert(edge.id.clone(), edge);
    }
    seen.into_values().collect()
}

fn list_agents() -> Result<Value, String> {
    let scan = commands::scan_environment();
    let agents = scan
        .entities
        .into_iter()
        .filter(|entity| entity.entity_type == "agent")
        .collect::<Vec<_>>();
    Ok(json!({
        "agents": agents,
        "count": agents.len()
    }))
}

fn get_run(arguments: Value) -> Result<Value, String> {
    let run_id = arguments.get("runId").and_then(Value::as_str);
    let audit_id = arguments
        .get("auditId")
        .or_else(|| arguments.get("auditRef"))
        .and_then(Value::as_str);
    let conversation_id = arguments
        .get("conversationId")
        .or_else(|| arguments.get("threadId"))
        .and_then(Value::as_str);
    let connection = storage::open_database(&database_path()?)?;
    let resolved_run_id = storage::resolve_handoff_run_id(
        &connection,
        run_id,
        audit_id,
        conversation_id,
    )?;
    let run = storage::load_handoff_run(&connection, &resolved_run_id)?;
    Ok(json!({ "run": run, "runId": resolved_run_id }))
}

fn search_audit_log(arguments: Value) -> Result<Value, String> {
    let query = arguments
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_owned();
    let limit = arguments
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(20)
        .clamp(1, MAX_SEARCH_LIMIT as u64) as usize;
    let connection = storage::open_database(&database_path()?)?;
    let records = load_audit_events(&connection, &query, limit)?;
    Ok(json!({
        "records": records,
        "count": records.len()
    }))
}

fn load_audit_events(
    connection: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<AuditEventRecord>, String> {
    let tokens = parse_audit_query_tokens(query);
    let mut records = if tokens.is_empty() {
        let mut statement = connection
            .prepare(
                "SELECT id, action, status, model, conversation_id, duration_ms, created_at
                 FROM audit_events
                 ORDER BY created_at DESC
                 LIMIT ?1",
            )
            .map_err(|error| format!("failed to prepare audit query: {error}"))?;
        let rows = statement
            .query_map([limit as i64], storage::map_audit_row)
            .map_err(|error| format!("failed to load audit events: {error}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to decode audit events: {error}"))?
    } else {
        let mut collected = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for token in tokens {
            let pattern = format!("%{token}%");
            let mut statement = connection
                .prepare(
                    "SELECT id, action, status, model, conversation_id, duration_ms, created_at
                     FROM audit_events
                     WHERE id LIKE ?1 COLLATE NOCASE
                        OR action LIKE ?1 COLLATE NOCASE
                        OR status LIKE ?1 COLLATE NOCASE
                        OR model LIKE ?1 COLLATE NOCASE
                        OR conversation_id LIKE ?1 COLLATE NOCASE
                     ORDER BY created_at DESC
                     LIMIT ?2",
                )
                .map_err(|error| format!("failed to prepare audit query: {error}"))?;
            let rows = statement
                .query_map(params![pattern, limit as i64], storage::map_audit_row)
                .map_err(|error| format!("failed to load audit events: {error}"))?;
            for record in rows.flatten() {
                if seen.insert(record.id.clone()) {
                    collected.push(record);
                }
            }
        }
        collected.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        collected.truncate(limit);
        collected
    };

    for record in &mut records {
        if record.action == "handoff.dispatch" {
            record.run_id = storage::lookup_handoff_run_id_by_audit_ref(connection, &record.id)?
                .or_else(|| {
                    storage::lookup_handoff_run_id_by_thread_id(connection, &record.conversation_id)
                        .ok()
                        .flatten()
                });
        }
    }

    Ok(records)
}

fn parse_audit_query_tokens(query: &str) -> Vec<String> {
    let mut normalized = query.replace('|', " ");
    for separator in [" OR ", " or ", " Or ", " oR "] {
        normalized = normalized.replace(separator, " ");
    }
    normalized
        .split_whitespace()
        .filter(|token| !token.is_empty() && !token.eq_ignore_ascii_case("or"))
        .map(str::to_owned)
        .take(8)
        .collect()
}

fn database_path() -> Result<PathBuf, String> {
    storage::resolve_database_path(None)
}

pub fn internal_error_response(id: Option<Value>, message: &str) -> Value {
    jsonrpc_error(
        id,
        -32603,
        "Internal error",
        Some(json!({ "message": message })),
    )
}

fn jsonrpc_result(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn jsonrpc_error(id: Option<Value>, code: i64, message: &str, data: Option<Value>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "error": JsonRpcError {
            code,
            message: message.to_owned(),
            data,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment_scan() -> EnvironmentScan {
        EnvironmentScan {
            scanned_at: "2026-06-08T00:00:00Z".to_owned(),
            project: None,
            tools: vec![],
            providers: vec![],
            processes: vec![],
            configs: vec![],
            entities: vec![
                DiscoveredEntity {
                    id: "agent:codex".to_owned(),
                    entity_type: "agent".to_owned(),
                    name: "Codex".to_owned(),
                    status: "running".to_owned(),
                    source: "command".to_owned(),
                    metadata: std::collections::BTreeMap::from([(
                        "command".to_owned(),
                        "codex".to_owned(),
                    )]),
                },
                DiscoveredEntity {
                    id: "tool:codex".to_owned(),
                    entity_type: "tool".to_owned(),
                    name: "codex".to_owned(),
                    status: "available".to_owned(),
                    source: "command".to_owned(),
                    metadata: std::collections::BTreeMap::new(),
                },
            ],
        }
    }

    #[test]
    fn builds_graph_snapshot_from_environment_scan() {
        let graph = build_graph_snapshot(&environment_scan());
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].label, "uses");
    }

    #[test]
    fn lists_tools() {
        let tools = tools_list(McpToolProfile::Full);
        let names = tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert!(names.contains(&"agentdeck.scan_environment"));
        assert!(names.contains(&"agentdeck.xai_research_search_web"));
        assert!(names.contains(&"agentdeck.dispatch_handoff"));
        assert!(names.contains(&"agentdeck.execute_skill"));
        assert!(names.contains(&"agentdeck.toggle_mcp_server"));
    }

    #[test]
    fn chatgpt_http_profile_exposes_submission_tools_only() {
        let tools = tools_list(McpToolProfile::ReadOnlyV1_1);
        let names = tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), chatgpt_submission_tool_names().len());
        assert!(names.contains("agentdeck.xai_research_search_web"));
        assert!(names.contains("agentdeck.xai_research_answer_with_sources"));
        assert!(names.contains("agentdeck.xai_research_summarize_url"));
        assert!(!names.contains("agentdeck.dispatch_handoff"));
        assert!(!names.contains("agentdeck.execute_skill"));
        assert!(!names.contains("agentdeck.toggle_mcp_server"));
    }

    #[test]
    fn rejects_unknown_run_ids() {
        let result = get_run(json!({"runId":"run:test"}));
        assert!(result.is_err());
    }

    #[test]
    fn rejects_get_run_without_identifier() {
        let result = get_run(json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn audit_query_tokens_split_or_terms() {
        let tokens = parse_audit_query_tokens("handoff OR provider");
        assert_eq!(tokens, vec!["handoff".to_owned(), "provider".to_owned()]);
    }

    #[test]
    fn detects_notification_only_payloads() {
        assert!(request_is_notification_only(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#
        ));
        assert!(!request_is_notification_only(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#
        ));
    }

    #[test]
    fn search_limit_defaults_within_bounds() {
        let arguments = json!({});
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(20)
            .clamp(1, MAX_SEARCH_LIMIT as u64);
        assert_eq!(limit, 20);
    }

    #[test]
    fn chatgpt_submission_tools_match_read_only_mcp_surface() {
        let manifest_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crate parent directory")
            .join("chatgpt-app-submission.json");
        let manifest_raw = std::fs::read_to_string(&manifest_path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", manifest_path.display()));
        let manifest: Value = serde_json::from_str(&manifest_raw)
            .expect("chatgpt-app-submission.json must be valid JSON");
        let submitted = manifest
            .get("tools")
            .and_then(Value::as_object)
            .expect("submission must include tools object");
        let live_tools = tools_list(McpToolProfile::Full);
        let live_names = live_tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect::<std::collections::BTreeSet<_>>();

        for tool_name in submitted.keys() {
            assert!(
                live_names.contains(tool_name.as_str()),
                "submitted tool {tool_name} is not exposed by the MCP server"
            );
            let annotations = submitted
                .get(tool_name)
                .and_then(|entry| entry.get("annotations"))
                .and_then(Value::as_object)
                .expect("tool annotations");
            assert_eq!(
                annotations.get("readOnlyHint").and_then(Value::as_bool),
                Some(true),
                "{tool_name} must be read-only in the ChatGPT submission profile"
            );
        }

        let deferred = manifest
            .get("submission_profile")
            .and_then(|profile| profile.get("deferred_tools"))
            .and_then(Value::as_array)
            .expect("submission_profile.deferred_tools");
        for tool_name in deferred {
            let name = tool_name.as_str().expect("deferred tool name");
            assert!(
                live_names.contains(name),
                "deferred tool {name} must still exist in the MCP server for developer mode"
            );
        }

        assert!(submitted.len() >= 7);
    }
}
