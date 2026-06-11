use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use chrono::Utc;
use keyring::Entry;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::models::{
    ChatMessageInput, DiscoveredEntity, LocalModel, ProviderAdapterStatus, ProviderCheckRequest,
    ProviderCredentialRequest, ProviderHealth,
};
use crate::storage;

const KEYCHAIN_SERVICE: &str = "com.agentdeck.desktop.provider";
const LOCAL_LM_STUDIO_URL: &str = "http://localhost:1234/v1";
const KEYCHAIN_LOOKUP_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub(crate) struct ProviderDefinition {
    pub id: &'static str,
    name: &'static str,
    kind: &'static str,
    base_url: &'static str,
    auth_mode: &'static str,
    key_env: Option<&'static str>,
    base_url_env: Option<&'static str>,
    pub adapter: ProviderAdapter,
    capabilities: &'static [&'static str],
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ProviderAdapter {
    OpenAiCompatible,
    Anthropic,
    ClaudeCode,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelRecord>,
}

#[derive(Debug, Deserialize)]
struct ModelRecord {
    id: String,
    owned_by: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicModelsResponse {
    data: Vec<AnthropicModelRecord>,
}

#[derive(Debug, Deserialize)]
struct AnthropicModelRecord {
    id: String,
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Debug, Serialize)]
struct CompletionRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessageInput],
    temperature: f32,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct CompletionResponse {
    choices: Vec<CompletionChoice>,
}

#[derive(Debug, Deserialize)]
struct CompletionChoice {
    message: CompletionMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CompletionMessage {
    content: String,
}

#[derive(Debug, Serialize)]
struct ResponsesRequest<'a> {
    model: &'a str,
    instructions: &'a str,
    input: &'a str,
}

#[derive(Debug, Deserialize)]
struct ResponsesResponse {
    output: Vec<ResponsesOutputItem>,
}

#[derive(Debug, Deserialize)]
struct ResponsesOutputItem {
    #[serde(default)]
    content: Vec<ResponsesContentBlock>,
}

#[derive(Debug, Deserialize)]
struct ResponsesContentBlock {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessagesRequest<'a> {
    model: &'a str,
    system: &'a str,
    messages: &'a [AnthropicMessageInput<'a>],
    max_tokens: u32,
}

#[derive(Debug, Serialize)]
struct AnthropicMessageInput<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct AnthropicMessagesResponse {
    content: Vec<AnthropicContentBlock>,
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    text: Option<String>,
}

#[tauri::command]
pub async fn list_provider_adapters() -> Result<Vec<ProviderAdapterStatus>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        provider_definitions()
            .into_iter()
            .map(|definition| status_for_definition(&definition, false))
            .collect()
    })
    .await
    .map_err(|error| format!("provider inventory task failed: {error}"))?
}

#[tauri::command]
pub async fn check_provider_adapter(
    app: AppHandle,
    request: ProviderCheckRequest,
) -> Result<ProviderAdapterStatus, String> {
    validate_provider_id(&request.provider_id)?;
    let database_path = database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let started_at = Utc::now();
        let definition = find_definition(&request.provider_id)?;
        let result = status_for_definition(&definition, true);
        if let Ok(provider) = &result {
            if provider.credential_status == "keychain" {
                let _ = storage::set_provider_credential_stored(&database_path, definition.id, true);
            }
        }
        let status = if result
            .as_ref()
            .is_ok_and(|provider| provider.health.available)
        {
            "success"
        } else {
            "error"
        };
        let _ = store_provider_audit(
            &database_path,
            "provider.check",
            status,
            definition.id,
            started_at,
        );
        result
    })
    .await
    .map_err(|error| format!("provider check task failed: {error}"))?
}

#[tauri::command]
pub async fn save_provider_api_key(
    app: AppHandle,
    request: ProviderCredentialRequest,
) -> Result<(), String> {
    validate_provider_id(&request.provider_id)?;
    let api_key = request.api_key.trim().to_owned();
    if api_key.len() < 8 {
        return Err("API key must contain at least 8 characters".to_owned());
    }
    let database_path = database_path(&app)?;

    tauri::async_runtime::spawn_blocking(move || {
        let started_at = Utc::now();
        let definition = find_definition(&request.provider_id)?;
        if definition.key_env.is_none() {
            return Err(format!(
                "{} does not use API key credentials",
                definition.name
            ));
        }
        let result = keychain_entry(definition.id)?
            .set_password(&api_key)
            .map_err(|error| format!("failed to save API key in Keychain: {error}"));
        if result.is_ok() {
            storage::set_provider_credential_stored(&database_path, definition.id, true)?;
        }
        let _ = store_provider_audit(
            &database_path,
            "provider.credential.save",
            if result.is_ok() { "success" } else { "error" },
            definition.id,
            started_at,
        );
        result
    })
    .await
    .map_err(|error| format!("key save task failed: {error}"))?
}

#[tauri::command]
pub async fn delete_provider_api_key(app: AppHandle, provider_id: String) -> Result<(), String> {
    validate_provider_id(&provider_id)?;
    let database_path = database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let started_at = Utc::now();
        let definition = find_definition(&provider_id)?;
        if definition.key_env.is_none() {
            return Err(format!(
                "{} does not use API key credentials",
                definition.name
            ));
        }
        let entry = keychain_entry(definition.id)?;
        let result = match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(format!("failed to delete API key from Keychain: {error}")),
        };
        if result.is_ok() {
            storage::set_provider_credential_stored(&database_path, &provider_id, false)?;
        }
        let _ = store_provider_audit(
            &database_path,
            "provider.credential.delete",
            if result.is_ok() { "success" } else { "error" },
            definition.id,
            started_at,
        );
        result
    })
    .await
    .map_err(|error| format!("key delete task failed: {error}"))?
}

#[derive(Debug, Clone)]
pub(crate) struct XaiReadiness {
    pub credential_status: String,
    pub subscription_active: bool,
    pub health: ProviderHealth,
}

pub(crate) fn xai_readiness() -> XaiReadiness {
    xai_readiness_with_probe(false)
}

pub(crate) fn xai_readiness_with_probe(probe_keychain: bool) -> XaiReadiness {
    let definition = find_definition("xai").expect("xAI provider definition must exist");
    let credential_status = credential_status(&definition, probe_keychain);
    let subscription_active = grok_subscription_active();
    let health = if credential_status == "missing" && subscription_active {
        ProviderHealth {
            name: definition.name.to_owned(),
            endpoint: resolved_base_url(&definition),
            available: true,
            detail: "Grok subscription marked active locally; xAI API key not configured."
                .to_owned(),
        }
    } else if credential_status == "missing" {
        ProviderHealth {
            name: definition.name.to_owned(),
            endpoint: resolved_base_url(&definition),
            available: false,
            detail: "xAI subscription or API key not configured.".to_owned(),
        }
    } else if credential_status == "keychain" && !probe_keychain {
        ProviderHealth {
            name: definition.name.to_owned(),
            endpoint: resolved_base_url(&definition),
            available: true,
            detail: "API key stored in Keychain. Run a provider check to verify.".to_owned(),
        }
    } else {
        xai_health(&definition)
    };

    XaiReadiness {
        credential_status,
        subscription_active,
        health,
    }
}

pub(crate) fn grok_source_agent(readiness: &XaiReadiness) -> DiscoveredEntity {
    let status = grok_status_label(&readiness.credential_status, readiness.health.available);
    let mut metadata = BTreeMap::new();
    metadata.insert("provider".to_owned(), "xAI".to_owned());
    metadata.insert("providerId".to_owned(), "xai".to_owned());
    metadata.insert("capability".to_owned(), "web research".to_owned());
    metadata.insert(
        "credentialStatus".to_owned(),
        readiness.credential_status.clone(),
    );
    metadata.insert(
        "subscriptionActive".to_owned(),
        readiness.subscription_active.to_string(),
    );
    metadata.insert("endpoint".to_owned(), readiness.health.endpoint.clone());
    metadata.insert("healthDetail".to_owned(), readiness.health.detail.clone());

    DiscoveredEntity {
        id: "agent:grok".to_owned(),
        entity_type: "agent".to_owned(),
        name: "Grok".to_owned(),
        status: status.to_owned(),
        source: "xai".to_owned(),
        metadata,
    }
}

pub(crate) fn grok_status_label(credential_status: &str, health_available: bool) -> &'static str {
    if health_available {
        "available"
    } else if credential_status == "missing" {
        "unavailable"
    } else {
        "degraded"
    }
}

fn status_for_definition(
    definition: &ProviderDefinition,
    check_remote: bool,
) -> Result<ProviderAdapterStatus, String> {
    let base_url = resolved_base_url(definition);
    let credential_status = credential_status(definition, check_remote);
    let mut health = ProviderHealth {
        name: definition.name.to_owned(),
        endpoint: base_url.clone(),
        available: false,
        detail: if definition.key_env.is_some() {
            "Not checked. Cloud provider checks run only when requested.".to_owned()
        } else {
            "Not checked.".to_owned()
        },
    };
    let mut models = Vec::new();

    if check_remote {
        match fetch_provider_models(definition, &base_url) {
            Ok(next_models) => {
                health.available = true;
                health.detail = format!("{} models available.", next_models.len());
                models = next_models;
            }
            Err(error) => {
                health.detail = error;
            }
        }
    }

    Ok(ProviderAdapterStatus {
        id: definition.id.to_owned(),
        name: definition.name.to_owned(),
        kind: definition.kind.to_owned(),
        base_url,
        auth_mode: definition.auth_mode.to_owned(),
        credential_status,
        health,
        models,
        capabilities: definition
            .capabilities
            .iter()
            .map(|capability| (*capability).to_owned())
            .collect(),
    })
}

fn xai_health(definition: &ProviderDefinition) -> ProviderHealth {
    let endpoint = format!(
        "{}/models",
        resolved_base_url(definition).trim_end_matches('/')
    );
    let client = match Client::builder()
        .timeout(Duration::from_secs(2))
        .no_proxy()
        .build()
    {
        Ok(client) => client,
        Err(error) => return provider_health_unavailable(&endpoint, error.to_string()),
    };

    let mut request = client.get(&endpoint);
    match provider_headers(definition) {
        Ok(Some(headers)) => {
            request = request.headers(headers);
        }
        Ok(None) => {}
        Err(error) => return provider_health_unavailable(&endpoint, error),
    }

    match request.send() {
        Ok(response) if response.status().is_success() => ProviderHealth {
            name: definition.name.to_owned(),
            endpoint,
            available: true,
            detail: format!("HTTP {}", response.status().as_u16()),
        },
        Ok(response) => provider_health_unavailable(
            &endpoint,
            format!("endpoint returned HTTP {}", response.status().as_u16()),
        ),
        Err(error) => provider_health_unavailable(&endpoint, error.to_string()),
    }
}

fn provider_health_unavailable(endpoint: &str, detail: String) -> ProviderHealth {
    ProviderHealth {
        name: "xAI".to_owned(),
        endpoint: endpoint.to_owned(),
        available: false,
        detail,
    }
}

fn grok_subscription_active() -> bool {
    storage::home_database_path()
        .and_then(|path| storage::load_app_settings(&path))
        .map(|settings| settings.grok_subscription_active)
        .unwrap_or(true)
}

pub(crate) fn fetch_provider_models_blocking(
    definition: &ProviderDefinition,
    base_url: &str,
) -> Result<Vec<LocalModel>, String> {
    fetch_provider_models(definition, base_url)
}

pub(crate) fn resolve_lm_studio_model_id(database_path: Option<&Path>) -> Result<String, String> {
    if let Some(path) = database_path {
        if let Ok(preferences) = storage::load_chat_preferences(path) {
            if preferences.last_provider_id == "lm-studio" && !preferences.last_model_id.is_empty() {
                return Ok(preferences.last_model_id);
            }
        }
    }

    let definition = find_definition("lm-studio")?;
    let base_url = resolved_base_url(&definition);
    let models = fetch_provider_models(&definition, &base_url)?;
    models
        .into_iter()
        .find(|model| !is_embedding_model_id(&model.id))
        .map(|model| model.id)
        .ok_or_else(|| "no chat models available from LM Studio".to_owned())
}

fn is_embedding_model_id(model_id: &str) -> bool {
    let normalized = model_id.to_ascii_lowercase();
    normalized.contains("embed")
}

fn fetch_provider_models(
    definition: &ProviderDefinition,
    base_url: &str,
) -> Result<Vec<LocalModel>, String> {
    match definition.adapter {
        ProviderAdapter::ClaudeCode => fetch_claude_code_models(),
        ProviderAdapter::Anthropic => fetch_anthropic_models(definition, base_url),
        ProviderAdapter::OpenAiCompatible if definition.id == "codex" => {
            fetch_codex_models(definition, base_url)
        }
        ProviderAdapter::OpenAiCompatible => {
            fetch_openai_compatible_models(definition, base_url)
        }
    }
}

fn fetch_openai_compatible_models(
    definition: &ProviderDefinition,
    base_url: &str,
) -> Result<Vec<LocalModel>, String> {
    if !is_http_url(base_url) {
        return Err(format!(
            "{} does not expose an HTTP /models endpoint",
            definition.name
        ));
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .no_proxy()
        .build()
        .map_err(|error| format!("failed to create provider HTTP client: {error}"))?;

    let endpoint = format!("{}/models", base_url.trim_end_matches('/'));
    let mut request = client.get(endpoint);
    if let Some(headers) = provider_headers(definition)? {
        request = request.headers(headers);
    }

    let response = request
        .send()
        .map_err(|error| format!("provider is unavailable: {error}"))?
        .error_for_status()
        .map_err(|error| format!("provider model request failed: {error}"))?
        .json::<ModelsResponse>()
        .map_err(|error| format!("provider returned invalid model data: {error}"))?;

    Ok(normalize_openai_models(response.data))
}

fn fetch_anthropic_models(
    definition: &ProviderDefinition,
    base_url: &str,
) -> Result<Vec<LocalModel>, String> {
    if !is_http_url(base_url) {
        return Err(format!(
            "{} does not expose an HTTP /models endpoint",
            definition.name
        ));
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .no_proxy()
        .build()
        .map_err(|error| format!("failed to create provider HTTP client: {error}"))?;

    let endpoint = format!("{}/models", base_url.trim_end_matches('/'));
    let mut headers = provider_headers(definition)?.unwrap_or_default();
    if !headers.contains_key("anthropic-version") {
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
    }

    let response = client
        .get(endpoint)
        .headers(headers)
        .send()
        .map_err(|error| format!("Anthropic is unavailable: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Anthropic model request failed: {error}"))?
        .json::<AnthropicModelsResponse>()
        .map_err(|error| format!("Anthropic returned invalid model data: {error}"))?;

    let mut models = response
        .data
        .into_iter()
        .filter(|model| !model.id.trim().is_empty())
        .map(|model| LocalModel {
            id: model.id,
            owned_by: model.display_name,
        })
        .collect::<Vec<_>>();
    if models.is_empty() {
        models = anthropic_fallback_models();
    }
    models.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(models)
}

fn fetch_codex_models(
    definition: &ProviderDefinition,
    base_url: &str,
) -> Result<Vec<LocalModel>, String> {
    let mut models = codex_static_models();
    if let Ok(api_models) = fetch_openai_compatible_models(definition, base_url) {
        for model in api_models {
            let id = model.id.to_lowercase();
            if (id.contains("codex") || id.starts_with("gpt-5"))
                && !models.iter().any(|entry| entry.id == model.id)
            {
                models.push(model);
            }
        }
    }
    models.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(models)
}

fn fetch_claude_code_models() -> Result<Vec<LocalModel>, String> {
    if !command_available("claude") {
        return Err(
            "Claude Code CLI is not installed or not on PATH. Install it, then reload models."
                .to_owned(),
        );
    }
    Ok(vec![LocalModel {
        id: "claude-code".to_owned(),
        owned_by: Some("claude-cli".to_owned()),
    }])
}

fn normalize_openai_models(models: Vec<ModelRecord>) -> Vec<LocalModel> {
    let mut normalized = models
        .into_iter()
        .filter(|model| !model.id.trim().is_empty())
        .map(|model| LocalModel {
            id: model.id,
            owned_by: model.owned_by,
        })
        .collect::<Vec<_>>();
    normalized.sort_by(|left, right| left.id.cmp(&right.id));
    normalized
}

fn codex_static_models() -> Vec<LocalModel> {
    [
        "gpt-5.5",
        "gpt-5.4",
        "gpt-5.4-mini",
        "gpt-5.3-codex-spark",
        "codex-mini-latest",
    ]
    .into_iter()
    .map(|id| LocalModel {
        id: id.to_owned(),
        owned_by: Some("openai-codex".to_owned()),
    })
    .collect()
}

fn anthropic_fallback_models() -> Vec<LocalModel> {
    [
        "claude-opus-4-6",
        "claude-sonnet-4-6",
        "claude-haiku-4-5",
    ]
    .into_iter()
    .map(|id| LocalModel {
        id: id.to_owned(),
        owned_by: Some("anthropic".to_owned()),
    })
    .collect()
}

fn command_available(name: &str) -> bool {
    let path = Path::new(name);
    if path.components().count() > 1 {
        return path.is_file();
    }
    env::var_os("PATH").is_some_and(|paths| {
        env::split_paths(&paths).any(|directory| directory.join(name).is_file())
    })
}

fn http_client(timeout: Duration) -> Result<Client, String> {
    Client::builder()
        .timeout(timeout)
        .no_proxy()
        .build()
        .map_err(|error| format!("failed to create provider HTTP client: {error}"))
}

pub(crate) fn dispatch_provider_handoff(
    provider_id: &str,
    model_id: &str,
    title: &str,
    task: &str,
    context: &str,
    source_agent_name: &str,
    prompt: &str,
) -> Result<(String, Option<String>), String> {
    let definition = find_definition(provider_id)?;
    let base_url = resolved_base_url(&definition);
    match definition.adapter {
        ProviderAdapter::OpenAiCompatible => dispatch_openai_compatible(
            &definition,
            &base_url,
            model_id,
            title,
            task,
            context,
            source_agent_name,
            prompt,
        ),
        ProviderAdapter::Anthropic => dispatch_anthropic(
            &definition,
            &base_url,
            model_id,
            title,
            task,
            context,
            source_agent_name,
            prompt,
        ),
        ProviderAdapter::ClaudeCode => dispatch_claude_code(prompt),
    }
}

pub(crate) fn find_provider(provider_id: &str) -> Result<ProviderDefinition, String> {
    find_definition(provider_id)
}

pub(crate) fn provider_base_url(definition: &ProviderDefinition) -> String {
    resolved_base_url(definition)
}

pub(crate) fn build_provider_headers(
    definition: &ProviderDefinition,
) -> Result<Option<HeaderMap>, String> {
    provider_headers(definition)
}

pub(crate) fn uses_responses_api(definition: &ProviderDefinition, base_url: &str) -> bool {
    uses_openai_responses_api(definition, base_url)
}

fn dispatch_claude_code(prompt: &str) -> Result<(String, Option<String>), String> {
    use std::process::{Command, Stdio};

    let output = Command::new("claude")
        .args(["-p", prompt])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("failed to launch Claude Code: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Claude Code exited with status {}: {}",
            output.status,
            stderr.trim()
        ));
    }

    let content = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if content.is_empty() {
        return Err("Claude Code returned an empty response".to_owned());
    }
    Ok((content, None))
}

fn dispatch_openai_compatible(
    definition: &ProviderDefinition,
    base_url: &str,
    model_id: &str,
    title: &str,
    task: &str,
    context: &str,
    source_agent_name: &str,
    prompt: &str,
) -> Result<(String, Option<String>), String> {
    if uses_openai_responses_api(definition, base_url) {
        return dispatch_openai_responses(
            definition,
            base_url,
            model_id,
            title,
            task,
            context,
            source_agent_name,
            prompt,
        );
    }

    let mut headers = provider_headers(definition)?.unwrap_or_default();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let response = http_client(Duration::from_secs(120))?
        .post(format!("{}/chat/completions", base_url.trim_end_matches('/')))
        .headers(headers)
        .json(&CompletionRequest {
            model: model_id,
            messages: &[
                ChatMessageInput {
                    role: "system".to_owned(),
                    content: format!(
                        "You are the target of a manual AgentDeck handoff. Source agent: {source_agent_name}. Title: {title}. Return a concise result with concrete next steps."
                    ),
                },
                ChatMessageInput {
                    role: "user".to_owned(),
                    content: format!("Task:\n{task}\n\nContext:\n{context}\n\n{prompt}"),
                },
            ],
            temperature: 0.2,
            stream: false,
        })
        .send()
        .map_err(|error| format!("handoff dispatch failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("handoff provider rejected the request: {error}"))?
        .json::<CompletionResponse>()
        .map_err(|error| format!("handoff provider returned invalid data: {error}"))?;

    let choice = response
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| "handoff provider returned no choices".to_owned())?;
    let content = choice.message.content.trim().to_owned();
    if content.is_empty() {
        return Err("handoff provider returned an empty response".to_owned());
    }
    Ok((content, choice.finish_reason))
}

fn dispatch_openai_responses(
    definition: &ProviderDefinition,
    base_url: &str,
    model_id: &str,
    title: &str,
    task: &str,
    context: &str,
    source_agent_name: &str,
    prompt: &str,
) -> Result<(String, Option<String>), String> {
    let mut headers = provider_headers(definition)?.unwrap_or_default();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let instructions = format!(
        "You are the target of a manual AgentDeck handoff. Source agent: {source_agent_name}. Title: {title}. Return a concise result with concrete next steps."
    );
    let input = format!("Task:\n{task}\n\nContext:\n{context}\n\n{prompt}");
    let response = http_client(Duration::from_secs(120))?
        .post(format!("{}/responses", base_url.trim_end_matches('/')))
        .headers(headers)
        .json(&ResponsesRequest {
            model: model_id,
            instructions: &instructions,
            input: &input,
        })
        .send()
        .map_err(|error| format!("handoff dispatch failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("handoff provider rejected the request: {error}"))?
        .json::<ResponsesResponse>()
        .map_err(|error| format!("handoff provider returned invalid data: {error}"))?;

    Ok((extract_responses_text(response)?, None))
}

fn extract_responses_text(response: ResponsesResponse) -> Result<String, String> {
    response
        .output
        .into_iter()
        .flat_map(|item| item.content)
        .filter_map(|block| block.text)
        .map(|text| text.trim().to_owned())
        .find(|text| !text.is_empty())
        .ok_or_else(|| "handoff provider returned an empty response".to_owned())
}

fn uses_openai_responses_api(definition: &ProviderDefinition, base_url: &str) -> bool {
    matches!(definition.id, "openai-compatible" | "codex")
        && base_url.trim_end_matches('/') == "https://api.openai.com/v1"
}

fn dispatch_anthropic(
    definition: &ProviderDefinition,
    base_url: &str,
    model_id: &str,
    title: &str,
    task: &str,
    context: &str,
    source_agent_name: &str,
    prompt: &str,
) -> Result<(String, Option<String>), String> {
    let mut headers = provider_headers(definition)?.unwrap_or_default();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let response = http_client(Duration::from_secs(120))?
        .post(format!("{}/messages", base_url.trim_end_matches('/')))
        .headers(headers)
        .json(&AnthropicMessagesRequest {
            model: model_id,
            system: &format!(
                "You are the target of a manual AgentDeck handoff. Source agent: {source_agent_name}. Title: {title}. Return a concise result with concrete next steps."
            ),
            messages: &[AnthropicMessageInput {
                role: "user",
                content: &format!("Task:\n{task}\n\nContext:\n{context}\n\n{prompt}"),
            }],
            max_tokens: 1024,
        })
        .send()
        .map_err(|error| format!("handoff dispatch failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("handoff provider rejected the request: {error}"))?
        .json::<AnthropicMessagesResponse>()
        .map_err(|error| format!("handoff provider returned invalid data: {error}"))?;

    let content = response
        .content
        .into_iter()
        .find_map(|block| block.text)
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
        .ok_or_else(|| "handoff provider returned an empty response".to_owned())?;
    Ok((content, response.stop_reason))
}

fn provider_headers(definition: &ProviderDefinition) -> Result<Option<HeaderMap>, String> {
    let Some(api_key) = resolve_api_key(definition)? else {
        if definition.key_env.is_some() {
            return Err("No API key found in Keychain or development environment.".to_owned());
        }
        return Ok(None);
    };

    let mut headers = HeaderMap::new();
    match definition.adapter {
        ProviderAdapter::OpenAiCompatible => {
            let value = HeaderValue::from_str(&format!("Bearer {api_key}"))
                .map_err(|error| format!("invalid API key header value: {error}"))?;
            headers.insert(AUTHORIZATION, value);
        }
        ProviderAdapter::Anthropic => {
            let key = HeaderValue::from_str(&api_key)
                .map_err(|error| format!("invalid API key header value: {error}"))?;
            headers.insert("x-api-key", key);
            headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        }
        ProviderAdapter::ClaudeCode => {}
    }
    Ok(Some(headers))
}

fn credential_status(definition: &ProviderDefinition, probe_keychain: bool) -> String {
    if definition.key_env.is_none() {
        return "not-required".to_owned();
    }
    if definition
        .key_env
        .and_then(|name| env::var(name).ok())
        .is_some_and(|value| !value.trim().is_empty())
    {
        return "environment".to_owned();
    }
    if probe_keychain && has_keychain_credential(definition.id) {
        return "keychain".to_owned();
    }
    if provider_credential_stored(definition.id) {
        return "keychain".to_owned();
    }
    "missing".to_owned()
}

fn provider_credential_stored(provider_id: &str) -> bool {
    storage::home_database_path()
        .map(|path| storage::is_provider_credential_stored(&path, provider_id))
        .unwrap_or(false)
}

fn resolve_api_key(definition: &ProviderDefinition) -> Result<Option<String>, String> {
    if definition.key_env.is_none() {
        return Ok(None);
    }
    if let Some(secret) = keychain_password(definition.id) {
        return Ok(Some(secret));
    }
    Ok(definition
        .key_env
        .and_then(|name| env::var(name).ok())
        .filter(|value| !value.trim().is_empty()))
}

fn has_keychain_credential(provider_id: &str) -> bool {
    keychain_password(provider_id).is_some()
}

fn keychain_password(provider_id: &str) -> Option<String> {
    let (sender, receiver) = mpsc::channel();
    let provider_id = provider_id.to_owned();
    std::thread::spawn(move || {
        let password = keychain_entry(&provider_id)
            .ok()
            .and_then(|entry| entry.get_password().ok())
            .filter(|value| !value.trim().is_empty());
        let _ = sender.send(password);
    });

    receiver.recv_timeout(KEYCHAIN_LOOKUP_TIMEOUT).ok().flatten()
}

fn keychain_entry(provider_id: &str) -> Result<Entry, String> {
    Entry::new(KEYCHAIN_SERVICE, provider_id)
        .map_err(|error| format!("failed to open Keychain entry: {error}"))
}

fn resolved_base_url(definition: &ProviderDefinition) -> String {
    definition
        .base_url_env
        .and_then(|name| env::var(name).ok())
        .filter(|value| is_http_url(value))
        .unwrap_or_else(|| definition.base_url.to_owned())
}

fn find_definition(provider_id: &str) -> Result<ProviderDefinition, String> {
    provider_definitions()
        .into_iter()
        .find(|definition| definition.id == provider_id)
        .ok_or_else(|| format!("unknown provider adapter: {provider_id}"))
}

fn provider_definitions() -> Vec<ProviderDefinition> {
    vec![
        ProviderDefinition {
            id: "lm-studio",
            name: "LM Studio",
            kind: "openai-compatible",
            base_url: LOCAL_LM_STUDIO_URL,
            auth_mode: "none",
            key_env: None,
            base_url_env: None,
            adapter: ProviderAdapter::OpenAiCompatible,
            capabilities: &["models", "chat"],
        },
        ProviderDefinition {
            id: "openai-compatible",
            name: "OpenAI-compatible",
            kind: "openai-compatible",
            base_url: "https://api.openai.com/v1",
            auth_mode: "bearer-key",
            key_env: Some("OPENAI_API_KEY"),
            base_url_env: Some("AGENTDECK_OPENAI_COMPATIBLE_BASE_URL"),
            adapter: ProviderAdapter::OpenAiCompatible,
            capabilities: &["models", "chat"],
        },
        ProviderDefinition {
            id: "xai",
            name: "xAI",
            kind: "openai-compatible",
            base_url: "https://api.x.ai/v1",
            auth_mode: "bearer-key",
            key_env: Some("XAI_API_KEY"),
            base_url_env: Some("AGENTDECK_XAI_BASE_URL"),
            adapter: ProviderAdapter::OpenAiCompatible,
            capabilities: &["models", "chat", "research"],
        },
        ProviderDefinition {
            id: "anthropic",
            name: "Anthropic",
            kind: "anthropic",
            base_url: "https://api.anthropic.com/v1",
            auth_mode: "x-api-key",
            key_env: Some("ANTHROPIC_API_KEY"),
            base_url_env: Some("AGENTDECK_ANTHROPIC_BASE_URL"),
            adapter: ProviderAdapter::Anthropic,
            capabilities: &["models", "chat"],
        },
        ProviderDefinition {
            id: "codex",
            name: "Codex",
            kind: "openai-compatible",
            base_url: "https://api.openai.com/v1",
            auth_mode: "bearer-key",
            key_env: Some("OPENAI_API_KEY"),
            base_url_env: Some("AGENTDECK_OPENAI_COMPATIBLE_BASE_URL"),
            adapter: ProviderAdapter::OpenAiCompatible,
            capabilities: &["models", "chat", "codex"],
        },
        ProviderDefinition {
            id: "claude-code",
            name: "Claude Code",
            kind: "claude-code-mcp",
            base_url: "stdio://claude",
            auth_mode: "none",
            key_env: None,
            base_url_env: None,
            adapter: ProviderAdapter::ClaudeCode,
            capabilities: &["chat", "tools"],
        },
    ]
}

fn validate_provider_id(provider_id: &str) -> Result<(), String> {
    if provider_id.is_empty() || provider_id.len() > 80 {
        return Err("provider ID must contain between 1 and 80 characters".to_owned());
    }
    if !provider_id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err("provider ID contains unsupported characters".to_owned());
    }
    Ok(())
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://")
}

fn database_path(app: &AppHandle) -> Result<PathBuf, String> {
    storage::database_path(app)
}

fn store_provider_audit(
    path: &Path,
    action: &str,
    status: &str,
    provider_id: &str,
    started_at: chrono::DateTime<Utc>,
) -> Result<(), String> {
    let connection = storage::open_database(path)?;
    let created_at = Utc::now();
    let id = format!(
        "audit:{:016x}",
        stable_hash(&format!("{action}:{provider_id}:{created_at}"))
    );
    connection
        .execute(
            "INSERT INTO audit_events
                (id, action, status, model, conversation_id, duration_ms, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                action,
                status,
                provider_id,
                "provider",
                (created_at - started_at).num_milliseconds(),
                created_at.to_rfc3339()
            ],
        )
        .map_err(|error| format!("failed to store provider audit event: {error}"))?;
    storage::append_log_event(
        path,
        "audit_event",
        serde_json::json!({
            "id": id,
            "action": action,
            "status": status,
            "model": provider_id,
            "conversationId": "provider",
            "durationMs": (created_at - started_at).num_milliseconds(),
            "createdAt": created_at.to_rfc3339(),
        }),
    );
    Ok(())
}

fn stable_hash(value: &str) -> u64 {
    value
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_ids_are_validated() {
        assert!(validate_provider_id("openai-compatible").is_ok());
        assert!(validate_provider_id("../bad").is_err());
    }

    #[test]
    fn known_providers_include_phase_four_adapters() {
        let ids: Vec<_> = provider_definitions()
            .into_iter()
            .map(|definition| definition.id)
            .collect();
        assert!(ids.contains(&"lm-studio"));
        assert!(ids.contains(&"openai-compatible"));
        assert!(ids.contains(&"xai"));
        assert!(ids.contains(&"anthropic"));
        assert!(ids.contains(&"codex"));
        assert!(ids.contains(&"claude-code"));
    }

    #[test]
    fn keyless_provider_does_not_require_credentials() {
        let definition = find_definition("lm-studio").unwrap();
        assert_eq!(credential_status(&definition, false), "not-required");
    }

    #[test]
    fn openai_official_adapter_uses_responses_api() {
        let openai = find_definition("openai-compatible").unwrap();
        let codex = find_definition("codex").unwrap();
        let xai = find_definition("xai").unwrap();

        assert!(uses_openai_responses_api(
            &openai,
            "https://api.openai.com/v1"
        ));
        assert!(uses_openai_responses_api(&codex, "https://api.openai.com/v1"));
        assert!(!uses_openai_responses_api(&xai, "https://api.x.ai/v1"));
        assert!(!uses_openai_responses_api(
            &openai,
            "https://example.test/v1"
        ));
    }

    #[test]
    fn codex_models_include_curated_defaults() {
        let definition = find_definition("codex").unwrap();
        let models = fetch_codex_models(&definition, "https://api.openai.com/v1").unwrap();
        assert!(models.iter().any(|model| model.id == "gpt-5.5"));
        assert!(models.iter().any(|model| model.id == "codex-mini-latest"));
    }

    #[test]
    fn claude_code_models_require_cli() {
        let result = fetch_claude_code_models();
        if command_available("claude") {
            assert_eq!(result.unwrap()[0].id, "claude-code");
        } else {
            assert!(result.is_err());
        }
    }

    #[test]
    fn extracts_text_from_responses_message_blocks() {
        let response = ResponsesResponse {
            output: vec![
                ResponsesOutputItem { content: vec![] },
                ResponsesOutputItem {
                    content: vec![ResponsesContentBlock {
                        text: Some("  handoff confirmed  ".to_owned()),
                    }],
                },
            ],
        };

        assert_eq!(
            extract_responses_text(response).unwrap(),
            "handoff confirmed"
        );
    }

    #[test]
    fn grok_status_reflects_credentials_and_health() {
        assert_eq!(grok_status_label("missing", true), "available");
        assert_eq!(grok_status_label("keychain", true), "available");
        assert_eq!(grok_status_label("environment", false), "degraded");
        assert_eq!(grok_status_label("missing", false), "unavailable");
    }

    #[test]
    fn grok_source_agent_uses_xai_readiness_metadata() {
        let health = ProviderHealth {
            name: "xAI".to_owned(),
            endpoint: "https://api.x.ai/v1/models".to_owned(),
            available: true,
            detail: "HTTP 200".to_owned(),
        };
        let readiness = XaiReadiness {
            credential_status: "keychain".to_owned(),
            subscription_active: true,
            health,
        };

        let grok = grok_source_agent(&readiness);

        assert_eq!(grok.id, "agent:grok");
        assert_eq!(grok.status, "available");
        assert_eq!(
            grok.metadata.get("providerId").map(String::as_str),
            Some("xai")
        );
        assert_eq!(
            grok.metadata.get("subscriptionActive").map(String::as_str),
            Some("true")
        );
    }
}
