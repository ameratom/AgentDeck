import { useEffect, useState } from "react";
import {
  deleteLocalData,
  exportLocalData,
  loadAppSettings,
  updateAppSettings,
} from "../../lib/invoke";
import type { AppSettings } from "../../lib/types";

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
  const [status, setStatus] = useState("Loading hardening settings.");

  useEffect(() => {
    let cancelled = false;

    async function load(): Promise<void> {
      try {
        const nextSettings = await loadAppSettings();
        if (!cancelled) {
          setSettings(nextSettings);
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
