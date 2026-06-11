import { describe, expect, it } from "vitest";
import type { PluginDefinition, SkillDefinition } from "../../lib/types";
import { pluginCounts, requiredPluginNames } from "./pluginModel";

const plugins: PluginDefinition[] = [
  {
    id: "one",
    name: "One",
    description: "First",
    category: "test",
    capabilities: [],
    enabled: true,
  },
  {
    id: "two",
    name: "Two",
    description: "Second",
    category: "test",
    capabilities: [],
    enabled: false,
  },
];

describe("plugin inventory helpers", () => {
  it("counts enabled plugins", () => {
    expect(pluginCounts(plugins)).toEqual({ enabled: 1, total: 2 });
  });

  it("resolves required plugin names", () => {
    const skill: SkillDefinition = {
      id: "skill",
      name: "Skill",
      description: "Test",
      pluginIds: ["one", "missing"],
      tags: [],
      instructions: "Run it.",
      source: "/tmp/skill.md",
      available: false,
    };
    expect(requiredPluginNames(skill, plugins)).toEqual(["One", "missing"]);
  });
});
