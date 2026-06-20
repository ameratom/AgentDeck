import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useMemo, useState } from "react";
import { AgentsView } from "../features/agents/AgentsView";
import { AuditView } from "../features/audit/AuditView";
import { ChatView } from "../features/chat/ChatView";
import { EntityDrawer } from "../features/graph/EntityDrawer";
import { EntitySelector } from "../features/graph/EntitySelector";
import { GraphCanvas } from "../features/graph/GraphCanvas";
import { OrbitalGraph } from "../features/graph/OrbitalGraph";
import { HandoffView } from "../features/handoffs/HandoffView";
import { McpView } from "../features/mcp/McpView";
import { OnboardingView } from "../features/onboarding/OnboardingView";
import { ProvidersView } from "../features/providers/ProvidersView";
import { PluginsView } from "../features/plugins/PluginsView";
import { ProjectsView } from "../features/projects/ProjectsView";
import { SettingsView } from "../features/settings/SettingsView";
import { isEnvironmentScan } from "../lib/discovery";
import { loadAppSettings, scanEnvironment } from "../lib/invoke";
import type {
  DiscoveredEntity,
  EnvironmentScan,
  PreflightResult,
} from "../lib/types";

const navigation = [
  "Chat",
  "Handoffs",
  "Graph",
  "Agents",
  "Activity",
  "Providers",
  "MCP",
  "Plugins",
  "Projects",
  "Settings",
] as const;

type NavigationItem = (typeof navigation)[number];
type View =
  | "Chat"
  | "Handoffs"
  | "Graph"
  | "Agents"
  | "Activity"
  | "Providers"
  | "MCP"
  | "Plugins"
  | "Projects"
  | "Settings";

type Result = PreflightResult | EnvironmentScan;

type NavigateViewPayload =
  | View
  | {
      view: View;
      runIndex?: string | number;
      runId?: string;
    };

export default function App() {
  const [activeView, setActiveView] = useState<View>("Graph");
  const [result, setResult] = useState<Result | null>(null);
  const [selectedEntity, setSelectedEntity] =
    useState<DiscoveredEntity | null>(null);
  const [status, setStatus] = useState("Preparing a read-only environment scan.");
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [showOnboarding, setShowOnboarding] = useState(false);
  const [handoffRunIndex, setHandoffRunIndex] = useState<number | null>(null);
  const [handoffHighlightRunId, setHandoffHighlightRunId] = useState<string | null>(
    null,
  );
  const scan = isEnvironmentScan(result) ? result : null;

  async function execute(
    label: string,
    action: () => Promise<Result>,
  ): Promise<void> {
    setBusyAction(label);
    setStatus(`${label} in progress...`);

    try {
      const nextResult = await withTimeout(action(), 20_000, label);
      setResult(nextResult);
      setStatus(`${label} completed.`);
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      setStatus(`${label} failed: ${detail}`);
    } finally {
      setBusyAction(null);
    }
  }

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void execute("Environment scan", scanEnvironment);
    }, 2_000);

    return () => window.clearTimeout(timer);
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    void listen<EnvironmentScan>("scan-updated", (event) => {
      setResult(event.payload);
      setStatus("Environment scan updated.");
    }).then((dispose) => {
      unlisten = dispose;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    void listen("project-changed", () => {
      setSelectedEntity(null);
      void execute("Project scan", scanEnvironment);
    }).then((dispose) => {
      unlisten = dispose;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    async function loadOnboardingState(): Promise<void> {
      try {
        const settings = await loadAppSettings();
        if (!cancelled && !settings.onboardingComplete) {
          setShowOnboarding(true);
        }
      } catch {
        if (!cancelled) {
          setShowOnboarding(true);
        }
      }
    }

    void loadOnboardingState();

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    void listen<NavigateViewPayload>("navigate-view", (event) => {
      const payload = event.payload;
      const view = typeof payload === "string" ? payload : payload.view;
      if (
        view === "Chat" ||
        view === "Handoffs" ||
        view === "Graph" ||
        view === "Agents" ||
        view === "Activity" ||
        view === "Providers" ||
        view === "MCP" ||
        view === "Plugins" ||
        view === "Projects" ||
        view === "Settings"
      ) {
        setActiveView(view);
        if (view === "Handoffs" && typeof payload !== "string") {
          if (payload.runId) {
            setHandoffHighlightRunId(payload.runId);
            setHandoffRunIndex(null);
          } else if (payload.runIndex !== undefined) {
            const parsed = Number(payload.runIndex);
            setHandoffRunIndex(Number.isNaN(parsed) ? null : parsed);
            setHandoffHighlightRunId(null);
          } else {
            setHandoffRunIndex(null);
            setHandoffHighlightRunId(null);
          }
        } else {
          setHandoffRunIndex(null);
          setHandoffHighlightRunId(null);
        }
        void getCurrentWindow()
          .show()
          .then(() => getCurrentWindow().unminimize())
          .then(() => getCurrentWindow().setFocus())
          .catch(() => undefined);
      }
    }).then((dispose) => {
      unlisten = dispose;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    void listen<{ entityId: string }>("select-entity", (event) => {
      const entity = scan?.entities.find(
        (candidate) => candidate.id === event.payload.entityId,
      );
      if (entity) {
        setSelectedEntity(entity);
        setActiveView("Graph");
      }
    }).then((dispose) => {
      unlisten = dispose;
    });

    return () => {
      unlisten?.();
    };
  }, [scan]);

  function navigate(item: NavigationItem): void {
    if (
      item === "Chat" ||
      item === "Handoffs" ||
      item === "Graph" ||
      item === "Agents" ||
      item === "Activity" ||
      item === "Providers" ||
      item === "MCP" ||
      item === "Plugins" ||
      item === "Projects" ||
      item === "Settings"
    ) {
      setActiveView(item);
      if (item !== "Handoffs") {
        setHandoffRunIndex(null);
        setHandoffHighlightRunId(null);
      }
    }
  }

  function openHandoffRun(runId: string): void {
    setHandoffHighlightRunId(runId);
    setHandoffRunIndex(null);
    setActiveView("Handoffs");
  }

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div>
          <p className="eyebrow">Local control plane</p>
          <h1>AgentDeck</h1>
        </div>

        <nav aria-label="Primary navigation">
          {navigation.map((item) => {
            const available =
              item === "Chat" ||
              item === "Handoffs" ||
              item === "Graph" ||
              item === "Agents" ||
              item === "Activity" ||
              item === "Providers" ||
              item === "MCP" ||
              item === "Plugins" ||
              item === "Projects" ||
              item === "Settings";

            return (
              <button
                className={item === activeView ? "nav-item active" : "nav-item"}
                disabled={!available}
                key={item}
                onClick={() => navigate(item)}
                type="button"
              >
                <span>{item}</span>
                <small>{available ? "Available" : "Planned"}</small>
              </button>
            );
          })}
        </nav>

        <div className="safety-note">
          <span className="status-dot" />
          <div>
            <strong>Read-only mode</strong>
            <p>No external configurations are changed.</p>
          </div>
        </div>
      </aside>

      {activeView === "Chat" ? (
        <ChatView
          project={scan?.project ?? null}
          onOpenProviders={() => navigate("Providers")}
        />
      ) : activeView === "Handoffs" ? (
        <HandoffView
          highlightRunId={handoffHighlightRunId}
          highlightRunIndex={handoffRunIndex}
          scan={scan}
          onOpenProviders={() => navigate("Providers")}
          onRefreshScan={() => void execute("Environment scan", scanEnvironment)}
        />
      ) : activeView === "MCP" ? (
        <McpView />
      ) : activeView === "Providers" ? (
        <ProvidersView />
      ) : activeView === "Plugins" ? (
        <PluginsView />
      ) : activeView === "Projects" ? (
        <ProjectsView />
      ) : activeView === "Settings" ? (
        <SettingsView />
      ) : activeView === "Graph" ? (
        <GraphView
          busyAction={busyAction}
          onRefresh={() => void execute("Environment scan", scanEnvironment)}
          onSelect={setSelectedEntity}
          onCloseDetails={() => setSelectedEntity(null)}
          scan={scan}
          selectedEntity={selectedEntity}
          status={status}
        />
      ) : activeView === "Agents" ? (
        <AgentsView
          busy={busyAction !== null}
          onRefresh={() => void execute("Environment scan", scanEnvironment)}
          scan={scan}
        />
      ) : activeView === "Activity" ? (
        <AuditView onOpenHandoffRun={openHandoffRun} />
      ) : null}

      {showOnboarding ? (
        <OnboardingView
          initialScan={scan}
          onComplete={() => setShowOnboarding(false)}
        />
      ) : null}
    </main>
  );
}

function withTimeout<T>(
  promise: Promise<T>,
  timeoutMs: number,
  label: string,
): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = window.setTimeout(() => {
      reject(new Error(`${label} timed out`));
    }, timeoutMs);

    promise.then(
      (value) => {
        window.clearTimeout(timer);
        resolve(value);
      },
      (error: unknown) => {
        window.clearTimeout(timer);
        reject(error);
      },
    );
  });
}

interface GraphViewProps {
  busyAction: string | null;
  onRefresh: () => void;
  onSelect: (entity: DiscoveredEntity | null) => void;
  onCloseDetails: () => void;
  scan: EnvironmentScan | null;
  selectedEntity: DiscoveredEntity | null;
  status: string;
}

function GraphView({
  busyAction,
  onRefresh,
  onSelect,
  onCloseDetails,
  scan,
  selectedEntity,
  status,
}: GraphViewProps) {
  const [useOrbital, setUseOrbital] = useState(true);
  const entities = useMemo(() => scan?.entities ?? [], [scan]);

  const clearSelection = () => {
    onSelect(null);
  };

  return (
    <section className="workspace graph-workspace">
      <header>
        <div>
          <p className="eyebrow">Graph View</p>
          <h2>Connection Map</h2>
          <p className="workspace-context">
            {scan?.project
              ? `Project configs scoped to ${scan.project.name} at ${scan.project.path}; runtime health remains machine-wide.`
              : "No active project. Showing machine-wide runtime health and user-level configs."}
          </p>
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
          <EntitySelector
            entities={entities}
            onSelect={onSelect}
            placeholder="Select entity to center..."
            selectedId={selectedEntity?.id ?? null}
          />

          {selectedEntity && (
            <button
              className="clear-button"
              onClick={clearSelection}
              type="button"
            >
              Clear
            </button>
          )}

          <button
            className={`mode-toggle ${useOrbital ? "active" : ""}`}
            onClick={() => setUseOrbital(true)}
            type="button"
          >
            Orbital
          </button>
          <button
            className={`mode-toggle ${!useOrbital ? "active" : ""}`}
            onClick={() => setUseOrbital(false)}
            type="button"
          >
            Flat
          </button>

          <button
            className="refresh-button"
            disabled={busyAction !== null}
            onClick={onRefresh}
            type="button"
          >
            {busyAction ? "Scanning..." : "Refresh"}
          </button>
        </div>
      </header>

      <div className="graph-status">
        <span className={busyAction ? "pulse indicator" : "indicator"} />
        <span>{status}</span>
        {scan && <span>{entities.length} entities • {scan.scannedAt}</span>}
      </div>

      <section className="graph-layout">
        {scan ? (
          useOrbital ? (
            <OrbitalGraph
              entities={entities}
              onSelect={onSelect}
              selectedId={selectedEntity?.id ?? null}
            />
          ) : (
            <GraphCanvas
              entities={entities}
              onSelect={onSelect}
              showProcesses={false}
            />
          )
        ) : (
          <div className="graph-loading">
            <span className="pulse indicator" />
            <p>Building the local connection map...</p>
          </div>
        )}
        <EntityDrawer entity={selectedEntity} onClose={onCloseDetails} />
      </section>
    </section>
  );
}
