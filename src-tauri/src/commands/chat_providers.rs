use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures_util::StreamExt;
use reqwest::header::{HeaderValue, CONTENT_TYPE};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::ipc::Channel;

use crate::mcp_server;
use crate::models::{ChatMessageInput, ChatStreamEvent};
use crate::commands::providers::{self, ProviderAdapter, ProviderDefinition};

const MAX_TOOL_TURNS: usize = 3;

#[derive(Debug, Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatCompletionChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChoice {
    message: ChatCompletionMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Deserialize)]
struct ToolCall {
    id: String,
    function: ToolCallFunction,
}

#[derive(Debug, Deserialize)]
struct ToolCallFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<AnthropicDelta>,
}

#[derive(Debug, Deserialize)]
struct AnthropicDelta {
    #[serde(default)]
    text: Option<String>,
}

pub async fn stream_provider_chat(
    definition: &ProviderDefinition,
    model_id: &str,
    messages: &[ChatMessageInput],
    enable_agent_tools: bool,
    on_event: &Channel<ChatStreamEvent>,
    cancelled: Arc<AtomicBool>,
) -> Result<(String, Option<String>), String> {
    match definition.adapter {
        ProviderAdapter::ClaudeCode => {
            stream_claude_code(messages, on_event, cancelled).await
        }
        ProviderAdapter::Anthropic => {
            stream_anthropic(definition, model_id, messages, on_event, cancelled).await
        }
        ProviderAdapter::OpenAiCompatible => {
            if definition.id == "xai" && enable_agent_tools {
                stream_openai_with_tools(definition, model_id, messages, on_event, cancelled)
                    .await
            } else {
                stream_openai_compatible(definition, model_id, messages, on_event, cancelled)
                    .await
            }
        }
    }
}

pub fn complete_provider_chat(
    definition: &ProviderDefinition,
    model_id: &str,
    messages: &[ChatMessageInput],
) -> Result<(String, Option<String>), String> {
    let prompt = messages
        .iter()
        .map(|message| format!("{}: {}", message.role, message.content))
        .collect::<Vec<_>>()
        .join("\n\n");
    providers::dispatch_provider_handoff(
        definition.id,
        model_id,
        "AgentDeck Chat",
        "Respond to the user conversation.",
        "",
        "agentdeck-chat",
        &prompt,
    )
}

async fn stream_openai_compatible(
    definition: &ProviderDefinition,
    model_id: &str,
    messages: &[ChatMessageInput],
    on_event: &Channel<ChatStreamEvent>,
    cancelled: Arc<AtomicBool>,
) -> Result<(String, Option<String>), String> {
    let base_url = providers::provider_base_url(definition);
    let mut headers = providers::build_provider_headers(definition)?.unwrap_or_default();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let client = async_client()?;
    let response = client
        .post(format!("{}/chat/completions", base_url.trim_end_matches('/')))
        .headers(headers)
        .json(&json!({
            "model": model_id,
            "messages": messages,
            "temperature": 0.4,
            "stream": true,
        }))
        .send()
        .await
        .map_err(|error| format!("chat stream failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("chat provider rejected stream request: {error}"))?;

    parse_openai_sse(response, on_event, cancelled).await
}

async fn stream_openai_with_tools(
    definition: &ProviderDefinition,
    model_id: &str,
    messages: &[ChatMessageInput],
    on_event: &Channel<ChatStreamEvent>,
    _cancelled: Arc<AtomicBool>,
) -> Result<(String, Option<String>), String> {
    let base_url = providers::provider_base_url(definition);
    let mut headers = providers::build_provider_headers(definition)?.unwrap_or_default();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let client = async_client()?;
    let mut conversation: Vec<Value> = messages
        .iter()
        .map(|message| {
            json!({
                "role": message.role,
                "content": message.content,
            })
        })
        .collect();

    for _ in 0..MAX_TOOL_TURNS {
        let response = client
            .post(format!("{}/chat/completions", base_url.trim_end_matches('/')))
            .headers(headers.clone())
            .json(&json!({
                "model": model_id,
                "messages": conversation,
                "temperature": 0.4,
                "stream": false,
                "tools": agentdeck_tool_definitions(),
            }))
            .send()
            .await
            .map_err(|error| format!("chat request failed: {error}"))?
            .error_for_status()
            .map_err(|error| format!("chat provider rejected request: {error}"))?
            .json::<ChatCompletionResponse>()
            .await
            .map_err(|error| format!("chat provider returned invalid data: {error}"))?;

        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| "chat provider returned no choices".to_owned())?;

        if choice.message.tool_calls.is_empty() {
            let content = choice.message.content.unwrap_or_default();
            if !content.is_empty() {
                let _ = on_event.send(ChatStreamEvent::Token {
                    content: content.clone(),
                });
            }
            return Ok((content, choice.finish_reason));
        }

        conversation.push(json!({
            "role": "assistant",
            "content": choice.message.content,
            "tool_calls": choice.message.tool_calls.iter().map(|tool_call| json!({
                "id": tool_call.id,
                "type": "function",
                "function": {
                    "name": tool_call.function.name,
                    "arguments": tool_call.function.arguments,
                }
            })).collect::<Vec<_>>(),
        }));

        for tool_call in choice.message.tool_calls {
            let args = serde_json::from_str::<Value>(&tool_call.function.arguments)
                .unwrap_or_else(|_| json!({}));
            let (result_value, _is_error) =
                mcp_server::execute_agentdeck_tool(&tool_call.function.name, args)?;
            let result = serde_json::to_string_pretty(&result_value)
                .map_err(|error| format!("failed to encode tool result: {error}"))?;
            let _ = on_event.send(ChatStreamEvent::Token {
                content: format!("\n[tool {}]\n", tool_call.function.name),
            });
            conversation.push(json!({
                "role": "tool",
                "tool_call_id": tool_call.id,
                "content": result,
            }));
        }
    }

    Err("tool-use loop exceeded maximum turns".to_owned())
}

async fn stream_anthropic(
    definition: &ProviderDefinition,
    model_id: &str,
    messages: &[ChatMessageInput],
    on_event: &Channel<ChatStreamEvent>,
    cancelled: Arc<AtomicBool>,
) -> Result<(String, Option<String>), String> {
    let base_url = providers::provider_base_url(definition);
    let mut headers = providers::build_provider_headers(definition)?.unwrap_or_default();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let (system, anthropic_messages) = split_anthropic_messages(messages);
    let client = async_client()?;
    let response = client
        .post(format!("{}/messages", base_url.trim_end_matches('/')))
        .headers(headers)
        .json(&json!({
            "model": model_id,
            "system": system,
            "messages": anthropic_messages,
            "max_tokens": 2048,
            "stream": true,
        }))
        .send()
        .await
        .map_err(|error| format!("anthropic stream failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("anthropic rejected stream request: {error}"))?;

    let mut stream = response.bytes_stream();
    let mut assembled = String::new();
    while let Some(chunk) = stream.next().await {
        if cancelled.load(Ordering::Relaxed) {
            return Err("chat stream cancelled".to_owned());
        }
        let chunk = chunk.map_err(|error| format!("anthropic stream read failed: {error}"))?;
        let text = String::from_utf8_lossy(&chunk);
        for line in text.lines() {
            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };
            if data == "[DONE]" {
                break;
            }
            let Ok(event) = serde_json::from_str::<AnthropicStreamEvent>(data) else {
                continue;
            };
            if event.event_type == "content_block_delta" {
                if let Some(token) = event.delta.and_then(|delta| delta.text).filter(|t| !t.is_empty())
                {
                    assembled.push_str(&token);
                    let _ = on_event.send(ChatStreamEvent::Token { content: token });
                }
            }
        }
    }
    if assembled.trim().is_empty() {
        return Err("anthropic returned an empty response".to_owned());
    }
    Ok((assembled, None))
}

async fn stream_claude_code(
    messages: &[ChatMessageInput],
    on_event: &Channel<ChatStreamEvent>,
    cancelled: Arc<AtomicBool>,
) -> Result<(String, Option<String>), String> {
    let prompt = messages
        .iter()
        .map(|message| format!("{}: {}", message.role, message.content))
        .collect::<Vec<_>>()
        .join("\n\n");

    let mut child = tokio::process::Command::new("claude")
        .args(["-p", &prompt])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to launch Claude Code: {error}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Claude Code stdout unavailable".to_owned())?;
    let mut reader = tokio::io::BufReader::new(stdout);
    let mut line = String::new();
    let mut assembled = String::new();

    loop {
        if cancelled.load(Ordering::Relaxed) {
            let _ = child.kill().await;
            return Err("chat stream cancelled".to_owned());
        }
        line.clear();
        let read = tokio::io::AsyncBufReadExt::read_line(&mut reader, &mut line)
            .await
            .map_err(|error| format!("failed to read Claude Code output: {error}"))?;
        if read == 0 {
            break;
        }
        assembled.push_str(&line);
        let _ = on_event.send(ChatStreamEvent::Token {
            content: line.clone(),
        });
    }

    let status = child
        .wait()
        .await
        .map_err(|error| format!("failed to wait for Claude Code: {error}"))?;
    if !status.success() {
        return Err(format!("Claude Code exited with status {status}"));
    }
    if assembled.trim().is_empty() {
        return Err("Claude Code returned an empty response".to_owned());
    }
    Ok((assembled, None))
}

async fn parse_openai_sse(
    response: reqwest::Response,
    on_event: &Channel<ChatStreamEvent>,
    cancelled: Arc<AtomicBool>,
) -> Result<(String, Option<String>), String> {
    let mut stream = response.bytes_stream();
    let mut assembled = String::new();
    let mut finish_reason = None;

    while let Some(chunk) = stream.next().await {
        if cancelled.load(Ordering::Relaxed) {
            return Err("chat stream cancelled".to_owned());
        }
        let chunk = chunk.map_err(|error| format!("chat stream read failed: {error}"))?;
        let text = String::from_utf8_lossy(&chunk);
        for line in text.lines() {
            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };
            if data == "[DONE]" {
                break;
            }
            let Ok(chunk) = serde_json::from_str::<StreamChunk>(data) else {
                continue;
            };
            if let Some(choice) = chunk.choices.first() {
                if let Some(token) = choice.delta.content.clone().filter(|value| !value.is_empty()) {
                    assembled.push_str(&token);
                    let _ = on_event.send(ChatStreamEvent::Token { content: token });
                }
                if choice.finish_reason.is_some() {
                    finish_reason = choice.finish_reason.clone();
                }
            }
        }
    }

    if assembled.trim().is_empty() {
        return Err("chat provider returned an empty response".to_owned());
    }
    Ok((assembled, finish_reason))
}

fn split_anthropic_messages(messages: &[ChatMessageInput]) -> (String, Vec<Value>) {
    let mut system_parts = Vec::new();
    let mut converted = Vec::new();
    for message in messages {
        if message.role == "system" {
            system_parts.push(message.content.clone());
            continue;
        }
        let role = if message.role == "assistant" {
            "assistant"
        } else {
            "user"
        };
        converted.push(json!({
            "role": role,
            "content": message.content,
        }));
    }
    (
        system_parts.join("\n\n"),
        converted,
    )
}

fn agentdeck_tool_definitions() -> Vec<Value> {
    vec![
        tool_definition("agentdeck.scan_environment", "Return the current local environment scan."),
        tool_definition("agentdeck.get_graph", "Return a graph snapshot from the current scan."),
        tool_definition("agentdeck.list_agents", "List discovered local agents and their status."),
        tool_definition(
            "agentdeck.list_mcp_servers",
            "List configured MCP servers from local inventory.",
        ),
        tool_definition("agentdeck.health_check", "Run the AgentDeck preflight health check."),
        tool_definition("agentdeck.get_run", "Fetch a handoff run by ID."),
        tool_definition("agentdeck.search_audit_log", "Search the local audit log."),
    ]
}

fn tool_definition(name: &str, description: &str) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": {
                "type": "object",
                "properties": {}
            }
        }
    })
}

fn async_client() -> Result<Client, String> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .no_proxy()
        .build()
        .map_err(|error| format!("failed to create async HTTP client: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agentdeck_tool_definitions_include_core_tools() {
        let tools = agentdeck_tool_definitions();
        let names: Vec<_> = tools
            .iter()
            .filter_map(|tool| {
                tool.get("function")
                    .and_then(|function| function.get("name"))
                    .and_then(Value::as_str)
            })
            .collect();
        assert!(names.contains(&"agentdeck.list_agents"));
        assert!(names.contains(&"agentdeck.scan_environment"));
    }

    #[test]
    fn splits_anthropic_system_messages() {
        let messages = vec![
            ChatMessageInput {
                role: "system".to_owned(),
                content: "Be concise".to_owned(),
            },
            ChatMessageInput {
                role: "user".to_owned(),
                content: "Hello".to_owned(),
            },
        ];
        let (system, converted) = split_anthropic_messages(&messages);
        assert_eq!(system, "Be concise");
        assert_eq!(converted.len(), 1);
    }
}