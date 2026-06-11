import { describe, expect, it } from "vitest";
import { summarizeDiscovery } from "./discovery";
import type { EnvironmentScan } from "./types";

describe("discovery summary", () => {
  it("counts only available tools and existing configs", () => {
    const scan = {
      tools: [{ available: true }, { available: false }],
      processes: [{}, {}],
      configs: [{ exists: true }, { exists: false }],
      entities: [{}, {}, {}],
    } as EnvironmentScan;

    expect(summarizeDiscovery(scan)).toEqual({
      availableTools: 1,
      runningProcesses: 2,
      detectedConfigs: 1,
      normalizedEntities: 3,
    });
  });
});
