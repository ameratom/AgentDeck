import type { EnvironmentScan, PreflightResult } from "./types";

export interface DiscoverySummary {
  availableTools: number;
  runningProcesses: number;
  detectedConfigs: number;
  normalizedEntities: number;
}

export function isEnvironmentScan(
  result: EnvironmentScan | PreflightResult | null,
): result is EnvironmentScan {
  return result !== null && "entities" in result;
}

export function summarizeDiscovery(scan: EnvironmentScan): DiscoverySummary {
  return {
    availableTools: scan.tools.filter((tool) => tool.available).length,
    runningProcesses: scan.processes.length,
    detectedConfigs: scan.configs.filter((config) => config.exists).length,
    normalizedEntities: scan.entities.length,
  };
}
