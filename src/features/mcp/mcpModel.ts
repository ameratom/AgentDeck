import type { McpConfigSource, McpServerDefinition } from "../../lib/types";

export function existingSources(sources: McpConfigSource[]): McpConfigSource[] {
  return sources.filter((source) => source.exists);
}

export function canToggleServer(server: McpServerDefinition): boolean {
  return server.source.endsWith(".json");
}

export function riskCounts(
  servers: McpServerDefinition[],
): Record<McpServerDefinition["riskLevel"], number> {
  return servers.reduce(
    (counts, server) => {
      counts[server.riskLevel] += 1;
      return counts;
    },
    { low: 0, medium: 0, high: 0 },
  );
}
