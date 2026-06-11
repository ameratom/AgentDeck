import type {
  PluginDefinition,
  SkillDefinition,
} from "../../lib/types";

export function pluginCounts(plugins: PluginDefinition[]) {
  return plugins.reduce(
    (counts, plugin) => {
      counts.total += 1;
      if (plugin.enabled) {
        counts.enabled += 1;
      }
      return counts;
    },
    { enabled: 0, total: 0 },
  );
}

export function requiredPluginNames(
  skill: SkillDefinition,
  plugins: PluginDefinition[],
): string[] {
  const names = new Map(plugins.map((plugin) => [plugin.id, plugin.name]));
  return skill.pluginIds.map((id) => names.get(id) ?? id);
}
