import type { AgentPermissionMatrix, DiscoveredEntity } from "../../lib/types";

export type AgentStatus = "running" | "available" | "configured" | "unavailable";

export function filterAgents(entities: DiscoveredEntity[]): DiscoveredEntity[] {
  return entities
    .filter((entity) => entity.entityType === "agent")
    .sort((left, right) => left.name.localeCompare(right.name));
}

export function agentStatusClass(status: string): string {
  switch (status as AgentStatus) {
    case "running":
      return "agent-status running";
    case "available":
      return "agent-status available";
    case "configured":
      return "agent-status configured";
    default:
      return "agent-status unavailable";
  }
}

export function agentStatusLabel(status: string): string {
  switch (status as AgentStatus) {
    case "running":
      return "Running";
    case "available":
      return "Available";
    case "configured":
      return "Configured";
    default:
      return "Unavailable";
  }
}

export function agentPid(entity: DiscoveredEntity): string | null {
  return entity.metadata.pid ?? null;
}

export function agentConfigCount(entity: DiscoveredEntity): string {
  return entity.metadata.configCount ?? "0";
}

export function agentVersion(entity: DiscoveredEntity): string {
  return entity.metadata.version ?? "—";
}

export function formatAgentId(agentId: string): string {
  return agentId.replace(/^agent:/, "").replace(/-/g, " ");
}

export function formatPermissionAction(action: string): string {
  return action.replace(/-/g, " ");
}

export function permissionAllowed(
  matrix: AgentPermissionMatrix,
  agentId: string,
  action: string,
): boolean {
  return (
    matrix.permissions.find(
      (entry) => entry.agentId === agentId && entry.action === action,
    )?.allow ?? false
  );
}