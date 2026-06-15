import { describe, expect, it } from "vitest";
import type { McpServerDefinition } from "../../lib/types";
import {
  commandLabel,
  filterServers,
  matchesServerFilter,
  matchesServerQuery,
} from "./mcpTableModel";

function server(
  overrides: Partial<McpServerDefinition> = {},
): McpServerDefinition {
  return {
    id: "server-1",
    name: "filesystem",
    client: "Claude Code",
    transport: "stdio",
    command: "npx",
    args: [],
    cwd: null,
    url: null,
    envKeys: [],
    source: "/Users/me/.claude.json",
    enabled: true,
    commandAvailable: true,
    declaredTools: [],
    riskLevel: "low",
    riskReasons: [],
    ...overrides,
  };
}

describe("mcpTableModel", () => {
  it("builds command labels from url, command, or fallback", () => {
    expect(commandLabel(server({ url: "http://127.0.0.1:7823/mcp" }))).toBe(
      "http://127.0.0.1:7823/mcp",
    );
    expect(
      commandLabel(server({ command: "node", commandAvailable: false })),
    ).toBe("node (unavailable)");
    expect(commandLabel(server({ command: null, url: null }))).toBe(
      "Remote transport",
    );
  });

  it("filters by enabled state and risk level", () => {
    const enabled = server({ enabled: true, riskLevel: "low" });
    const disabled = server({ id: "server-2", enabled: false, riskLevel: "high" });

    expect(matchesServerFilter(enabled, "enabled")).toBe(true);
    expect(matchesServerFilter(disabled, "enabled")).toBe(false);
    expect(matchesServerFilter(disabled, "high")).toBe(true);
  });

  it("searches across name, client, source, command, and env keys", () => {
    const match = server({
      name: "git",
      client: "Codex",
      source: "/Users/me/.codex/config.toml",
      envKeys: ["GITHUB_TOKEN"],
    });

    expect(matchesServerQuery(match, "codex")).toBe(true);
    expect(matchesServerQuery(match, "github_token")).toBe(true);
    expect(matchesServerQuery(match, "missing")).toBe(false);
  });

  it("combines query and chip filters", () => {
    const servers = [
      server({ id: "a", name: "alpha", enabled: true, riskLevel: "low" }),
      server({ id: "b", name: "beta", enabled: false, riskLevel: "medium" }),
    ];

    expect(filterServers(servers, "beta", "all")).toEqual([servers[1]]);
    expect(filterServers(servers, "", "enabled")).toEqual([servers[0]]);
  });
});