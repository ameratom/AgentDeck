import type {
  DiscoveredEntity,
  EnvironmentScan,
  HandoffRequest,
  ProjectConnectorSettings,
  ProviderAdapterStatus,
  ToolStatus,
} from "../../lib/types";
import { filterAgents } from "../agents/agentModel";
import {
  buildApprovalRecord,
  buildHandoffRequest,
  selectDefaultModel,
} from "../handoffs/handoffModel";
import { defaultProjectName, normalizeProjectPath } from "../projects/projectModel";

export type OnboardingStepId =
  | "scan"
  | "inventory"
  | "project"
  | "grok-key"
  | "test-handoff"
  | "connectors"
  | "done";

export const ONBOARDING_STEP_ORDER: OnboardingStepId[] = [
  "scan",
  "inventory",
  "project",
  "grok-key",
  "test-handoff",
  "connectors",
  "done",
];

export interface InventorySummary {
  agentCount: number;
  runningAgents: number;
  availableAgents: number;
  toolCount: number;
  availableTools: number;
  mcpConfigCount: number;
  validMcpConfigs: number;
  highlights: string[];
  gaps: string[];
}

export interface ConnectorExportDefaults {
  filesystemEnabled: boolean;
  gitEnabled: boolean;
  claudeCodeServeEnabled: boolean;
}

export function stepIndex(step: OnboardingStepId): number {
  return ONBOARDING_STEP_ORDER.indexOf(step);
}

export function stepLabel(step: OnboardingStepId): string {
  switch (step) {
    case "scan":
      return "Environment scan";
    case "inventory":
      return "Local inventory";
    case "project":
      return "Project workspace";
    case "grok-key":
      return "Grok API key";
    case "test-handoff":
      return "Test handoff";
    case "connectors":
      return "MCP exports";
    case "done":
      return "Ready";
  }
}

export function nextOnboardingStep(
  current: OnboardingStepId,
): OnboardingStepId | null {
  const index = stepIndex(current);
  if (index < 0 || index >= ONBOARDING_STEP_ORDER.length - 1) {
    return null;
  }
  return ONBOARDING_STEP_ORDER[index + 1] ?? null;
}

export function summarizeInventory(scan: EnvironmentScan): InventorySummary {
  const agents = filterAgents(scan.entities);
  const runningAgents = agents.filter((agent) => agent.status === "running").length;
  const availableAgents = agents.filter((agent) =>
    ["running", "available", "configured"].includes(agent.status),
  ).length;
  const availableTools = scan.tools.filter((tool) => tool.available).length;
  const validMcpConfigs = scan.configs.filter(
    (config) => config.exists && config.valid === true,
  ).length;

  const highlights: string[] = [];
  const gaps: string[] = [];

  if (scan.project) {
    highlights.push(`Active project: ${scan.project.name}`);
  } else {
    gaps.push("No project workspace registered yet");
  }

  if (runningAgents > 0) {
    highlights.push(`${runningAgents} agent${runningAgents === 1 ? "" : "s"} running now`);
  }
  if (availableTools > 0) {
    highlights.push(`${availableTools} local tool${availableTools === 1 ? "" : "s"} detected`);
  }
  if (validMcpConfigs > 0) {
    highlights.push(`${validMcpConfigs} MCP config${validMcpConfigs === 1 ? "" : "s"} validated`);
  }

  if (agents.length === 0) {
    gaps.push("No local agents discovered yet");
  } else if (availableAgents === 0) {
    gaps.push("Agents were found but none are currently available");
  }
  if (availableTools === 0) {
    gaps.push("No CLI tools detected in PATH");
  }
  if (scan.configs.length === 0) {
    gaps.push("No MCP configuration files found");
  } else if (validMcpConfigs === 0) {
    gaps.push("MCP configs exist but none validated cleanly");
  }

  return {
    agentCount: agents.length,
    runningAgents,
    availableAgents,
    toolCount: scan.tools.length,
    availableTools,
    mcpConfigCount: scan.configs.length,
    validMcpConfigs,
    highlights,
    gaps,
  };
}

export function toolAvailable(tools: ToolStatus[], name: string): boolean {
  return tools.some(
    (tool) => tool.name.toLowerCase() === name.toLowerCase() && tool.available,
  );
}

export function suggestConnectorDefaults(scan: EnvironmentScan): ConnectorExportDefaults {
  return {
    filesystemEnabled: true,
    gitEnabled: false,
    claudeCodeServeEnabled: toolAvailable(scan.tools, "claude"),
  };
}

export function buildConnectorExportRequest(
  settings: ConnectorExportDefaults,
): Pick<
  ProjectConnectorSettings,
  "filesystemEnabled" | "gitEnabled" | "claudeCodeServeEnabled"
> {
  return {
    filesystemEnabled: settings.filesystemEnabled,
    gitEnabled: settings.gitEnabled,
    claudeCodeServeEnabled: settings.claudeCodeServeEnabled,
  };
}

export function connectorExportSummary(settings: ProjectConnectorSettings): string[] {
  const enabled: string[] = ["AgentDeck HTTP MCP"];
  if (settings.filesystemEnabled) {
    enabled.push("Filesystem MCP");
  }
  if (settings.gitEnabled) {
    enabled.push("Git MCP");
  }
  if (settings.claudeCodeServeEnabled) {
    enabled.push("Claude Code MCP serve");
  }
  return enabled;
}

export function suggestedProjectPath(scan: EnvironmentScan | null): string {
  if (scan?.project?.path) {
    return scan.project.path;
  }
  return "";
}

export function buildProjectRegistration(path: string, name: string): {
  path: string;
  name: string | null;
} {
  const normalized = normalizeProjectPath(path);
  const trimmedName = name.trim();
  return {
    path: normalized,
    name: trimmedName || defaultProjectName(normalized) || null,
  };
}

export function grokCredentialReady(
  providers: ProviderAdapterStatus[],
): boolean {
  const grok = providers.find((provider) => provider.id === "xai");
  return (
    grok?.credentialStatus === "stored" ||
    grok?.credentialStatus === "environment"
  );
}

export function selectTestHandoffTarget(
  providers: ProviderAdapterStatus[],
): ProviderAdapterStatus | null {
  const lmStudio = providers.find(
    (provider) =>
      provider.id === "lm-studio" &&
      provider.health.available &&
      provider.models.length > 0,
  );
  if (lmStudio) {
    return lmStudio;
  }

  const grok = providers.find(
    (provider) =>
      provider.id === "xai" &&
      grokCredentialReady(providers) &&
      provider.models.length > 0,
  );
  if (grok) {
    return grok;
  }

  return (
    providers.find(
      (provider) => provider.health.available && provider.models.length > 0,
    ) ?? null
  );
}

export function selectOnboardingSourceAgent(
  agents: DiscoveredEntity[],
): DiscoveredEntity | null {
  return (
    agents.find((agent) => agent.status === "running") ??
    agents.find((agent) => agent.status === "available") ??
    agents[0] ??
    null
  );
}

export function buildOnboardingHandoffRequest(args: {
  sourceAgent: DiscoveredEntity;
  provider: ProviderAdapterStatus;
}): HandoffRequest {
  return buildHandoffRequest({
    sourceAgentId: args.sourceAgent.id,
    sourceAgentName: args.sourceAgent.name,
    provider: args.provider,
    modelId: selectDefaultModel(args.provider),
    title: "AgentDeck onboarding check",
    task: "Reply with a one-sentence confirmation that AgentDeck can route a local handoff.",
    context: "This is the first-run onboarding smoke test.",
    approvals: [buildApprovalRecord()],
  });
}