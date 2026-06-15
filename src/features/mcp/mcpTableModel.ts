import type { McpServerDefinition } from "../../lib/types";

export type ServerFilter =
  | "all"
  | "enabled"
  | "disabled"
  | "low"
  | "medium"
  | "high";

export function commandLabel(server: McpServerDefinition): string {
  if (server.url) {
    return server.url;
  }
  if (server.command) {
    return `${server.command}${
      server.commandAvailable === false ? " (unavailable)" : ""
    }`;
  }
  return "Remote transport";
}

export function matchesServerFilter(
  server: McpServerDefinition,
  filter: ServerFilter,
): boolean {
  switch (filter) {
    case "enabled":
      return server.enabled;
    case "disabled":
      return !server.enabled;
    case "low":
    case "medium":
    case "high":
      return server.riskLevel === filter;
    default:
      return true;
  }
}

export function matchesServerQuery(
  server: McpServerDefinition,
  query: string,
): boolean {
  const normalized = query.trim().toLowerCase();
  if (!normalized) {
    return true;
  }
  const haystack = [
    server.name,
    server.client,
    server.source,
    commandLabel(server),
    server.envKeys.join(" "),
  ]
    .join(" ")
    .toLowerCase();
  return haystack.includes(normalized);
}

export function filterServers(
  servers: McpServerDefinition[],
  query: string,
  filter: ServerFilter,
): McpServerDefinition[] {
  return servers.filter(
    (server) =>
      matchesServerQuery(server, query) &&
      matchesServerFilter(server, filter),
  );
}