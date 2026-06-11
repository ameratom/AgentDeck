import { useEffect, useState } from "react";
import {
  executeSkill,
  loadPluginInventory,
  setPluginEnabled,
} from "../../lib/invoke";
import type { PluginInventory } from "../../lib/types";
import { pluginCounts, requiredPluginNames } from "./pluginModel";

export function PluginsView() {
  const [inventory, setInventory] = useState<PluginInventory | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [status, setStatus] = useState("Loading plugin and skill registry.");

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

  const counts = inventory
    ? pluginCounts(inventory.plugins)
    : { enabled: 0, total: 0 };
  const availableSkills =
    inventory?.skills.filter((skill) => skill.available).length ?? 0;

  return (
    <section className="workspace plugins-workspace">
      <header>
        <div>
          <p className="eyebrow">Phase 8 / Registry</p>
          <h2>Plugins &amp; Skills</h2>
          <p>
            Manage AgentDeck integration modules and inspect reusable workflows
            loaded from local YAML and markdown files.
          </p>
        </div>
        <span className="phase-badge">Local registry</span>
      </header>

      <div className="plugin-status" role="status">
        <span className={busyId ? "pulse indicator" : "indicator"} />
        <span>{status}</span>
        <span className="registry-count">{counts.enabled}/{counts.total} plugins</span>
        <span className="registry-count">{availableSkills} skills ready</span>
      </div>

      <section aria-labelledby="plugins-heading">
        <div className="section-heading">
          <div>
            <p className="eyebrow">Integration modules</p>
            <h3 id="plugins-heading">Plugin Registry</h3>
          </div>
          <small>Settings persist in AgentDeck only</small>
        </div>
        <div className="plugin-grid">
          {inventory?.plugins.map((plugin) => (
            <article className="plugin-card" key={plugin.id}>
              <div className="plugin-card-heading">
                <div>
                  <p className="eyebrow">{plugin.category}</p>
                  <h3>{plugin.name}</h3>
                </div>
                <span className={plugin.enabled ? "plugin-state enabled" : "plugin-state"}>
                  {plugin.enabled ? "Enabled" : "Disabled"}
                </span>
              </div>
              <p>{plugin.description}</p>
              <div className="tag-row">
                {plugin.capabilities.map((capability) => (
                  <span key={capability}>{capability}</span>
                ))}
              </div>
              <button
                className={plugin.enabled ? "secondary-button" : ""}
                disabled={busyId !== null}
                onClick={() => void togglePlugin(plugin.id, !plugin.enabled)}
                type="button"
              >
                {busyId === plugin.id
                  ? "Saving..."
                  : plugin.enabled
                    ? "Disable"
                    : "Enable"}
              </button>
            </article>
          ))}
        </div>
      </section>

      <section className="skills-section" aria-labelledby="skills-heading">
        <div className="section-heading">
          <div>
            <p className="eyebrow">Reusable workflows</p>
            <h3 id="skills-heading">Skill Library</h3>
          </div>
          <small>Execution action writes an audit record</small>
        </div>
        <div className="skill-grid">
          {inventory?.skills.map((skill) => (
            <article className="skill-card" key={skill.id}>
              <div className="plugin-card-heading">
                <div>
                  <p className="eyebrow">{skill.tags.join(" / ")}</p>
                  <h3>{skill.name}</h3>
                </div>
                <span className={skill.available ? "plugin-state enabled" : "plugin-state"}>
                  {skill.available ? "Ready" : "Unavailable"}
                </span>
              </div>
              <p>{skill.description}</p>
              <dl>
                <div>
                  <dt>Required plugins</dt>
                  <dd>
                    {requiredPluginNames(skill, inventory.plugins).join(", ")}
                  </dd>
                </div>
                <div>
                  <dt>Source</dt>
                  <dd>{skill.source}</dd>
                </div>
              </dl>
              <details>
                <summary>View instructions</summary>
                <p>{skill.instructions}</p>
              </details>
              <button
                disabled={busyId !== null || !skill.available}
                onClick={() => void logExecution(skill.id)}
                type="button"
              >
                {busyId === skill.id ? "Logging..." : "Log execution"}
              </button>
            </article>
          ))}
        </div>
      </section>
    </section>
  );
}

function formatError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
