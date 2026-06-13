export interface ToolStatus {
  name: string;
  available: boolean;
  version: string | null;
  path: string | null;
  error: string | null;
}

export interface ProviderHealth {
  name: string;
  endpoint: string;
  available: boolean;
  detail: string;
}

export interface DetectedProcess {
  id: string;
  pid: number;
  name: string;
  executable: string | null;
  command: string | null;
  category: string;
}

export interface DetectedConfig {
  id: string;
  kind: string;
  path: string;
  exists: boolean;
  format: string | null;
  valid: boolean | null;
  topLevelKeys: string[];
  error: string | null;
}

export interface DiscoveredEntity {
  id: string;
  entityType: string;
  name: string;
  status: string;
  source: string;
  metadata: Record<string, string>;
}

export interface EnvironmentScan {
  scannedAt: string;
  project: ProjectContext | null;
  tools: ToolStatus[];
  providers: ProviderHealth[];
  processes: DetectedProcess[];
  configs: DetectedConfig[];
  entities: DiscoveredEntity[];
}

export interface ProjectContext {
  id: string;
  name: string;
  path: string;
}

export interface PreflightResult {
  checkedAt: string;
  tools: ToolStatus[];
  providers: ProviderHealth[];
  ready: boolean;
}

export interface LocalModel {
  id: string;
  ownedBy: string | null;
}

export type ChatRole = "system" | "user" | "assistant";

export interface ChatMessageInput {
  role: ChatRole;
  content: string;
}

export interface ChatMessage {
  id: string | null;
  conversationId: string;
  role: ChatRole;
  content: string;
  model: string;
  createdAt: string | null;
}

export interface ChatRequest {
  conversationId: string;
  projectId?: string | null;
  providerId: string;
  model: string;
  messages: ChatMessageInput[];
  enableAgentTools?: boolean;
}

export interface ChatPreferences {
  lastProviderId: string;
  lastModelId: string;
}

export type ChatStreamEvent =
  | { event: "token"; data: { content: string } }
  | {
      event: "done";
      data: { finishReason: string | null; message: ChatMessage };
    }
  | { event: "error"; data: { message: string } };

export interface ChatResponse {
  message: ChatMessage;
  finishReason: string | null;
}

export type CredentialStatus =
  | "not-required"
  | "stored"
  | "environment"
  | "missing"
  | "import-failed"
  | "unreadable";

export type CatalogSource = "none" | "live" | "static" | "fallback";

export interface LegacyCredentialImportOutcome {
  slotId: string;
  label: string;
  status:
    | "found"
    | "already-imported"
    | "imported"
    | "imported-unverified"
    | "not-found"
    | "denied"
    | "conflict"
    | "error";
  detail: string;
}

export interface LegacyCredentialImportResult {
  imported: string[];
  verified: string[];
  missing: string[];
  conflicts: string[];
  errors: string[];
  outcomes: LegacyCredentialImportOutcome[];
}

export interface ProviderAdapterStatus {
  id: string;
  name: string;
  kind: string;
  baseUrl: string;
  authMode: string;
  credentialStatus: CredentialStatus;
  catalogSource: CatalogSource;
  verifiedAvailable: boolean;
  health: ProviderHealth;
  models: LocalModel[];
  capabilities: string[];
}

export interface ProviderCredentialRequest {
  providerId: string;
  apiKey: string;
}

export interface ProviderCheckRequest {
  providerId: string;
}

export interface McpInventory {
  scannedAt: string;
  sources: McpConfigSource[];
  servers: McpServerDefinition[];
}

export interface McpConfigSource {
  id: string;
  client: string;
  path: string;
  exists: boolean;
  parsed: boolean;
  serverCount: number;
  error: string | null;
}

export interface McpServerDefinition {
  id: string;
  name: string;
  client: string;
  transport: string;
  command: string | null;
  args: string[];
  cwd: string | null;
  url: string | null;
  envKeys: string[];
  source: string;
  enabled: boolean;
  commandAvailable: boolean | null;
  declaredTools: string[];
  riskLevel: "low" | "medium" | "high";
  riskReasons: string[];
}

export interface McpToggleResult {
  serverId: string;
  serverName: string;
  enabled: boolean;
  configPath: string;
  backupPath: string;
}

export interface ProjectConnectorSettings {
  projectId: string;
  projectName: string;
  projectPath: string;
  filesystemEnabled: boolean;
  gitEnabled: boolean;
  claudeCodeServeEnabled: boolean;
  claudeExportPath: string;
  codexExportPath: string;
  claudeCodeServeExportPath: string;
  updatedAt: string;
}

export interface GrokMcpBridgeStatus {
  path: string;
  exists: boolean;
  hasKey: boolean;
  updatedAt: string | null;
  detail: string;
}

export interface SecureTunnelStatus {
  configured: boolean;
  running: boolean;
  ready: boolean;
  pid: number | null;
  configPath: string;
  adminUrl: string | null;
  logPath: string;
  detail: string;
}

export interface ProjectWorkspace {
  id: string;
  name: string;
  path: string;
  exists: boolean;
  active: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface ProjectWorkspaceList {
  loadedAt: string;
  projects: ProjectWorkspace[];
}

export interface RouterRule {
  id: string;
  priority: number;
  name: string;
  enabled: boolean;
  sourceAgentId: string | null;
  keyword: string | null;
  targetProviderId: string;
  targetModelId: string | null;
  updatedAt: string;
}

export interface RouterRuleMatrix {
  loadedAt: string;
  rules: RouterRule[];
}

export interface HandoffRouteSuggestion {
  ruleId: string;
  ruleName: string;
  targetProviderId: string;
  targetModelId: string | null;
  reason: string;
}

export interface AgentPermission {
  agentId: string;
  action: string;
  allow: boolean;
}

export interface AgentPermissionMatrix {
  agents: string[];
  actions: string[];
  permissions: AgentPermission[];
}

export interface HandoffRequest {
  projectId?: string | null;
  sourceAgentId: string;
  sourceAgentName: string;
  targetProviderId: string;
  targetProviderName: string;
  targetModelId: string;
  title: string;
  task: string;
  context: string;
  approvals: string[];
}

export interface HandoffRun {
  id: string;
  projectId: string | null;
  threadId: string;
  sourceAgentId: string;
  sourceAgentName: string;
  targetProviderId: string;
  targetProviderName: string;
  targetModelId: string;
  title: string;
  task: string;
  context: string;
  status: "running" | "completed" | "failed";
  output: string;
  error: string | null;
  approvals: string[];
  auditRef: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface PluginDefinition {
  id: string;
  name: string;
  description: string;
  category: string;
  capabilities: string[];
  enabled: boolean;
}

export interface SkillDefinition {
  id: string;
  name: string;
  description: string;
  pluginIds: string[];
  tags: string[];
  instructions: string;
  source: string;
  available: boolean;
}

export interface PluginInventory {
  loadedAt: string;
  plugins: PluginDefinition[];
  skills: SkillDefinition[];
}

export interface PluginToggleRequest {
  pluginId: string;
  enabled: boolean;
}

export interface SkillExecutionRecord {
  id: string;
  skillId: string;
  skillName: string;
  status: string;
  auditRef: string;
  createdAt: string;
  output: string;
}

export interface AppSettings {
  redactSensitiveExports: boolean;
  crashSafeLogging: boolean;
  grokSubscriptionActive: boolean;
  onboardingComplete: boolean;
}

export interface AppSettingsUpdateRequest {
  redactSensitiveExports: boolean;
  crashSafeLogging: boolean;
  grokSubscriptionActive: boolean;
  onboardingComplete: boolean;
}

export interface LocalExportResult {
  exportedAt: string;
  path: string;
  redacted: boolean;
  bytesWritten: number;
}

export interface LocalDeleteResult {
  deletedAt: string;
  path: string;
  removedFiles: string[];
}

export interface AuditEventRecord {
  id: string;
  action: string;
  status: string;
  model: string;
  conversationId: string;
  durationMs: number;
  createdAt: string;
}

export interface AuditEventsPage {
  events: AuditEventRecord[];
  total: number;
  limit: number;
  offset: number;
}
