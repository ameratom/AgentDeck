use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures_util::StreamExt;
use reqwest::header::{HeaderValue, CONTENT_TYPE};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::ipc::Channel;

use crate::commands::providers::{self, ProviderAdapter, ProviderDefinition};
use crate::mcp_server;
use crate::models::{ChatMessageInput, ChatStreamEvent};

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
        ProviderAdapter::ClaudeCode => stream_claude_code(messages, on_event, cancelled).await,
        ProviderAdapter::CodexCli => {
            stream_codex_cli(model_id, messages, on_event, cancelled).await
        }
        ProviderAdapter::Anthropic => {
            stream_anthropic(definition, model_id, messages, on_event, cancelled).await
        }
        ProviderAdapter::OpenAiCompatible => {
            if definition.id == "xai" && enable_agent_tools {
                stream_openai_with_tools(definition, model_id, messages, on_event, cancelled).await
            } else if providers::uses_responses_api(
                definition,
                &providers::provider_base_url(definition),
            ) {
                stream_openai_responses(definition, model_id, messages, on_event, cancelled).await
            } else {
                stream_openai_compatible(definition, model_id, messages, on_event, cancelled).await
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

async fn stream_openai_responses(
    definition: &ProviderDefinition,
    model_id: &str,
    messages: &[ChatMessageInput],
    on_event: &Channel<ChatStreamEvent>,
    cancelled: Arc<AtomicBool>,
) -> Result<(String, Option<String>), String> {
    let provider_id = definition.id.to_owned();
    let model_id = model_id.to_owned();
    let messages = messages.to_vec();
    let (text, finish_reason) = tauri::async_runtime::spawn_blocking(move || {
        let definition = providers::find_provider(&provider_id)?;
        let base_url = providers::provider_base_url(&definition);
        providers::complete_openai_responses_chat(&definition, &base_url, &model_id, &messages)
    })
    .await
    .map_err(|error| format!("responses chat task failed: {error}"))??;

    if cancelled.load(Ordering::Relaxed) {
        return Err("chat stream cancelled".to_owned());
    }
    if text.trim().is_empty() {
        return Err("chat provider returned an empty response".to_owned());
    }

    let mut assembled = String::new();
    for word in text.split_inclusive(char::is_whitespace) {
        if cancelled.load(Ordering::Relaxed) {
            return Err("chat stream cancelled".to_owned());
        }
        assembled.push_str(word);
        let _ = on_event.send(ChatStreamEvent::Token {
            content: word.to_owned(),
        });
    }

    Ok((assembled, finish_reason))
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
        .post(format!(
            "{}/chat/completions",
            base_url.trim_end_matches('/')
        ))
        .headers(headers)
        .json(&json!({
            "model": model_id,
            "messages": messages,
            "temperature": 0.4,
            "stream": true,
        }))
        .send()
        .await
        .map_err(|error| format!("chat stream failed: {error}"))?;
    if let Err(error) = response.error_for_status_ref() {
        let detail = response
            .text()
            .await
            .unwrap_or_else(|_| "unable to read response body".to_owned());
        return Err(providers::format_provider_http_error(
            "chat provider rejected stream request",
            &error,
            &detail,
        ));
    }

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
            .post(format!(
                "{}/chat/completions",
                base_url.trim_end_matches('/')
            ))
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
    if !headers.contains_key("anthropic-version") {
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
    }
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let (system, anthropic_messages) = prepare_anthropic_messages(messages)?;
    let client = async_client()?;
    let mut body = json!({
        "model": model_id,
        "messages": anthropic_messages,
        "max_tokens": 2048,
        "stream": true,
    });
    if !system.is_empty() {
        body["system"] = json!(system);
    }
    let response = client
        .post(format!("{}/messages", base_url.trim_end_matches('/')))
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|error| format!("anthropic stream failed: {error}"))?;
    if let Err(error) = response.error_for_status_ref() {
        let detail = response
            .text()
            .await
            .unwrap_or_else(|_| "unable to read response body".to_owned());
        return Err(providers::format_provider_http_error(
            "anthropic rejected stream request",
            &error,
            &detail,
        ));
    }
    let response = response;

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
                if let Some(token) = event
                    .delta
                    .and_then(|delta| delta.text)
                    .filter(|t| !t.is_empty())
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

    let claude = providers::resolve_cli_executable("claude")?;
    let mut child = tokio::process::Command::new(&claude);
    child
        .args(["-p", &prompt])
        .env("PATH", providers::enriched_cli_path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = child
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

#[derive(Debug, Deserialize)]
struct CodexStreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    item: Option<CodexStreamItem>,
}

#[derive(Debug, Deserialize)]
struct CodexStreamItem {
    #[serde(rename = "type")]
    item_type: String,
    text: Option<String>,
}

async fn stream_codex_cli(
    model_id: &str,
    messages: &[ChatMessageInput],
    on_event: &Channel<ChatStreamEvent>,
    cancelled: Arc<AtomicBool>,
) -> Result<(String, Option<String>), String> {
    let prompt = messages
        .iter()
        .map(|message| format!("{}: {}", message.role, message.content))
        .collect::<Vec<_>>()
        .join("\n\n");

    let codex = providers::resolve_cli_executable("codex")?;
    let mut child = tokio::process::Command::new(&codex);
    child
        .args(["exec", "--json", "-m", model_id, &prompt])
        .env("PATH", providers::enriched_cli_path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = child
        .spawn()
        .map_err(|error| format!("failed to launch Codex CLI: {error}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Codex CLI stdout unavailable".to_owned())?;
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
            .map_err(|error| format!("failed to read Codex CLI output: {error}"))?;
        if read == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(event) = serde_json::from_str::<CodexStreamEvent>(trimmed) else {
            continue;
        };
        if event.event_type != "item.completed" {
            continue;
        }
        let Some(item) = event.item else {
            continue;
        };
        if item.item_type != "agent_message" {
            continue;
        }
        if let Some(token) = item.text.filter(|value| !value.is_empty()) {
            assembled.push_str(&token);
            let _ = on_event.send(ChatStreamEvent::Token { content: token });
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|error| format!("failed to wait for Codex CLI: {error}"))?;
    if !status.success() {
        return Err(format!("Codex CLI exited with status {status}"));
    }
    if assembled.trim().is_empty() {
        return Err("Codex CLI returned an empty response".to_owned());
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
                if let Some(token) = choice
                    .delta
                    .content
                    .clone()
                    .filter(|value| !value.is_empty())
                {
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
    (system_parts.join("\n\n"), converted)
}

fn prepare_anthropic_messages(
    messages: &[ChatMessageInput],
) -> Result<(String, Vec<Value>), String> {
    let (system, mut converted) = split_anthropic_messages(messages);
    converted = normalize_anthropic_turns(converted);
    if converted.is_empty() {
        return Err(
            "anthropic requires at least one user message in the conversation".to_owned(),
        );
    }
    let anthropic_messages = converted
        .into_iter()
        .map(|message| {
            let role = message
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("user")
                .to_owned();
            let content = message
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            json!({
                "role": role,
                "content": [{
                    "type": "text",
                    "text": content,
                }],
            })
        })
        .collect();
    Ok((system, anthropic_messages))
}

fn normalize_anthropic_turns(messages: Vec<Value>) -> Vec<Value> {
    let mut normalized: Vec<Value> = Vec::new();
    for message in messages {
        let Some(role) = message.get("role").and_then(Value::as_str) else {
            continue;
        };
        let Some(content) = message.get("content").and_then(Value::as_str) else {
            continue;
        };
        if content.trim().is_empty() {
            continue;
        }
        if let Some(last) = normalized.last_mut() {
            let same_role = last
                .get("role")
                .and_then(Value::as_str)
                .is_some_and(|last_role| last_role == role);
            if same_role {
                let merged = format!(
                    "{}\n\n{}",
                    last.get("content").and_then(Value::as_str).unwrap_or_default(),
                    content
                );
                *last = json!({ "role": role, "content": merged });
                continue;
            }
        }
        normalized.push(json!({ "role": role, "content": content }));
    }

    if normalized
        .first()
        .and_then(|message| message.get("role"))
        .and_then(Value::as_str)
        .is_some_and(|role| role != "user")
    {
        normalized.remove(0);
    }

    let mut alternating: Vec<Value> = Vec::new();
    for message in normalized {
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("user");
        if let Some(last) = alternating.last() {
            let last_role = last
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("user");
            if last_role == role {
                continue;
            }
        }
        alternating.push(message);
    }
    alternating
}

fn agentdeck_tool_definitions() -> Vec<Value> {
    vec![
        tool_definition(
            "agentdeck.scan_environment",
            "Return the current local environment scan.",
        ),
        tool_definition(
            "agentdeck.get_graph",
            "Return a graph snapshot from the current scan.",
        ),
        tool_definition(
            "agentdeck.list_agents",
            "List discovered local agents and their status.",
        ),
        tool_definition(
            "agentdeck.list_mcp_servers",
            "List configured MCP servers from local inventory.",
        ),
        tool_definition(
            "agentdeck.health_check",
            "Run the AgentDeck preflight health check.",
        ),
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

    #[test]
    fn merges_consecutive_user_messages_for_anthropic() {
        let messages = vec![
            ChatMessageInput {
                role: "user".to_owned(),
                content: "First".to_owned(),
            },
            ChatMessageInput {
                role: "user".to_owned(),
                content: "Second".to_owned(),
            },
        ];
        let (_, prepared) = prepare_anthropic_messages(&messages).unwrap();
        assert_eq!(prepared.len(), 1);
        assert_eq!(
            prepared[0].get("content").and_then(Value::as_array).map(|blocks| blocks.len()),
            Some(1)
        );
    }

    #[test]
    fn prepare_anthropic_messages_uses_content_blocks() {
        let messages = vec![ChatMessageInput {
            role: "user".to_owned(),
            content: "Hello".to_owned(),
        }];
        let (system, prepared) = prepare_anthropic_messages(&messages).unwrap();
        assert!(system.is_empty());
        assert_eq!(
            prepared[0]["content"][0]["type"].as_str(),
            Some("text")
        );
    }
}
