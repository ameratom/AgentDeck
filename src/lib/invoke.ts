import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  ChatRequest,
  ChatResponse,
  ChatPreferences,
  ChatStreamEvent,
  EnvironmentScan,
  LocalModel,
  PreflightResult,
  ChatMessage,
  ProviderAdapterStatus,
  ProviderCheckRequest,
  ProviderCredentialRequest,
  McpInventory,
  McpToggleResult,
  AgentPermissionMatrix,
  HandoffRequest,
  HandoffRun,
  PluginInventory,
  PluginToggleRequest,
  SkillExecutionRecord,
  AppSettings,
  AppSettingsUpdateRequest,
  LocalDeleteResult,
  LocalExportResult,
  AuditEventsPage,
} from "./types";

export function runPreflight(): Promise<PreflightResult> {
  return invoke<PreflightResult>("run_preflight");
}

export function scanEnvironment(): Promise<EnvironmentScan> {
  return invoke<EnvironmentScan>("scan_environment");
}

export function listLmStudioModels(): Promise<LocalModel[]> {
  return invoke<LocalModel[]>("list_lm_studio_models");
}

export function sendChatMessage(request: ChatRequest): Promise<ChatResponse> {
  return invoke<ChatResponse>("send_chat_message", { request });
}

export function streamChatMessage(
  request: ChatRequest,
  onEvent: Channel<ChatStreamEvent>,
): Promise<ChatResponse> {
  return invoke<ChatResponse>("stream_chat_message", { request, onEvent });
}

export function cancelStreamChat(): Promise<void> {
  return invoke<void>("cancel_stream_chat");
}

export function loadChatPreferences(): Promise<ChatPreferences> {
  return invoke<ChatPreferences>("load_chat_preferences");
}

export function saveChatPreferences(
  preferences: ChatPreferences,
): Promise<ChatPreferences> {
  return invoke<ChatPreferences>("save_chat_preferences", { preferences });
}

export function loadChatMessages(
  conversationId: string,
): Promise<ChatMessage[]> {
  return invoke<ChatMessage[]>("load_chat_messages", { conversationId });
}

export function listProviderAdapters(): Promise<ProviderAdapterStatus[]> {
  return invoke<ProviderAdapterStatus[]>("list_provider_adapters");
}

export function checkProviderAdapter(
  request: ProviderCheckRequest,
): Promise<ProviderAdapterStatus> {
  return invoke<ProviderAdapterStatus>("check_provider_adapter", { request });
}

export function saveProviderApiKey(
  request: ProviderCredentialRequest,
): Promise<void> {
  return invoke<void>("save_provider_api_key", { request });
}

export function deleteProviderApiKey(providerId: string): Promise<void> {
  return invoke<void>("delete_provider_api_key", { providerId });
}

export function scanMcpInventory(): Promise<McpInventory> {
  return invoke<McpInventory>("scan_mcp_inventory");
}

export function toggleMcpServer(
  serverId: string,
  enabled: boolean,
  agentId?: string,
): Promise<McpToggleResult> {
  return invoke<McpToggleResult>("toggle_mcp_server", {
    serverId,
    enabled,
    agentId,
  });
}

export function loadAgentPermissions(): Promise<AgentPermissionMatrix> {
  return invoke<AgentPermissionMatrix>("load_agent_permissions");
}

export function setAgentPermission(
  agentId: string,
  action: string,
  allow: boolean,
): Promise<AgentPermissionMatrix> {
  return invoke<AgentPermissionMatrix>("set_agent_permission", {
    agentId,
    action,
    allow,
  });
}

export function runHandoff(request: HandoffRequest): Promise<HandoffRun> {
  return invoke<HandoffRun>("run_handoff", { request });
}

export function loadHandoffRuns(limit = 12): Promise<HandoffRun[]> {
  return invoke<HandoffRun[]>("load_handoff_runs", { limit });
}

export function loadPluginInventory(): Promise<PluginInventory> {
  return invoke<PluginInventory>("load_plugin_inventory");
}

export function setPluginEnabled(
  request: PluginToggleRequest,
): Promise<PluginInventory> {
  return invoke<PluginInventory>("set_plugin_enabled", { request });
}

export function executeSkill(skillId: string): Promise<SkillExecutionRecord> {
  return invoke<SkillExecutionRecord>("execute_skill", {
    request: { skillId },
  });
}

export function loadAppSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("load_app_settings");
}

export function updateAppSettings(
  request: AppSettingsUpdateRequest,
): Promise<AppSettings> {
  return invoke<AppSettings>("update_app_settings", { request });
}

export function completeOnboarding(): Promise<AppSettings> {
  return invoke<AppSettings>("complete_onboarding");
}

export function exportLocalData(): Promise<LocalExportResult> {
  return invoke<LocalExportResult>("export_local_data");
}

export function deleteLocalData(): Promise<LocalDeleteResult> {
  return invoke<LocalDeleteResult>("delete_local_data");
}

export function loadAuditEvents(
  limit = 25,
  offset = 0,
  filter?: string,
): Promise<AuditEventsPage> {
  return invoke<AuditEventsPage>("load_audit_events", { limit, offset, filter });
}


