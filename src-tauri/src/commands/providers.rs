use std::collections::{BTreeMap, HashMap};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use chrono::Utc;
use keyring::Entry;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::models::{
    CatalogSource, ChatMessageInput, CredentialStatus, DiscoveredEntity,
    LegacyCredentialImportOutcome, LegacyCredentialImportResult, LocalModel,
    ProviderAdapterStatus, ProviderCheckRequest, ProviderCredentialRequest, ProviderHealth,
};
use crate::secrets;
use crate::storage;

const LOCAL_LM_STUDIO_URL: &str = "http://localhost:1234/v1";
const LEGACY_KEYCHAIN_SERVICE: &str = "com.agentdeck.desktop.provider";
static API_KEY_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn api_key_cache() -> &'static Mutex<HashMap<String, String>> {
    API_KEY_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn credential_slot_id(definition: &ProviderDefinition) -> &'static str {
    match definition.key_env {
        Some("OPENAI_API_KEY") => "openai",
        Some("ANTHROPIC_API_KEY") => "anthropic",
        Some("XAI_API_KEY") => "xai",
        _ => definition.id,
    }
}

fn cache_api_key(slot_id: &str, key: &str) {
    if let Ok(mut cache) = api_key_cache().lock() {
        cache.insert(slot_id.to_owned(), key.to_owned());
    }
}

fn get_cached_api_key(slot_id: &str) -> Option<String> {
    api_key_cache()
        .lock()
        .ok()
        .and_then(|cache| cache.get(slot_id).cloned())
}

fn evict_cached_api_key(slot_id: &str) {
    if let Ok(mut cache) = api_key_cache().lock() {
        cache.remove(slot_id);
    }
}

#[cfg(test)]
fn clear_api_key_cache() {
    if let Ok(mut cache) = api_key_cache().lock() {
        cache.clear();
    }
}

/// Encrypt and persist an API key in the app-local secret store, keyed by slot.
fn store_provider_secret(
    path: &Path,
    definition: &ProviderDefinition,
    api_key: &str,
) -> Result<(), String> {
    let slot = credential_slot_id(definition);
    let master = secrets::master_key(path)?;
    let ciphertext = secrets::encrypt(&master, api_key)?;
    storage::store_provider_secret(path, slot, &ciphertext)?;
    cache_api_key(slot, api_key);
    Ok(())
}

/// Read and decrypt the stored API key for a provider, if present.
fn read_stored_secret_at_path(
    database_path: Option<&Path>,
    definition: &ProviderDefinition,
) -> Result<Option<String>, String> {
    let path = match database_path {
        Some(path) => path.to_path_buf(),
        None => storage::resolve_database_path(None)?,
    };
    read_stored_secret_at(&path, definition)
}

fn read_stored_secret(definition: &ProviderDefinition) -> Result<Option<String>, String> {
    read_stored_secret_at_path(None, definition)
}

fn read_stored_secret_at(
    path: &Path,
    definition: &ProviderDefinition,
) -> Result<Option<String>, String> {
    let slot = credential_slot_id(definition);
    let Some(ciphertext) = storage::read_provider_secret(&path, slot)? else {
        return Ok(None);
    };
    let master = secrets::load_master_key(&path)?;
    let secret = secrets::decrypt(&master, &ciphertext)?;
    cache_api_key(slot, &secret);
    Ok(Some(secret))
}

/// Remove the stored API key from the secret store and the in-memory cache.
fn delete_stored_secret(path: &Path, definition: &ProviderDefinition) -> Result<(), String> {
    let slot = credential_slot_id(definition);
    storage::delete_provider_secret(path, slot)?;
    evict_cached_api_key(slot);
    Ok(())
}

fn mark_shared_credential_stored(
    path: &Path,
    definition: &ProviderDefinition,
    stored: bool,
) -> Result<(), String> {
    let Some(key_env) = definition.key_env else {
        return Ok(());
    };
    for sibling in provider_definitions() {
        if sibling.key_env == Some(key_env) {
            storage::set_provider_credential_stored(path, sibling.id, stored)?;
        }
    }
    Ok(())
}

#[derive(Debug)]
struct CredentialState {
    status: CredentialStatus,
    error: Option<String>,
}

fn credential_state(definition: &ProviderDefinition) -> CredentialState {
    let path = storage::resolve_database_path(None).ok();
    credential_state_at(definition, path.as_deref())
}

fn credential_state_at(
    definition: &ProviderDefinition,
    database_path: Option<&Path>,
) -> CredentialState {
    if definition.key_env.is_none() {
        return CredentialState {
            status: CredentialStatus::NotRequired,
            error: None,
        };
    }

    let slot = credential_slot_id(definition);
    let cache_matches_database = database_path.is_none_or(|path| {
        storage::resolve_database_path(None)
            .ok()
            .is_some_and(|home| home == path)
    });
    if cache_matches_database && get_cached_api_key(slot).is_some() {
        return CredentialState {
            status: CredentialStatus::Stored,
            error: None,
        };
    }

    let Some(path) = database_path else {
        return CredentialState {
            status: CredentialStatus::Unreadable,
            error: Some("Stored credential path is unavailable.".to_owned()),
        };
    };

    match read_stored_secret_at(path, definition) {
        Ok(Some(_)) => CredentialState {
            status: CredentialStatus::Stored,
            error: None,
        },
        Ok(None) => {
            if provider_definitions().iter().any(|sibling| {
                sibling.key_env == definition.key_env
                    && storage::is_provider_credential_stored(path, sibling.id)
            }) {
                let _ = mark_shared_credential_stored(path, definition, false);
            }
            if read_env_api_key(definition).is_some() {
                return CredentialState {
                    status: CredentialStatus::Environment,
                    error: None,
                };
            }
            CredentialState {
                status: CredentialStatus::Missing,
                error: None,
            }
        }
        Err(error) => CredentialState {
            status: CredentialStatus::Unreadable,
            error: Some(format!("Stored credential is unreadable: {error}")),
        },
    }
}

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
        let database_path = storage::resolve_database_path(None)?;
        provider_definitions()
            .into_iter()
            .map(|definition| status_for_definition(&definition, false, Some(&database_path)))
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
        let result = status_for_definition(&definition, true, Some(&database_path));
        if let Ok(provider) = &result {
            if provider.credential_status == CredentialStatus::Stored {
                let _ = mark_shared_credential_stored(&database_path, &definition, true);
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
        let result = store_provider_secret(&database_path, &definition, &api_key);
        if result.is_ok() {
            mark_shared_credential_stored(&database_path, &definition, true)?;
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
pub async fn import_legacy_provider_credentials(
    app: AppHandle,
) -> Result<LegacyCredentialImportResult, String> {
    let database_path = database_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let started_at = Utc::now();
        let (candidates, mut result) = collect_legacy_credentials_for_slots(
            lookup_legacy_keychain_credential,
            |slot| !encrypted_slot_is_usable(&database_path, slot),
        );

        for (slot, secret) in candidates {
            let provider_id = match slot.as_str() {
                "openai" => "openai-compatible",
                "anthropic" => "anthropic",
                "xai" => "xai",
                _ => continue,
            };
            let definition = find_definition(provider_id)?;
            let label = legacy_slot_label(&slot).to_owned();
            match store_provider_secret(&database_path, &definition, &secret) {
                Ok(()) => {
                    mark_shared_credential_stored(&database_path, &definition, true)?;
                    result.imported.push(label.clone());
                    let base_url = resolved_base_url(&definition);
                    match fetch_provider_models(&definition, &base_url) {
                        Ok(models) if !models.is_empty() => {
                            result.verified.push(label.clone());
                            set_import_outcome(
                                &mut result,
                                &slot,
                                "imported",
                                format!(
                                    "{label} was imported and verified with {} models.",
                                    models.len()
                                ),
                            );
                        }
                        Ok(_) => {
                            let detail = format!("{label}: provider returned no models");
                            result.errors.push(detail.clone());
                            set_import_outcome(
                                &mut result,
                                &slot,
                                "imported-unverified",
                                detail,
                            );
                        }
                        Err(error) => {
                            let detail = format!("{label}: {error}");
                            result.errors.push(detail.clone());
                            set_import_outcome(
                                &mut result,
                                &slot,
                                "imported-unverified",
                                detail,
                            );
                        }
                    }
                }
                Err(error) => {
                    let detail = format!("{label}: {error}");
                    result.errors.push(detail.clone());
                    set_import_outcome(&mut result, &slot, "error", detail);
                }
            }
        }
        result.imported.sort();
        result.verified.sort();
        result.missing.sort();
        result.conflicts.sort();
        result.errors.sort();
        result.outcomes.sort_by(|left, right| left.slot_id.cmp(&right.slot_id));
        let already_imported = result
            .outcomes
            .iter()
            .any(|outcome| outcome.status == "already-imported");
        let audit_status = if result.imported.is_empty() && !already_imported {
            "error"
        } else if result.errors.is_empty() && result.conflicts.is_empty() {
            "success"
        } else {
            "partial"
        };
        let _ = store_provider_audit(
            &database_path,
            "provider.credential.import",
            audit_status,
            "legacy-keychain",
            started_at,
        );
        Ok(result)
    })
    .await
    .map_err(|error| format!("credential import task failed: {error}"))?
}

fn legacy_slot_label(slot: &str) -> &'static str {
    match slot {
        "openai" => "OpenAI / Codex",
        "anthropic" => "Anthropic",
        "xai" => "xAI",
        _ => "Unknown provider",
    }
}

fn import_outcome(
    slot: &str,
    status: &str,
    detail: impl Into<String>,
) -> LegacyCredentialImportOutcome {
    LegacyCredentialImportOutcome {
        slot_id: slot.to_owned(),
        label: legacy_slot_label(slot).to_owned(),
        status: status.to_owned(),
        detail: detail.into(),
    }
}

fn set_import_outcome(
    result: &mut LegacyCredentialImportResult,
    slot: &str,
    status: &str,
    detail: impl Into<String>,
) {
    let next = import_outcome(slot, status, detail);
    if let Some(existing) = result
        .outcomes
        .iter_mut()
        .find(|outcome| outcome.slot_id == slot)
    {
        *existing = next;
    } else {
        result.outcomes.push(next);
    }
}

#[cfg(test)]
fn collect_legacy_credentials<F>(
    mut lookup: F,
) -> (BTreeMap<String, String>, LegacyCredentialImportResult)
where
    F: FnMut(&str) -> Result<Option<String>, String>,
{
    collect_legacy_credentials_for_slots(&mut lookup, |_| true)
}

fn collect_legacy_credentials_for_slots<F, S>(
    mut lookup: F,
    mut should_import: S,
) -> (BTreeMap<String, String>, LegacyCredentialImportResult)
where
    F: FnMut(&str) -> Result<Option<String>, String>,
    S: FnMut(&str) -> bool,
{
    let slots: [(&str, &[&str]); 3] = [
        ("openai", &["openai-compatible", "codex", "openai"]),
        ("anthropic", &["anthropic"]),
        ("xai", &["xai"]),
    ];
    let mut candidates = BTreeMap::new();
    let mut result = LegacyCredentialImportResult {
        imported: Vec::new(),
        verified: Vec::new(),
        missing: Vec::new(),
        conflicts: Vec::new(),
        errors: Vec::new(),
        outcomes: Vec::new(),
    };

    for (slot, accounts) in slots {
        if !should_import(slot) {
            set_import_outcome(
                &mut result,
                slot,
                "already-imported",
                format!(
                    "{} is already stored and readable; Keychain was not accessed.",
                    legacy_slot_label(slot)
                ),
            );
            continue;
        }
        let mut values = Vec::new();
        let mut lookup_failed = false;
        for account in accounts {
            match lookup(account) {
                Ok(Some(secret)) if !secret.trim().is_empty() => {
                    if !values.iter().any(|existing: &String| existing == &secret) {
                        values.push(secret);
                    }
                }
                Ok(_) => {}
                Err(error) => {
                    lookup_failed = true;
                    result.errors.push(format!(
                        "{} Keychain account {account}: {error}",
                        legacy_slot_label(slot)
                    ));
                }
            }
        }

        if lookup_failed {
            set_import_outcome(
                &mut result,
                slot,
                "denied",
                format!(
                    "{} could not be read from macOS Keychain. Approve access or enter the key manually.",
                    legacy_slot_label(slot)
                ),
            );
            continue;
        }
        match values.len() {
            0 => {
                let label = legacy_slot_label(slot).to_owned();
                result.missing.push(label.clone());
                set_import_outcome(
                    &mut result,
                    slot,
                    "not-found",
                    format!("No legacy Keychain entry was found for {label}."),
                );
            }
            1 => {
                candidates.insert(slot.to_owned(), values.remove(0));
                set_import_outcome(
                    &mut result,
                    slot,
                    "found",
                    format!(
                        "{} was found and is ready to import.",
                        legacy_slot_label(slot)
                    ),
                );
            }
            _ => {
                let detail = format!(
                    "{} has conflicting legacy Keychain entries; no key was imported",
                    legacy_slot_label(slot)
                );
                result.conflicts.push(detail.clone());
                set_import_outcome(&mut result, slot, "conflict", detail);
            }
        }
    }

    (candidates, result)
}

fn encrypted_slot_is_usable(database_path: &Path, slot: &str) -> bool {
    let provider_id = match slot {
        "openai" => "openai-compatible",
        "anthropic" => "anthropic",
        "xai" => "xai",
        _ => return false,
    };
    find_definition(provider_id)
        .ok()
        .and_then(|definition| read_stored_secret_at(database_path, &definition).ok())
        .flatten()
        .is_some()
}

fn lookup_legacy_keychain_credential(account: &str) -> Result<Option<String>, String> {
    let entry = Entry::new(LEGACY_KEYCHAIN_SERVICE, account)
        .map_err(|error| format!("failed to open Keychain entry: {error}"))?;
    match entry.get_password() {
        Ok(secret) if !secret.trim().is_empty() => Ok(Some(secret)),
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(format!("failed to read Keychain entry: {error}")),
    }
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
        let result = delete_stored_secret(&database_path, &definition);
        if result.is_ok() {
            mark_shared_credential_stored(&database_path, &definition, false)?;
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

pub(crate) fn xai_readiness_with_probe(probe_remote: bool) -> XaiReadiness {
    let definition = find_definition("xai").expect("xAI provider definition must exist");
    let state = credential_state(&definition);
    let credential_status = credential_status_text(state.status).to_owned();
    let subscription_active = grok_subscription_active();
    let health = if let Some(error) = state.error {
        ProviderHealth {
            name: definition.name.to_owned(),
            endpoint: resolved_base_url(&definition),
            available: false,
            detail: error,
        }
    } else if credential_status == "missing" && subscription_active {
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
    } else if credential_status == "stored" && !probe_remote {
        ProviderHealth {
            name: definition.name.to_owned(),
            endpoint: resolved_base_url(&definition),
            available: true,
            detail: "API key stored locally. Run a provider check to verify.".to_owned(),
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
    database_path: Option<&Path>,
) -> Result<ProviderAdapterStatus, String> {
    let base_url = resolved_base_url(definition);
    let resolved_path = match database_path {
        Some(path) => Some(path.to_path_buf()),
        None => storage::resolve_database_path(None).ok(),
    };
    let state = credential_state_at(definition, resolved_path.as_deref());
    let credential_status = state.status;
    let mut health = ProviderHealth {
        name: definition.name.to_owned(),
        endpoint: base_url.clone(),
        available: false,
        detail: if let Some(error) = state.error {
            error
        } else if definition.key_env.is_some() {
            "Not checked. Cloud provider checks run only when requested.".to_owned()
        } else {
            "Not checked.".to_owned()
        },
    };
    let mut models = Vec::new();
    let mut catalog_source = CatalogSource::None;
    let mut verified_available = false;

    if check_remote {
        let credentials_ready = definition.key_env.is_none()
            || matches!(
                credential_status,
                CredentialStatus::Stored
                    | CredentialStatus::Environment
                    | CredentialStatus::NotRequired
            );
        if credentials_ready {
            match fetch_provider_models(definition, &base_url) {
                Ok(next_models) => {
                    verified_available = !next_models.is_empty();
                    health.available = verified_available;
                    health.detail = if verified_available {
                        format!("{} models available.", next_models.len())
                    } else {
                        "Provider returned no usable models.".to_owned()
                    };
                    catalog_source = CatalogSource::Live;
                    models = next_models;
                    if models.is_empty()
                        && matches!(definition.adapter, ProviderAdapter::Anthropic)
                    {
                        models = anthropic_fallback_models();
                        catalog_source = CatalogSource::Fallback;
                    }
                }
                Err(error) => {
                    health.available = false;
                    health.detail = provider_check_error(definition, &error);
                    if definition.id == "codex" {
                        models = codex_static_models();
                        catalog_source = CatalogSource::Static;
                    } else if matches!(definition.adapter, ProviderAdapter::Anthropic) {
                        models = anthropic_fallback_models();
                        catalog_source = CatalogSource::Fallback;
                    }
                }
            }
        } else if credential_status != CredentialStatus::Unreadable {
            health.detail = format!(
                "No API key found for {}. Save your API key in Providers or import the legacy Keychain entry.",
                definition.name
            );
        }
    } else if definition.id == "codex" {
        models = codex_static_models();
        catalog_source = CatalogSource::Static;
    } else if matches!(definition.adapter, ProviderAdapter::Anthropic) {
        models = anthropic_fallback_models();
        catalog_source = CatalogSource::Fallback;
    }

    Ok(ProviderAdapterStatus {
        id: definition.id.to_owned(),
        name: definition.name.to_owned(),
        kind: definition.kind.to_owned(),
        base_url,
        auth_mode: definition.auth_mode.to_owned(),
        credential_status,
        catalog_source,
        verified_available,
        health,
        models,
        capabilities: definition
            .capabilities
            .iter()
            .map(|capability| (*capability).to_owned())
            .collect(),
    })
}

fn provider_check_error(definition: &ProviderDefinition, error: &str) -> String {
    if error.contains("401") || error.contains("403") {
        format!(
            "{} rejected the saved API key. Replace it in Providers.",
            definition.name
        )
    } else {
        error.to_owned()
    }
}

fn credential_status_text(status: CredentialStatus) -> &'static str {
    match status {
        CredentialStatus::NotRequired => "not-required",
        CredentialStatus::Stored => "stored",
        CredentialStatus::Environment => "environment",
        CredentialStatus::Missing => "missing",
        CredentialStatus::Unreadable => "unreadable",
    }
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
    storage::resolve_database_path(None)
        .ok()
        .and_then(|path| storage::load_app_settings(&path).ok())
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
    models.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(models)
}

fn fetch_codex_models(
    definition: &ProviderDefinition,
    base_url: &str,
) -> Result<Vec<LocalModel>, String> {
    let mut models = codex_static_models();
    let api_models = fetch_openai_compatible_models(definition, base_url)?;
    for model in api_models {
        let id = model.id.to_lowercase();
        if (id.contains("codex") || id.starts_with("gpt-5"))
            && !models.iter().any(|entry| entry.id == model.id)
        {
            models.push(model);
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
    verify_provider_model(&definition, &base_url, model_id)?;
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

pub(crate) fn verify_provider_model(
    definition: &ProviderDefinition,
    base_url: &str,
    model_id: &str,
) -> Result<(), String> {
    let models = fetch_provider_models(definition, base_url)?;
    if models.iter().any(|model| model.id == model_id) {
        Ok(())
    } else {
        Err(format!(
            "{} did not verify model {model_id}. Reload models before dispatch.",
            definition.name
        ))
    }
}

pub(crate) fn uses_responses_api(definition: &ProviderDefinition, base_url: &str) -> bool {
    uses_openai_responses_api(definition, base_url)
}

fn dispatch_claude_code(prompt: &str) -> Result<(String, Option<String>), String> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::{Duration, Instant};

    const TIMEOUT: Duration = Duration::from_secs(120);

    let mut child = Command::new("claude")
        .args(["-p", prompt])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to launch Claude Code: {error}"))?;

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture Claude Code stdout".to_owned())?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to capture Claude Code stderr".to_owned())?;

    let stdout_handle = thread::spawn(move || {
        let mut buffer = Vec::new();
        stdout
            .read_to_end(&mut buffer)
            .map(|_| buffer)
            .map_err(|error| format!("failed to read Claude Code stdout: {error}"))
    });
    let stderr_handle = thread::spawn(move || {
        let mut buffer = Vec::new();
        stderr
            .read_to_end(&mut buffer)
            .map(|_| buffer)
            .map_err(|error| format!("failed to read Claude Code stderr: {error}"))
    });

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if started.elapsed() >= TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Claude Code subprocess timed out after 120s".to_owned());
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(error) => {
                return Err(format!("failed while waiting for Claude Code: {error}"));
            }
        }
    };

    let stdout_bytes = stdout_handle
        .join()
        .map_err(|_| "failed to join Claude Code stdout reader".to_owned())??;
    let stderr_bytes = stderr_handle
        .join()
        .map_err(|_| "failed to join Claude Code stderr reader".to_owned())??;

    if !status.success() {
        let stderr = String::from_utf8_lossy(&stderr_bytes);
        return Err(format!(
            "Claude Code exited with status {}: {}",
            status,
            stderr.trim()
        ));
    }

    let content = String::from_utf8_lossy(&stdout_bytes).trim().to_owned();
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
            max_tokens: 4096,
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
            return Err(format!(
                "No API key found for {}. Save your API key in the Providers tab.",
                definition.name
            ));
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

fn resolve_api_key(definition: &ProviderDefinition) -> Result<Option<String>, String> {
    if definition.key_env.is_none() {
        return Ok(None);
    }
    let slot = credential_slot_id(definition);
    // 1. Memory cache — resolved earlier this session
    if let Some(cached) = get_cached_api_key(slot) {
        return Ok(Some(cached));
    }
    // 2. App-encrypted secret store (no OS prompt, stable across rebuilds)
    if let Some(secret) = read_stored_secret(definition)? {
        return Ok(Some(secret));
    }
    // 3. Environment variable fallback (dev override)
    if let Some(env_key) = read_env_api_key(definition) {
        cache_api_key(slot, &env_key);
        return Ok(Some(env_key));
    }
    Ok(None)
}

fn read_env_api_key(definition: &ProviderDefinition) -> Option<String> {
    definition
        .key_env
        .and_then(|name| env::var(name).ok())
        .filter(|value| !value.trim().is_empty())
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
        storage::stable_hash(&format!("{action}:{provider_id}:{created_at}"))
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
        assert_eq!(
            credential_state(&definition).status,
            CredentialStatus::NotRequired
        );
    }

    #[test]
    fn openai_providers_share_one_secret_slot() {
        let codex = find_definition("codex").unwrap();
        let openai = find_definition("openai-compatible").unwrap();

        assert_eq!(credential_slot_id(&codex), "openai");
        assert_eq!(credential_slot_id(&openai), "openai");
    }

    #[test]
    fn non_openai_providers_keep_distinct_secret_slots() {
        let anthropic = find_definition("anthropic").unwrap();
        let xai = find_definition("xai").unwrap();

        assert_eq!(credential_slot_id(&anthropic), "anthropic");
        assert_eq!(credential_slot_id(&xai), "xai");
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
        let models = codex_static_models();
        assert!(models.iter().any(|model| model.id == "gpt-5.5"));
        assert!(models.iter().any(|model| model.id == "codex-mini-latest"));
    }

    #[test]
    fn stale_database_flag_does_not_report_stored_credentials() {
        clear_api_key_cache();
        let dir = std::env::temp_dir().join(format!(
            "agentdeck-stale-credential-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let database_path = dir.join("agentdeck.sqlite3");
        let definition = find_definition("anthropic").unwrap();
        storage::set_provider_credential_stored(&database_path, definition.id, true).unwrap();

        let state = credential_state_at(&definition, Some(&database_path));

        assert_eq!(state.status, CredentialStatus::Missing);
        assert!(!storage::is_provider_credential_stored(
            &database_path,
            definition.id
        ));
        assert!(!dir.join("secret.key").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ciphertext_without_master_key_is_unreadable() {
        clear_api_key_cache();
        let dir = std::env::temp_dir().join(format!(
            "agentdeck-unreadable-credential-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let database_path = dir.join("agentdeck.sqlite3");
        let definition = find_definition("anthropic").unwrap();
        evict_cached_api_key(credential_slot_id(&definition));
        storage::store_provider_secret(&database_path, "anthropic", "ciphertext").unwrap();

        let state = credential_state_at(&definition, Some(&database_path));

        assert_eq!(state.status, CredentialStatus::Unreadable);
        assert!(state.error.unwrap().contains("master key"));
        assert!(!dir.join("secret.key").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn shared_openai_secret_survives_restart_and_deletes_for_both_providers() {
        let dir = std::env::temp_dir().join(format!(
            "agentdeck-shared-secret-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let database_path = dir.join("agentdeck.sqlite3");
        let openai = find_definition("openai-compatible").unwrap();
        let codex = find_definition("codex").unwrap();

        store_provider_secret(&database_path, &openai, "sk-shared-test").unwrap();
        evict_cached_api_key("openai");
        assert_eq!(
            read_stored_secret_at(&database_path, &codex).unwrap(),
            Some("sk-shared-test".to_owned())
        );

        delete_stored_secret(&database_path, &codex).unwrap();
        assert_eq!(
            read_stored_secret_at(&database_path, &openai).unwrap(),
            None
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stored_secret_round_trips_through_resolve_database_path() {
        let dir = std::env::temp_dir().join(format!(
            "agentdeck-resolve-path-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let database_path = dir.join("agentdeck.sqlite3");
        let definition = find_definition("xai").unwrap();

        store_provider_secret(&database_path, &definition, "sk-roundtrip-test").unwrap();
        evict_cached_api_key("xai");
        assert_eq!(
            read_stored_secret_at_path(Some(&database_path), &definition).unwrap(),
            Some("sk-roundtrip-test".to_owned())
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn static_and_fallback_catalogs_are_not_verified_without_checks() {
        let codex = status_for_definition(&find_definition("codex").unwrap(), false, None).unwrap();
        assert_eq!(codex.catalog_source, CatalogSource::Static);
        assert!(!codex.verified_available);
        assert!(!codex.models.is_empty());

        let anthropic =
            status_for_definition(&find_definition("anthropic").unwrap(), false, None).unwrap();
        assert_eq!(anthropic.catalog_source, CatalogSource::Fallback);
        assert!(!anthropic.verified_available);
        assert!(!anthropic.models.is_empty());
    }

    #[test]
    fn legacy_import_deduplicates_shared_openai_entries() {
        let (candidates, result) = collect_legacy_credentials(|account| {
            Ok(match account {
                "openai-compatible" | "codex" => Some("sk-shared".to_owned()),
                _ => None,
            })
        });

        assert_eq!(candidates.get("openai").map(String::as_str), Some("sk-shared"));
        assert!(result.conflicts.is_empty());
        assert!(result.errors.is_empty());
        assert_eq!(
            result
                .outcomes
                .iter()
                .find(|outcome| outcome.slot_id == "openai")
                .map(|outcome| outcome.status.as_str()),
            Some("found")
        );
    }

    #[test]
    fn legacy_import_rejects_conflicting_openai_entries() {
        let (candidates, result) = collect_legacy_credentials(|account| {
            Ok(match account {
                "openai-compatible" => Some("sk-first".to_owned()),
                "codex" => Some("sk-second".to_owned()),
                _ => None,
            })
        });

        assert!(!candidates.contains_key("openai"));
        assert_eq!(result.conflicts.len(), 1);
        assert!(result
            .outcomes
            .iter()
            .any(|outcome| outcome.slot_id == "openai" && outcome.status == "conflict"));
    }

    #[test]
    fn legacy_import_reports_missing_and_denied_entries() {
        let (candidates, result) = collect_legacy_credentials(|account| {
            if account == "xai" {
                Err("access denied".to_owned())
            } else {
                Ok(None)
            }
        });

        assert!(candidates.is_empty());
        assert!(result.missing.contains(&"Anthropic".to_owned()));
        assert!(result.errors.iter().any(|error| error.contains("xAI")));
        assert!(result
            .outcomes
            .iter()
            .any(|outcome| outcome.slot_id == "xai" && outcome.status == "denied"));
    }

    #[test]
    fn legacy_import_skips_slots_already_in_encrypted_store() {
        let mut looked_up = Vec::new();
        let (candidates, result) = collect_legacy_credentials_for_slots(
            |account| {
                looked_up.push(account.to_owned());
                Ok(None)
            },
            |slot| slot != "xai",
        );

        assert!(candidates.is_empty());
        assert!(!looked_up.iter().any(|account| account == "xai"));
        assert!(result.outcomes.iter().any(|outcome| {
            outcome.slot_id == "xai" && outcome.status == "already-imported"
        }));
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
        assert_eq!(grok_status_label("stored", true), "available");
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
            credential_status: "stored".to_owned(),
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

    #[test]
    #[ignore = "live smoke: temporarily corrupts secret.key with backup/restore"]
    fn live_smoke_corrupt_master_key_surfaces_unreadable() {
        let database_path = storage::resolve_database_path(None).expect("home database");
        let key_path = database_path
            .parent()
            .expect("database parent")
            .join("secret.key");
        let backup_path = key_path.with_extension("key.smoke-bak");
        let backup = std::fs::read(&key_path).expect("secret.key should exist");
        std::fs::write(&backup_path, &backup).expect("backup secret.key");
        std::fs::write(&key_path, b"corrupted-smoke-test-key!!!!!!")
            .expect("corrupt secret.key");

        clear_api_key_cache();
        let definition = find_definition("anthropic").unwrap();
        let state = credential_state_at(&definition, Some(&database_path));

        let _ = std::fs::write(&key_path, backup);
        let _ = std::fs::remove_file(&backup_path);

        assert_eq!(state.status, CredentialStatus::Unreadable);
        assert!(state
            .error
            .unwrap_or_default()
            .to_lowercase()
            .contains("master key"));
    }
}
