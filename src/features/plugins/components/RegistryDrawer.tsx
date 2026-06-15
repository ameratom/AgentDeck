import { useEffect } from "react";
import type { PluginDefinition, SkillDefinition } from "../../../lib/types";
import { requiredPluginNames } from "../pluginModel";

export type RegistryDrawerKind = "plugin" | "skill" | null;

type RegistryDrawerProps = {
  kind: RegistryDrawerKind;
  plugin: PluginDefinition | null;
  skill: SkillDefinition | null;
  plugins: PluginDefinition[];
  busyId: string | null;
  onClose: () => void;
  onToggle: (pluginId: string, enabled: boolean) => void;
  onRun: (skillId: string) => void;
};

export function RegistryDrawer({
  kind,
  plugin,
  skill,
  plugins,
  busyId,
  onClose,
  onToggle,
  onRun,
}: RegistryDrawerProps) {
  const open = kind !== null && (plugin !== null || skill !== null);

  useEffect(() => {
    if (!open) {
      return;
    }
    function handleKeyDown(event: KeyboardEvent): void {
      if (event.key === "Escape") {
        onClose();
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open, onClose]);

  return (
    <>
      <button
        aria-label="Close registry details"
        className={`reg-drawer-scrim ${open ? "open" : ""}`}
        onClick={onClose}
        tabIndex={open ? 0 : -1}
        type="button"
      />
      <aside
        aria-hidden={!open}
        aria-labelledby="reg-drawer-title"
        className={`reg-drawer ${open ? "open" : ""}`}
        role="dialog"
      >
        {kind === "plugin" && plugin ? (
          <>
            <div className="reg-drawer-head">
              <div className="reg-drawer-top">
                <div>
                  <p className="eyebrow">{plugin.category}</p>
                  <h3 id="reg-drawer-title">{plugin.name}</h3>
                </div>
                <button
                  aria-label="Close"
                  className="reg-drawer-close"
                  onClick={onClose}
                  type="button"
                >
                  ✕
                </button>
              </div>
              <div className="reg-drawer-badges">
                <span
                  className={`reg-ebadge ${plugin.enabled ? "on" : ""}`}
                >
                  {plugin.enabled ? "Enabled" : "Disabled"}
                </span>
                <span className={`catpill ${plugin.category}`}>
                  {plugin.category}
                </span>
              </div>
            </div>

            <div className="reg-drawer-body">
              <p className="reg-drawer-desc">{plugin.description}</p>
              <dl className="reg-ddl">
                <div>
                  <dt>Capabilities</dt>
                  <dd>
                    <div className="chiplist">
                      {plugin.capabilities.map((capability) => (
                        <span className="cap" key={capability}>
                          {capability}
                        </span>
                      ))}
                    </div>
                  </dd>
                </div>
                <div>
                  <dt>Plugin ID</dt>
                  <dd className="mono">{plugin.id}</dd>
                </div>
                <div>
                  <dt>Source</dt>
                  <dd className="mono">data/plugins.yaml</dd>
                </div>
              </dl>
            </div>

            <div className="reg-drawer-foot">
              <button
                className="secondary-button"
                disabled={busyId !== null}
                onClick={() => onToggle(plugin.id, !plugin.enabled)}
                type="button"
              >
                {busyId === plugin.id
                  ? "Saving..."
                  : plugin.enabled
                    ? "Disable plugin"
                    : "Enable plugin"}
              </button>
              <p className="reg-drawer-note">AgentDeck only</p>
            </div>
          </>
        ) : null}

        {kind === "skill" && skill ? (
          <>
            <div className="reg-drawer-head">
              <div className="reg-drawer-top">
                <div>
                  <p className="eyebrow">{skill.tags.join(" / ") || "skill"}</p>
                  <h3 id="reg-drawer-title">{skill.name}</h3>
                </div>
                <button
                  aria-label="Close"
                  className="reg-drawer-close"
                  onClick={onClose}
                  type="button"
                >
                  ✕
                </button>
              </div>
              <div className="reg-drawer-badges">
                <span
                  className={`reg-ebadge ${skill.available ? "on" : ""}`}
                >
                  {skill.available ? "Ready" : "Unavailable"}
                </span>
              </div>
            </div>

            <div className="reg-drawer-body">
              <p className="reg-drawer-desc">{skill.description}</p>
              <dl className="reg-ddl">
                <div>
                  <dt>Required plugins</dt>
                  <dd>
                    <div className="chiplist">
                      {requiredPluginNames(skill, plugins).map((name) => (
                        <span className="cap" key={name}>
                          {name}
                        </span>
                      ))}
                    </div>
                  </dd>
                </div>
                <div>
                  <dt>Tags</dt>
                  <dd>
                    <div className="chiplist">
                      {skill.tags.map((tag) => (
                        <span className="cap" key={tag}>
                          {tag}
                        </span>
                      ))}
                    </div>
                  </dd>
                </div>
                <div>
                  <dt>Source</dt>
                  <dd className="mono">{skill.source}</dd>
                </div>
              </dl>
              <div className="reg-instr">
                <p className="reg-pane-eyebrow">Instructions</p>
                <p>{skill.instructions}</p>
              </div>
            </div>

            <div className="reg-drawer-foot">
              <button
                disabled={busyId !== null || !skill.available}
                onClick={() => onRun(skill.id)}
                type="button"
              >
                {busyId === skill.id ? "Logging..." : "Log execution"}
              </button>
              <p className="reg-drawer-note">writes audit</p>
            </div>
          </>
        ) : null}
      </aside>
    </>
  );
}