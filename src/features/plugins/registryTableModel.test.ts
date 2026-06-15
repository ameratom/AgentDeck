import { describe, expect, it } from "vitest";
import type { PluginDefinition, SkillDefinition } from "../../lib/types";
import {
  filterPlugins,
  filterSkills,
  matchesPluginFilter,
  matchesSkillQuery,
} from "./registryTableModel";

function plugin(overrides: Partial<PluginDefinition> = {}): PluginDefinition {
  return {
    id: "agentdeck-core",
    name: "AgentDeck Core",
    description: "Core orchestration plugin",
    category: "agent",
    capabilities: ["routing", "audit"],
    enabled: true,
    ...overrides,
  };
}

function skill(overrides: Partial<SkillDefinition> = {}): SkillDefinition {
  return {
    id: "overnight-batch",
    name: "Overnight Batch",
    description: "Autonomous coding workflow",
    pluginIds: ["agentdeck-core"],
    tags: ["autonomy", "batch"],
    instructions: "Run tasks in order.",
    source: "data/skills/overnight-batch.md",
    available: true,
    ...overrides,
  };
}

describe("registryTableModel", () => {
  it("filters plugins by enabled state", () => {
    const plugins = [
      plugin({ id: "a", enabled: true }),
      plugin({ id: "b", enabled: false }),
    ];
    expect(matchesPluginFilter(plugins[0], "enabled")).toBe(true);
    expect(filterPlugins(plugins, "", "disabled")).toHaveLength(1);
  });

  it("searches plugins across name, category, and capabilities", () => {
    const plugins = [
      plugin({ capabilities: ["routing"] }),
      plugin({
        id: "db",
        name: "Neon Postgres",
        category: "database",
        capabilities: ["sql"],
      }),
    ];
    expect(filterPlugins(plugins, "routing", "all")).toHaveLength(1);
    expect(filterPlugins(plugins, "database", "all")).toHaveLength(1);
  });

  it("filters skills by readiness and required plugin names", () => {
    const plugins = [plugin()];
    const skills = [
      skill(),
      skill({ id: "blocked", available: false, pluginIds: ["missing"] }),
    ];
    expect(filterSkills(skills, plugins, "", "ready")).toHaveLength(1);
    expect(
      matchesSkillQuery(skill(), plugins, "agentdeck core"),
    ).toBe(true);
  });
});