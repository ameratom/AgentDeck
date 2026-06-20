import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  chatgptReviewHealth,
  grokMcpBridgeStatus,
  loadProjectConnectorSettings,
  openSecureTunnelUi,
  scanMcpInventory,
  secureTunnelStatus,
  saveProjectConnectorSettings,
  startSecureTunnel,
  stopSecureTunnel,
  syncGrokMcpBridge,
  toggleMcpServer,
} from "../../lib/invoke";
import type {
  ChatgptReviewHealth,
  GrokMcpBridgeStatus,
  McpInventory,
  McpToggleResult,
  ProjectConnectorSettings,
  SecureTunnelStatus,
} from "../../lib/types";
import { McpConnectorStrip } from "./components/McpConnectorStrip";
import { McpServerDrawer } from "./components/McpServerDrawer";
import { McpServerTable } from "./components/McpServerTable";
import { McpStatusCards } from "./components/McpStatusCards";
import { existingSources, riskCounts } from "./mcpModel";

export function McpView() {
  const [inventory, setInventory] = useState<McpInventory | null>(null);
  const [status, setStatus] = useState("Preparing read-only MCP inventory.");
  const [scanning, setScanning] = useState(true);
  const [togglingId, setTogglingId] = useState<string | null>(null);
  const [lastToggle, setLastToggle] = useState<McpToggleResult | null>(null);
  const [selectedServerId, setSelectedServerId] = useState<string | null>(null);
  const [bridgeStatus, setBridgeStatus] = useState<GrokMcpBridgeStatus | null>(
    null,
  );
  const [syncingBridge, setSyncingBridge] = useState(false);
  const [tunnelStatus, setTunnelStatus] = useState<SecureTunnelStatus | null>(
    null,
  );
  const [tunnelAction, setTunnelAction] = useState<
    "refresh" | "start" | "stop" | "open" | null
  >(null);
  const [reviewHealth, setReviewHealth] = useState<ChatgptReviewHealth | null>(
    null,
  );
  const [reviewRefreshing, setReviewRefreshing] = useState(false);
  const [projectConnectors, setProjectConnectors] =
    useState<ProjectConnectorSettings | null>(null);
  const [savingProjectConnectors, setSavingProjectConnectors] = useState(false);

  async function scan(): Promise<void> {
    setScanning(true);
    setStatus("Scanning known MCP config locations...");
    try {
      const nextInventory = await scanMcpInventory();
      setInventory(nextInventory);
      setStatus(
        `Found ${nextInventory.servers.length} server definitions across ${
          existingSources(nextInventory.sources).length
        } detected config sources.`,
      );
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      setStatus(`MCP inventory failed: ${detail}`);
    } finally {
      setScanning(false);
    }
  }

  async function syncBridge(): Promise<void> {
    setSyncingBridge(true);
    setStatus("Syncing Grok MCP bridge from encrypted xAI credentials...");
    try {
      const nextBridgeStatus = await syncGrokMcpBridge();
      setBridgeStatus(nextBridgeStatus);
      setStatus(nextBridgeStatus.detail);
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      setStatus(`Grok MCP bridge sync failed: ${detail}`);
    } finally {
      setSyncingBridge(false);
    }
  }

  async function toggleServer(serverId: string, enabled: boolean): Promise<void> {
    setTogglingId(serverId);
    setStatus(`${enabled ? "Enabling" : "Disabling"} MCP server...`);
    try {
      const result = await toggleMcpServer(serverId, enabled);
      setLastToggle(result);
      const nextInventory = await scanMcpInventory();
      setInventory(nextInventory);
      setStatus(
        `${result.serverName} ${enabled ? "enabled" : "disabled"}. Backup saved to ${result.backupPath}.`,
      );
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      setStatus(`MCP toggle failed: ${detail}`);
    } finally {
      setTogglingId(null);
    }
  }

  const refreshReviewHealth = useCallback(async (): Promise<void> => {
    setReviewRefreshing(true);
    try {
      const nextHealth = await chatgptReviewHealth();
      setReviewHealth(nextHealth);
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      setStatus(`ChatGPT review health check failed: ${detail}`);
    } finally {
      setReviewRefreshing(false);
    }
  }, []);

  async function updateTunnel(
    action: "refresh" | "start" | "stop" | "open",
  ): Promise<void> {
    setTunnelAction(action);
    setStatus(
      action === "start"
        ? "Starting OpenAI Secure MCP Tunnel..."
        : action === "stop"
          ? "Stopping OpenAI Secure MCP Tunnel..."
          : action === "open"
            ? "Opening tunnel operator UI..."
            : "Refreshing tunnel status...",
    );
    try {
      const nextStatus =
        action === "start"
          ? await startSecureTunnel()
          : action === "stop"
            ? await stopSecureTunnel()
            : action === "open"
              ? await openSecureTunnelUi()
              : await secureTunnelStatus();
      setTunnelStatus(nextStatus);
      setStatus(nextStatus.detail);
      if (action === "start" || action === "refresh") {
        await refreshReviewHealth();
      }
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      const message = `Secure tunnel ${action} failed: ${detail}`;
      setStatus(message);
      setTunnelStatus((current) =>
        current
          ? { ...current, detail: message }
          : current,
      );
    } finally {
      setTunnelAction(null);
    }
  }

  async function saveProjectConnectors(): Promise<void> {
    if (!projectConnectors) {
      return;
    }
    setSavingProjectConnectors(true);
    setStatus(`Exporting connector profile for ${projectConnectors.projectName}...`);
    try {
      const nextSettings = await saveProjectConnectorSettings({
        filesystemEnabled: projectConnectors.filesystemEnabled,
        gitEnabled: projectConnectors.gitEnabled,
        claudeCodeServeEnabled: projectConnectors.claudeCodeServeEnabled,
        grokMcpEnabled: projectConnectors.grokMcpEnabled,
        xaiResearchMcpEnabled: projectConnectors.xaiResearchMcpEnabled,
      });
      const nextInventory = await scanMcpInventory();
      setProjectConnectors(nextSettings);
      setInventory(nextInventory);
      setStatus(
        `Project connector exports updated for ${nextSettings.projectName}. Existing client configs were not modified.`,
      );
    } catch (error) {
      setStatus(`Project connector export failed: ${formatError(error)}`);
    } finally {
      setSavingProjectConnectors(false);
    }
  }

  useEffect(() => {
    let cancelled = false;

    async function load(): Promise<void> {
      try {
        const [nextInventory, nextBridgeStatus, nextTunnelStatus, nextProjectConnectors, nextReviewHealth] = await Promise.all([
          scanMcpInventory(),
          grokMcpBridgeStatus(),
          secureTunnelStatus(),
          loadProjectConnectorSettings().catch(() => null),
          chatgptReviewHealth().catch(() => null),
        ]);
        if (!cancelled) {
          setInventory(nextInventory);
          setBridgeStatus(nextBridgeStatus);
          setTunnelStatus(nextTunnelStatus);
          setProjectConnectors(nextProjectConnectors);
          setReviewHealth(nextReviewHealth);
          setStatus(
            `Found ${nextInventory.servers.length} server definitions across ${
              existingSources(nextInventory.sources).length
            } detected config sources.`,
          );
        }
      } catch (error) {
        if (!cancelled) {
          const detail = error instanceof Error ? error.message : String(error);
          setStatus(`MCP inventory failed: ${detail}`);
        }
      } finally {
        if (!cancelled) {
          setScanning(false);
        }
      }
    }

    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<ChatgptReviewHealth>("chatgpt-review-updated", (event) => {
      setReviewHealth(event.payload);
    }).then((dispose) => {
      unlisten = dispose;
    });
    const interval = window.setInterval(() => {
      void refreshReviewHealth();
    }, 60_000);
    return () => {
      window.clearInterval(interval);
      unlisten?.();
    };
  }, [refreshReviewHealth]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen("project-changed", () => {
      void Promise.all([
        scanMcpInventory(),
        loadProjectConnectorSettings().catch(() => null),
      ]).then(([nextInventory, nextSettings]) => {
        setInventory(nextInventory);
        setProjectConnectors(nextSettings);
      });
    }).then((dispose) => {
      unlisten = dispose;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  const detectedSources = inventory ? existingSources(inventory.sources) : [];
  const counts = inventory
    ? riskCounts(inventory.servers)
    : { low: 0, medium: 0, high: 0 };
  const servers = useMemo(() => inventory?.servers ?? [], [inventory]);
  const selectedServer = useMemo(
    () => servers.find((server) => server.id === selectedServerId) ?? null,
    [servers, selectedServerId],
  );

  return (
    <section className="workspace mcp-workspace mcp-workspace--compact">
      <header className="mcp-compact-header">
        <div>
          <p className="eyebrow">Phase 5 / Control Plane</p>
          <h2>MCP Servers</h2>
          <p className="mcp-compact-subtitle">
            Inspect transports, commands, and risk indicators. JSON configs can
            be toggled with automatic backup.
          </p>
        </div>
        <div className="mcp-compact-header-meta">
          <div className="mcp-compact-stats">
            <span>{servers.length} servers</span>
            <span>·</span>
            <span>{detectedSources.length} sources</span>
          </div>
          <div className="mcp-compact-risks">
            <span className="risk-count low">{counts.low} low</span>
            <span className="risk-count medium">{counts.medium} medium</span>
            <span className="risk-count high">{counts.high} high</span>
          </div>
          <button
            className="refresh-button"
            disabled={scanning}
            onClick={() => void scan()}
            type="button"
          >
            {scanning ? "Scanning..." : "Refresh inventory"}
          </button>
        </div>
      </header>

      <div className="mcp-compact-status" role="status">
        <span className={scanning ? "pulse indicator" : "indicator"} />
        <span>{status}</span>
      </div>

      {lastToggle ? (
        <p className="mcp-toggle-note mcp-toggle-note--compact">
          Last backup: <code>{lastToggle.backupPath}</code>
        </p>
      ) : null}

      <McpStatusCards
        bridgeStatus={bridgeStatus}
        onRefreshReview={() => void refreshReviewHealth()}
        onSyncBridge={() => void syncBridge()}
        onTunnelAction={(action) => void updateTunnel(action)}
        reviewHealth={reviewHealth}
        reviewRefreshing={reviewRefreshing}
        syncingBridge={syncingBridge}
        tunnelAction={tunnelAction}
        tunnelStatus={tunnelStatus}
      />

      {projectConnectors ? (
        <McpConnectorStrip
          onSave={() => void saveProjectConnectors()}
          onToggle={(key, enabled) =>
            setProjectConnectors((current) =>
              current ? { ...current, [key]: enabled } : current,
            )
          }
          saving={savingProjectConnectors}
          settings={projectConnectors}
        />
      ) : null}

      <McpServerTable
        onRowClick={setSelectedServerId}
        onToggle={(serverId, enabled) => void toggleServer(serverId, enabled)}
        servers={servers}
        togglingId={togglingId}
      />

      <McpServerDrawer
        onClose={() => setSelectedServerId(null)}
        onToggle={(serverId, enabled) => void toggleServer(serverId, enabled)}
        server={selectedServer}
        togglingId={togglingId}
      />
    </section>
  );
}

function formatError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}