import { useEffect, useState } from "react";
import {
  deleteLocalData,
  exportLocalData,
  loadAppSettings,
  loadRouterRules,
  saveRouterRules,
  updateAppSettings,
} from "../../lib/invoke";
import type { AppSettings, RouterRule } from "../../lib/types";
import {
  ROUTER_PROVIDER_OPTIONS,
  ROUTER_SOURCE_OPTIONS,
  createRouterRule,
  moveRouterRule,
  removeRouterRule,
  updateRouterRule,
} from "./routerModel";

const DEFAULT_SETTINGS: AppSettings = {
  redactSensitiveExports: true,
  crashSafeLogging: true,
  grokSubscriptionActive: true,
  onboardingComplete: false,
};

export function SettingsView() {
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [busyAction, setBusyAction] = useState<"export" | "delete" | null>(null);
  const [routerRules, setRouterRules] = useState<RouterRule[]>([]);
  const [savingRouter, setSavingRouter] = useState(false);
  const [status, setStatus] = useState("Loading hardening settings.");

  useEffect(() => {
    let cancelled = false;

    async function load(): Promise<void> {
      try {
        const [nextSettings, routerMatrix] = await Promise.all([
          loadAppSettings(),
          loadRouterRules(),
        ]);
        if (!cancelled) {
          setSettings(nextSettings);
          setRouterRules(routerMatrix.rules);
          setStatus("Hardening settings loaded.");
        }
      } catch (error) {
        if (!cancelled) {
          setStatus(`Settings load failed: ${formatError(error)}`);
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  async function saveSettings(nextSettings: AppSettings): Promise<void> {
    setSaving(true);
    setStatus("Saving local settings...");
    try {
      const saved = await updateAppSettings(nextSettings);
      setSettings(saved);
      setStatus("Settings updated.");
    } catch (error) {
      setStatus(`Settings save failed: ${formatError(error)}`);
    } finally {
      setSaving(false);
    }
  }

  async function handleExport(): Promise<void> {
    setBusyAction("export");
    setStatus("Exporting local data snapshot...");
    try {
      const result = await exportLocalData();
      setStatus(
        `Exported ${result.bytesWritten} bytes to ${result.path} (${result.redacted ? "redacted" : "full"}).`,
      );
    } catch (error) {
      setStatus(`Export failed: ${formatError(error)}`);
    } finally {
      setBusyAction(null);
    }
  }

  async function persistRouterRules(nextRules: RouterRule[]): Promise<void> {
    setSavingRouter(true);
    setStatus("Saving handoff router rules...");
    try {
      const saved = await saveRouterRules(nextRules);
      setRouterRules(saved.rules);
      setStatus(`Saved ${saved.rules.length} router rules.`);
    } catch (error) {
      setStatus(`Router save failed: ${formatError(error)}`);
    } finally {
      setSavingRouter(false);
    }
  }

  async function handleDelete(): Promise<void> {
    if (
      !window.confirm(
        "Delete the local AgentDeck database and generated export/log files?",
      )
    ) {
      return;
    }
    setBusyAction("delete");
    setStatus("Deleting local data...");
    try {
      const result = await deleteLocalData();
      setStatus(
        `Deleted local data at ${result.deletedAt}. Removed ${result.removedFiles.length} files.`,
      );
    } catch (error) {
      setStatus(`Delete failed: ${formatError(error)}`);
    } finally {
      setBusyAction(null);
    }
  }

  return (
    <section className="workspace settings-workspace">
      <header>
        <div>
          <p className="eyebrow">Phase 9 / Hardening</p>
          <h2>Settings</h2>
          <p>
            Control export redaction and manage local data snapshots. These
            settings stay on this machine.
          </p>
        </div>
        <span className="phase-badge">Privacy first</span>
      </header>

      <p className="settings-status" role="status">
        <span className={loading || saving || busyAction ? "pulse indicator" : "indicator"} />
        {status}
      </p>

      <section className="settings-grid">
        <article className="settings-card">
          <div className="settings-card-heading">
            <div>
              <p className="eyebrow">Privacy</p>
              <h3>Redaction</h3>
            </div>
            <span>{settings.redactSensitiveExports ? "On" : "Off"}</span>
          </div>
          <label className="settings-toggle">
            <input
              checked={settings.redactSensitiveExports}
              disabled={saving || loading}
              onChange={(event) =>
                void saveSettings({
                  ...settings,
                  redactSensitiveExports: event.target.checked,
                })
              }
              type="checkbox"
            />
            <span>Redact sensitive fields in exports and crash logs.</span>
          </label>
        </article>

        <article className="settings-card">
          <div className="settings-card-heading">
            <div>
              <p className="eyebrow">Logging</p>
              <h3>Crash-safe audit log</h3>
            </div>
            <span>{settings.crashSafeLogging ? "On" : "Off"}</span>
          </div>
          <label className="settings-toggle">
            <input
              checked={settings.crashSafeLogging}
              disabled={saving || loading}
              onChange={(event) =>
                void saveSettings({
                  ...settings,
                  crashSafeLogging: event.target.checked,
                })
              }
              type="checkbox"
            />
            <span>Append audit events to a local JSONL log file.</span>
          </label>
        </article>

        <article className="settings-card">
          <div className="settings-card-heading">
            <div>
              <p className="eyebrow">Agents</p>
              <h3>Grok subscription</h3>
            </div>
            <span>{settings.grokSubscriptionActive ? "On" : "Off"}</span>
          </div>
          <label className="settings-toggle">
            <input
              checked={settings.grokSubscriptionActive}
              disabled={saving || loading}
              onChange={(event) =>
                void saveSettings({
                  ...settings,
                  grokSubscriptionActive: event.target.checked,
                })
              }
              type="checkbox"
            />
            <span>Keep Grok available as a source agent from your active subscription.</span>
          </label>
        </article>
      </section>

      <section className="settings-router" aria-label="Handoff router rules">
        <header>
          <div>
            <p className="eyebrow">Routing</p>
            <h3>Handoff router rules</h3>
            <p>
              Priority-ordered suggestions for the Handoffs view. Lower priority
              numbers win first. Rules match source agent and/or keywords in the
              title and task.
            </p>
          </div>
        </header>

        <div className="settings-router-list">
          {routerRules.length ? (
            routerRules.map((rule, index) => (
              <article className="settings-router-rule" key={rule.id}>
                <div className="settings-router-rule-head">
                  <div>
                    <strong>{rule.name}</strong>
                    <small>
                      priority {index} · {rule.id}
                    </small>
                  </div>
                  <label className="settings-toggle">
                    <input
                      checked={rule.enabled}
                      disabled={savingRouter || loading}
                      onChange={(event) =>
                        setRouterRules((current) =>
                          updateRouterRule(current, rule.id, {
                            enabled: event.target.checked,
                          }),
                        )
                      }
                      type="checkbox"
                    />
                    <span>Enabled</span>
                  </label>
                </div>

                <div className="settings-router-fields">
                  <label>
                    Rule name
                    <input
                      disabled={savingRouter || loading}
                      onChange={(event) =>
                        setRouterRules((current) =>
                          updateRouterRule(current, rule.id, {
                            name: event.target.value,
                          }),
                        )
                      }
                      value={rule.name}
                    />
                  </label>
                  <label>
                    Source agent
                    <select
                      disabled={savingRouter || loading}
                      onChange={(event) =>
                        setRouterRules((current) =>
                          updateRouterRule(current, rule.id, {
                            sourceAgentId: event.target.value || null,
                          }),
                        )
                      }
                      value={rule.sourceAgentId ?? ""}
                    >
                      {ROUTER_SOURCE_OPTIONS.map((option) => (
                        <option key={option.id || "any"} value={option.id}>
                          {option.label}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label>
                    Keyword
                    <input
                      disabled={savingRouter || loading}
                      onChange={(event) =>
                        setRouterRules((current) =>
                          updateRouterRule(current, rule.id, {
                            keyword: event.target.value || null,
                          }),
                        )
                      }
                      placeholder="review, research, local"
                      value={rule.keyword ?? ""}
                    />
                  </label>
                  <label>
                    Target provider
                    <select
                      disabled={savingRouter || loading}
                      onChange={(event) =>
                        setRouterRules((current) =>
                          updateRouterRule(current, rule.id, {
                            targetProviderId: event.target.value,
                          }),
                        )
                      }
                      value={rule.targetProviderId}
                    >
                      {ROUTER_PROVIDER_OPTIONS.map((option) => (
                        <option key={option.id} value={option.id}>
                          {option.label}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label>
                    Target model (optional)
                    <input
                      disabled={savingRouter || loading}
                      onChange={(event) =>
                        setRouterRules((current) =>
                          updateRouterRule(current, rule.id, {
                            targetModelId: event.target.value || null,
                          }),
                        )
                      }
                      placeholder="Leave blank for default model"
                      value={rule.targetModelId ?? ""}
                    />
                  </label>
                </div>

                <div className="settings-router-actions">
                  <button
                    disabled={savingRouter || loading || index === 0}
                    onClick={() =>
                      setRouterRules((current) =>
                        moveRouterRule(current, rule.id, "up"),
                      )
                    }
                    type="button"
                  >
                    Move up
                  </button>
                  <button
                    disabled={
                      savingRouter || loading || index === routerRules.length - 1
                    }
                    onClick={() =>
                      setRouterRules((current) =>
                        moveRouterRule(current, rule.id, "down"),
                      )
                    }
                    type="button"
                  >
                    Move down
                  </button>
                  <button
                    className="secondary-button danger-button"
                    disabled={savingRouter || loading}
                    onClick={() =>
                      setRouterRules((current) =>
                        removeRouterRule(current, rule.id),
                      )
                    }
                    type="button"
                  >
                    Remove
                  </button>
                </div>
              </article>
            ))
          ) : (
            <p className="empty-state">
              No router rules yet. Add one to suggest target providers from
              handoff title and task keywords.
            </p>
          )}
        </div>

        <div className="settings-router-actions">
          <button
            disabled={savingRouter || loading || routerRules.length >= 50}
            onClick={() =>
              setRouterRules((current) => [
                ...current,
                createRouterRule(current.length),
              ])
            }
            type="button"
          >
            Add rule
          </button>
          <button
            className="secondary-button"
            disabled={savingRouter || loading}
            onClick={() => void persistRouterRules(routerRules)}
            type="button"
          >
            {savingRouter ? "Saving..." : "Save router rules"}
          </button>
        </div>
      </section>

      <section className="settings-actions" aria-label="Data controls">
        <article className="settings-card danger-card">
          <div className="settings-card-heading">
            <div>
              <p className="eyebrow">Backup</p>
              <h3>Export local data</h3>
            </div>
            <span>JSON snapshot</span>
          </div>
          <p>
            Write the current chats, handoffs, audits, plugin settings, and
            app settings to a local export file.
          </p>
          <button
            disabled={busyAction !== null || loading}
            onClick={() => void handleExport()}
            type="button"
          >
            {busyAction === "export" ? "Exporting..." : "Export data"}
          </button>
        </article>

        <article className="settings-card danger-card">
          <div className="settings-card-heading">
            <div>
              <p className="eyebrow">Danger zone</p>
              <h3>Delete local data</h3>
            </div>
            <span>Irreversible</span>
          </div>
          <p>
            Remove the database, WAL files, export snapshots, and crash logs
            from this machine.
          </p>
          <button
            className="secondary-button danger-button"
            disabled={busyAction !== null || loading}
            onClick={() => void handleDelete()}
            type="button"
          >
            {busyAction === "delete" ? "Deleting..." : "Delete data"}
          </button>
        </article>
      </section>
    </section>
  );
}

function formatError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
