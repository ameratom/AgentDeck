import type { PluginDefinition, SkillDefinition } from "../../lib/types";
import { requiredPluginNames } from "./pluginModel";

export type PluginFilter = "all" | "enabled" | "disabled";
export type SkillFilter = "all" | "ready";

export function matchesPluginFilter(
  plugin: PluginDefinition,
  filter: PluginFilter,
): boolean {
  switch (filter) {
    case "enabled":
      return plugin.enabled;
    case "disabled":
      return !plugin.enabled;
    default:
      return true;
  }
}

function matchesPluginQuery(
  plugin: PluginDefinition,
  query: string,
): boolean {
  const normalized = query.trim().toLowerCase();
  if (!normalized) {
    return true;
  }
  const haystack = [
    plugin.name,
    plugin.category,
    plugin.description,
    plugin.capabilities.join(" "),
  ]
    .join(" ")
    .toLowerCase();
  return haystack.includes(normalized);
}

export function filterPlugins(
  plugins: PluginDefinition[],
  query: string,
  filter: PluginFilter,
): PluginDefinition[] {
  return plugins.filter(
    (plugin) =>
      matchesPluginFilter(plugin, filter) &&
      matchesPluginQuery(plugin, query),
  );
}

function matchesSkillFilter(
  skill: SkillDefinition,
  filter: SkillFilter,
): boolean {
  switch (filter) {
    case "ready":
      return skill.available;
    default:
      return true;
  }
}

export function matchesSkillQuery(
  skill: SkillDefinition,
  plugins: PluginDefinition[],
  query: string,
): boolean {
  const normalized = query.trim().toLowerCase();
  if (!normalized) {
    return true;
  }
  const haystack = [
    skill.name,
    skill.description,
    skill.tags.join(" "),
    requiredPluginNames(skill, plugins).join(" "),
  ]
    .join(" ")
    .toLowerCase();
  return haystack.includes(normalized);
}

export function filterSkills(
  skills: SkillDefinition[],
  plugins: PluginDefinition[],
  query: string,
  filter: SkillFilter,
): SkillDefinition[] {
  return skills.filter(
    (skill) =>
      matchesSkillFilter(skill, filter) &&
      matchesSkillQuery(skill, plugins, query),
  );
}