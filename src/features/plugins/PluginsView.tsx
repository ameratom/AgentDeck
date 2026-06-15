import { useEffect, useMemo, useState } from "react";
import {
  executeSkill,
  loadPluginInventory,
  setPluginEnabled,
} from "../../lib/invoke";
import type { PluginInventory } from "../../lib/types";
import { PluginTable } from "./components/PluginTable";
import { RegistryDrawer, type RegistryDrawerKind } from "./components/RegistryDrawer";
import { SkillTable } from "./components/SkillTable";
import { pluginCounts } from "./pluginModel";

export function PluginsView() {
  const [inventory, setInventory] = useState<PluginInventory | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [status, setStatus] = useState("Loading plugin and skill registry.");
  const [drawerKind, setDrawerKind] = useState<RegistryDrawerKind>(null);
  const [selectedPluginId, setSelectedPluginId] = useState<string | null>(null);
  const [selectedSkillId, setSelectedSkillId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load(): Promise<void> {
      try {
        const nextInventory = await loadPluginInventory();
        if (!cancelled) {
          setInventory(nextInventory);
          setStatus(
            `Loaded ${nextInventory.plugins.length} plugins and ${nextInventory.skills.length} skills.`,
          );
        }
      } catch (error) {
        if (!cancelled) {
          setStatus(`Registry load failed: ${formatError(error)}`);
        }
      }
    }

    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  async function togglePlugin(pluginId: string, enabled: boolean) {
    setBusyId(pluginId);
    setStatus(`${enabled ? "Enabling" : "Disabling"} plugin...`);
    try {
      const nextInventory = await setPluginEnabled({ pluginId, enabled });
      setInventory(nextInventory);
      setStatus(`Plugin ${enabled ? "enabled" : "disabled"}.`);
    } catch (error) {
      setStatus(`Plugin update failed: ${formatError(error)}`);
    } finally {
      setBusyId(null);
    }
  }

  async function logExecution(skillId: string) {
    setBusyId(skillId);
    setStatus("Logging skill execution...");
    try {
      const execution = await executeSkill(skillId);
      setStatus(
        `${execution.skillName} ${execution.status} (${execution.auditRef}).`,
      );
    } catch (error) {
      setStatus(`Skill execution failed: ${formatError(error)}`);
    } finally {
      setBusyId(null);
    }
  }

  function openPluginDrawer(pluginId: string): void {
    setDrawerKind("plugin");
    setSelectedPluginId(pluginId);
    setSelectedSkillId(null);
  }

  function openSkillDrawer(skillId: string): void {
    setDrawerKind("skill");
    setSelectedSkillId(skillId);
    setSelectedPluginId(null);
  }

  function closeDrawer(): void {
    setDrawerKind(null);
    setSelectedPluginId(null);
    setSelectedSkillId(null);
  }

  const counts = inventory
    ? pluginCounts(inventory.plugins)
    : { enabled: 0, total: 0 };
  const availableSkills =
    inventory?.skills.filter((skill) => skill.available).length ?? 0;
  const plugins = inventory?.plugins ?? [];
  const skills = inventory?.skills ?? [];

  const selectedPlugin = useMemo(
    () => plugins.find((plugin) => plugin.id === selectedPluginId) ?? null,
    [plugins, selectedPluginId],
  );
  const selectedSkill = useMemo(
    () => skills.find((skill) => skill.id === selectedSkillId) ?? null,
    [skills, selectedSkillId],
  );

  return (
    <section className="workspace plugins-workspace plugins-workspace--compact">
      <header className="reg-compact-header">
        <div>
          <p className="eyebrow">Phase 8 / Registry</p>
          <h2>Plugins &amp; Skills</h2>
          <p className="reg-compact-subtitle">
            Manage AgentDeck integration modules and inspect reusable workflows
            loaded from local YAML and markdown files.
          </p>
        </div>
        <div className="reg-compact-header-meta">
          <span className="phase-badge">Local registry</span>
          <div className="reg-meta-pills">
            <span className="registry-count">
              {counts.enabled}/{counts.total} plugins
            </span>
            <span className={`registry-count ${availableSkills > 0 ? "ok" : ""}`}>
              {availableSkills} skills ready
            </span>
          </div>
        </div>
      </header>

      <div className="reg-compact-status" role="status">
        <span className={busyId ? "pulse indicator" : "indicator"} />
        <span>{status}</span>
      </div>

      <div className="reg-panes">
        <PluginTable
          busyId={busyId}
          onRowClick={openPluginDrawer}
          onToggle={(pluginId, enabled) => void togglePlugin(pluginId, enabled)}
          plugins={plugins}
        />
        <SkillTable
          busyId={busyId}
          onRowClick={openSkillDrawer}
          onRun={(skillId) => void logExecution(skillId)}
          plugins={plugins}
          skills={skills}
        />
      </div>

      <RegistryDrawer
        busyId={busyId}
        kind={drawerKind}
        onClose={closeDrawer}
        onRun={(skillId) => void logExecution(skillId)}
        onToggle={(pluginId, enabled) => void togglePlugin(pluginId, enabled)}
        plugin={selectedPlugin}
        plugins={plugins}
        skill={selectedSkill}
      />
    </section>
  );
}

function formatError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}