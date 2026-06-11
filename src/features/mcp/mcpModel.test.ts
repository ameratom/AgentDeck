import { describe, expect, it } from "vitest";
import { canToggleServer, existingSources, riskCounts } from "./mcpModel";
import type { McpConfigSource, McpServerDefinition } from "../../lib/types";

const server = (riskLevel: "low" | "medium" | "high"): McpServerDefinition => ({
  id: `server:${riskLevel}`,
  name: riskLevel,
  client: "Test",
  transport: "stdio",
  command: "test",
  args: [],
  cwd: null,
  url: null,
  envKeys: [],
  source: "/tmp/test.json",
  enabled: true,
  commandAvailable: true,
  declaredTools: [],
  riskLevel,
  riskReasons: [],
});

describe("MCP inventory helpers", () => {
  it("counts deterministic risk labels", () => {
    expect(riskCounts([server("low"), server("high"), server("high")])).toEqual({
      low: 1,
      medium: 0,
      high: 2,
    });
  });

  it("allows toggling only for json config sources", () => {
    expect(canToggleServer(server("low"))).toBe(true);
    expect(
      canToggleServer({
        ...server("low"),
        source: "/tmp/config.toml",
      }),
    ).toBe(false);
  });

  it("keeps only detected config sources", () => {
    const sources: McpConfigSource[] = [
      {
        id: "one",
        client: "Test",
        path: "/one",
        exists: true,
        parsed: true,
        serverCount: 1,
        error: null,
      },
      {
        id: "two",
        client: "Test",
        path: "/two",
        exists: false,
        parsed: false,
        serverCount: 0,
        error: null,
      },
    ];
    expect(existingSources(sources)).toHaveLength(1);
  });
});
