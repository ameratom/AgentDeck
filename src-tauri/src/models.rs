use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    pub name: String,
    pub available: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_source: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderHealth {
    pub name: String,
    pub endpoint: String,
    pub available: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentScan {
    pub scanned_at: String,
    pub project: Option<ProjectContext>,
    pub tools: Vec<ToolStatus>,
    pub providers: Vec<ProviderHealth>,
    pub processes: Vec<DetectedProcess>,
    pub configs: Vec<DetectedConfig>,
    pub entities: Vec<DiscoveredEntity>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectContext {
    pub id: String,
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedProcess {
    pub id: String,
    pub pid: u32,
    pub name: String,
    pub executable: Option<String>,
    pub command: Option<String>,
    pub category: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedConfig {
    pub id: String,
    pub kind: String,
    pub path: String,
    pub exists: bool,
    pub format: Option<String>,
    pub valid: Option<bool>,
    pub top_level_keys: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredEntity {
    pub id: String,
    pub entity_type: String,
    pub name: String,
    pub status: String,
    pub source: String,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightResult {
    pub checked_at: String,
    pub tools: Vec<ToolStatus>,
    pub providers: Vec<ProviderHealth>,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModel {
    pub id: String,
    pub owned_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: Option<String>,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub model: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    pub conversation_id: String,
    pub project_id: Option<String>,
    pub provider_id: String,
    pub model: String,
    pub messages: Vec<ChatMessageInput>,
    #[serde(default)]
    pub enable_agent_tools: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatPreferences {
    pub last_provider_id: String,
    pub last_model_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "event", content = "data")]
pub enum ChatStreamEvent {
    Token { content: String },
    Done {
        finish_reason: Option<String>,
        message: ChatMessage,
    },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageInput {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResponse {
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAdapterStatus {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub auth_mode: String,
    pub credential_status: CredentialStatus,
    pub catalog_source: CatalogSource,
    pub verified_available: bool,
    pub health: ProviderHealth,
    pub models: Vec<LocalModel>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CredentialStatus {
    NotRequired,
    Stored,
    Environment,
    Missing,
    ImportFailed,
    Unreadable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CatalogSource {
    None,
    Live,
    Static,
    Fallback,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialRequest {
    pub provider_id: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCheckRequest {
    pub provider_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyCredentialImportResult {
    pub imported: Vec<String>,
    pub verified: Vec<String>,
    pub missing: Vec<String>,
    pub conflicts: Vec<String>,
    pub errors: Vec<String>,
    pub outcomes: Vec<LegacyCredentialImportOutcome>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyCredentialImportOutcome {
    pub slot_id: String,
    pub label: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpInventory {
    pub scanned_at: String,
    pub sources: Vec<McpConfigSource>,
    pub servers: Vec<McpServerDefinition>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConfigSource {
    pub id: String,
    pub client: String,
    pub path: String,
    pub exists: bool,
    pub parsed: bool,
    pub server_count: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerDefinition {
    pub id: String,
    pub name: String,
    pub client: String,
    pub transport: String,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub url: Option<String>,
    pub env_keys: Vec<String>,
    pub source: String,
    pub enabled: bool,
    pub command_available: Option<bool>,
    pub declared_tools: Vec<String>,
    pub risk_level: String,
    pub risk_reasons: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffRequest {
    pub project_id: Option<String>,
    pub source_agent_id: String,
    pub source_agent_name: String,
    pub target_provider_id: String,
    pub target_provider_name: String,
    pub target_model_id: String,
    pub title: String,
    pub task: String,
    pub context: String,
    pub approvals: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffRun {
    pub id: String,
    pub project_id: Option<String>,
    pub thread_id: String,
    pub source_agent_id: String,
    pub source_agent_name: String,
    pub target_provider_id: String,
    pub target_provider_name: String,
    pub target_model_id: String,
    pub title: String,
    pub task: String,
    pub context: String,
    pub status: String,
    pub output: String,
    pub error: Option<String>,
    pub approvals: Vec<String>,
    pub audit_ref: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub entity_type: String,
    pub status: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphSnapshot {
    pub scanned_at: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEventRecord {
    pub id: String,
    pub action: String,
    pub status: String,
    pub model: String,
    pub conversation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub duration_ms: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEventsPage {
    pub events: Vec<AuditEventRecord>,
    pub total: u32,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub capabilities: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub plugin_ids: Vec<String>,
    pub tags: Vec<String>,
    pub instructions: String,
    pub source: String,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInventory {
    pub loaded_at: String,
    pub plugins: Vec<PluginDefinition>,
    pub skills: Vec<SkillDefinition>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginToggleRequest {
    pub plugin_id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillExecutionRequest {
    pub skill_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillExecutionRecord {
    pub id: String,
    pub skill_id: String,
    pub skill_name: String,
    pub status: String,
    pub audit_ref: String,
    pub created_at: String,
    pub output: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToggleResult {
    pub server_id: String,
    pub server_name: String,
    pub enabled: bool,
    pub config_path: String,
    pub backup_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectConnectorSettings {
    pub project_id: String,
    pub project_name: String,
    pub project_path: String,
    pub filesystem_enabled: bool,
    pub git_enabled: bool,
    pub claude_code_serve_enabled: bool,
    pub grok_mcp_enabled: bool,
    pub xai_research_mcp_enabled: bool,
    pub claude_export_path: String,
    pub codex_export_path: String,
    pub claude_code_serve_export_path: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProjectConnectorSettingsRequest {
    pub filesystem_enabled: bool,
    pub git_enabled: bool,
    pub claude_code_serve_enabled: bool,
    pub grok_mcp_enabled: bool,
    pub xai_research_mcp_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPermission {
    pub agent_id: String,
    pub action: String,
    pub allow: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPermissionMatrix {
    pub agents: Vec<String>,
    pub actions: Vec<String>,
    pub permissions: Vec<AgentPermission>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub redact_sensitive_exports: bool,
    pub crash_safe_logging: bool,
    pub grok_subscription_active: bool,
    pub onboarding_complete: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsUpdateRequest {
    pub redact_sensitive_exports: bool,
    pub crash_safe_logging: bool,
    pub grok_subscription_active: bool,
    pub onboarding_complete: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalExportResult {
    pub exported_at: String,
    pub path: String,
    pub redacted: bool,
    pub bytes_written: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalDeleteResult {
    pub deleted_at: String,
    pub path: String,
    pub removed_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterRule {
    pub id: String,
    pub priority: i32,
    pub name: String,
    pub enabled: bool,
    pub source_agent_id: Option<String>,
    pub keyword: Option<String>,
    pub target_provider_id: String,
    pub target_model_id: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterRuleMatrix {
    pub loaded_at: String,
    pub rules: Vec<RouterRule>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRouterRulesRequest {
    pub rules: Vec<RouterRule>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffRouteRequest {
    pub source_agent_id: String,
    pub title: String,
    pub task: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffRouteSuggestion {
    pub rule_id: String,
    pub rule_name: String,
    pub target_provider_id: String,
    pub target_model_id: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkspace {
    pub id: String,
    pub name: String,
    pub path: String,
    pub exists: bool,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkspaceList {
    pub loaded_at: String,
    pub projects: Vec<ProjectWorkspace>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterProjectRequest {
    pub path: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookEndpoint {
    pub id: String,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub event_types: Vec<String>,
    pub has_secret: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookEndpointMatrix {
    pub loaded_at: String,
    pub plugin_enabled: bool,
    pub endpoints: Vec<WebhookEndpoint>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveWebhookEndpointsRequest {
    pub endpoints: Vec<WebhookEndpoint>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookSecretRequest {
    pub endpoint_id: String,
    pub secret: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookDispatchRequest {
    pub endpoint_id: String,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub approvals: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookDispatchResult {
    pub endpoint_id: String,
    pub event_type: String,
    pub status_code: u16,
    pub success: bool,
    pub detail: String,
    pub audit_ref: String,
    pub dispatched_at: String,
}
