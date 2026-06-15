import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  checkProviderAdapter,
  listProviderAdapters,
  loadAppSettings,
  loadHandoffRuns,
  runHandoff,
  scanEnvironment,
  suggestHandoffRoute,
} from "../../lib/invoke";
import {
  routerAutoApplyKey,
  shouldAutoApplyRouter,
} from "../settings/routerAutoApplyModel";
import type {
  EnvironmentScan,
  HandoffRouteSuggestion,
  HandoffRun,
  ProviderAdapterStatus,
} from "../../lib/types";
import {
  buildApprovalRecord,
  buildHandoffRequestFromTarget,
  filterChatModels,
  recentOutput,
  selectActiveScan,
  resolvePreferredHandoffModel,
  resolveSuggestedHandoffModel,
  selectDefaultModel,
  selectDefaultTargetProvider,
} from "./handoffModel";
import {
  providerCredentialBlocked,
  providerDispatchBlocked,
  providerTargetLabel,
} from "../providers/providerModel";

interface HandoffViewProps {
  scan: EnvironmentScan | null;
  highlightRunId?: string | null;
  highlightRunIndex?: number | null;
  onOpenProviders: () => void;
  onRefreshScan: () => void;
}

interface ApprovalSnapshot {
  sourceAgentId: string;
  sourceAgentName: string;
  providerId: string;
  providerName: string;
  modelId: string;
}

interface RouteSuggestionResult {
  requestKey: string;
  suggestion: HandoffRouteSuggestion | null;
}

export function HandoffView({
  scan,
  highlightRunId = null,
  highlightRunIndex = null,
  onOpenProviders,
  onRefreshScan,
}: HandoffViewProps) {
  const [providers, setProviders] = useState<ProviderAdapterStatus[]>([]);
  const [runs, setRuns] = useState<HandoffRun[]>([]);
  const [selectedSourceId, setSelectedSourceId] = useState("");
  const [selectedProviderId, setSelectedProviderId] = useState("");
  const [selectedModelId, setSelectedModelId] = useState("");
  const [title, setTitle] = useState("Review this handoff");
  const [task, setTask] = useState(
    "Review the current local workspace and summarize the next safe action.",
  );
  const [context, setContext] = useState("");
  const [status, setStatus] = useState("Loading manual handoff targets.");
  const [loading, setLoading] = useState(true);
  const [refreshingModels, setRefreshingModels] = useState(false);
  const [scanningSources, setScanningSources] = useState(false);
  const [localScan, setLocalScan] = useState<EnvironmentScan | null>(scan);
  const [dispatching, setDispatching] = useState(false);
  const [approvalOpen, setApprovalOpen] = useState(false);
  const [approvalError, setApprovalError] = useState<string | null>(null);
  const [approvalSnapshot, setApprovalSnapshot] =
    useState<ApprovalSnapshot | null>(null);
  const [routeSuggestionResult, setRouteSuggestionResult] =
    useState<RouteSuggestionResult | null>(null);
  const [routerAutoApply, setRouterAutoApply] = useState(true);
  const [displayAutoAppliedKey, setDisplayAutoAppliedKey] = useState<
    string | null
  >(null);
  const lastAutoAppliedRef = useRef<string | null>(null);

  const activeScan = selectActiveScan(localScan, scan);
  const activeProject = activeScan?.project ?? null;
  const agents =
    activeScan?.entities.filter((entity) => entity.entityType === "agent") ?? [];
  const effectiveSourceId = selectedSourceId || agents[0]?.id || "";
  const selectedSource =
    agents.find((agent) => agent.id === effectiveSourceId) ?? null;
  const selectedProvider =
    providers.find((provider) => provider.id === selectedProviderId) ?? null;
  const modelOptions = useMemo(
    () => filterChatModels(selectedProvider?.models ?? []),
    [selectedProvider],
  );
  const selectedProviderBlocked = providerDispatchBlocked(selectedProvider);
  const routeSuggestionRequestKey =
    effectiveSourceId && title.trim() && task.trim()
      ? JSON.stringify([effectiveSourceId, title, task])
      : "";
  const routeSuggestion =
    routeSuggestionResult?.requestKey === routeSuggestionRequestKey
      ? routeSuggestionResult.suggestion
      : null;

  const canDispatch =
    selectedSource !== null &&
    selectedProviderId !== "" &&
    selectedModelId !== "" &&
    title.trim() !== "" &&
    task.trim() !== "" &&
    !selectedProviderBlocked;
  const scopedRuns = runs.filter(
    (run) => run.projectId === (activeProject?.id ?? null),
  );
  const highlightedRunIndex = useMemo(() => {
    if (highlightRunId) {
      const index = scopedRuns.findIndex((run) => run.id === highlightRunId);
      if (index >= 0) {
        return index;
      }
    }
    return highlightRunIndex;
  }, [highlightRunId, highlightRunIndex, scopedRuns]);

  const refreshProviderModels = useCallback(async (
    providerId: string,
    knownProviders: ProviderAdapterStatus[],
    cancelled = false,
  ): Promise<ProviderAdapterStatus | null> => {
    const provider = knownProviders.find((candidate) => candidate.id === providerId);
    if (provider && providerCredentialBlocked(provider)) {
      setStatus(
        provider.credentialStatus === "unreadable"
          ? `${provider.name} has an unreadable stored key. Re-save it in Providers.`
          : provider.credentialStatus === "import-failed"
            ? `${provider.name} legacy import failed. Approve Keychain access or enter the key in Providers.`
            : `${provider.name} target provider needs an API key before models can load.`,
      );
      return null;
    }

    setRefreshingModels(true);
    setStatus(`Loading models for ${providerId}...`);
    try {
      const nextProvider = await checkProviderAdapter({ providerId });
      if (cancelled) {
        return null;
      }
      setProviders((current) =>
        current.map((provider) =>
          provider.id === nextProvider.id ? nextProvider : provider,
        ),
      );
      setSelectedModelId((current) => {
        if (!nextProvider.verifiedAvailable) {
          return current;
        }
        return nextProvider.models.some((model) => model.id === current)
          ? current
          : selectDefaultModel(nextProvider);
      });
      setStatus(
        nextProvider.verifiedAvailable
          ? `${nextProvider.name} is ready with ${nextProvider.models.length} models.`
          : `${nextProvider.name}: ${nextProvider.health.detail}`,
      );
      return nextProvider;
    } catch (error) {
      if (!cancelled) {
        const detail = error instanceof Error ? error.message : String(error);
        setStatus(`Failed to load models: ${detail}`);
      }
      return null;
    } finally {
      if (!cancelled) {
        setRefreshingModels(false);
      }
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    void loadAppSettings()
      .then((settings) => {
        if (!cancelled) {
          setRouterAutoApply(settings.routerAutoApply);
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    async function load(): Promise<void> {
      try {
        const [nextProviders, nextRuns] = await Promise.all([
          listProviderAdapters(),
          loadHandoffRuns(12),
        ]);
        if (cancelled) {
          return;
        }

        setProviders(nextProviders);
        setRuns(nextRuns);

        const initialProviderId =
          selectDefaultTargetProvider(nextProviders)?.id ?? "";
        const provider =
          nextProviders.find((entry) => entry.id === initialProviderId) ?? null;
        setSelectedProviderId(initialProviderId);
        setSelectedModelId(selectDefaultModel(provider));

        setStatus(
          `Loaded ${nextProviders.length} providers and ${nextRuns.length} recent handoff runs.`,
        );

        if (initialProviderId) {
          void refreshProviderModels(initialProviderId, nextProviders, cancelled);
        }
      } catch (error) {
        if (!cancelled) {
          const detail = error instanceof Error ? error.message : String(error);
          setStatus(`Handoff setup failed: ${detail}`);
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
  }, [refreshProviderModels]);

  useEffect(() => {
    if (!routeSuggestionRequestKey) {
      return;
    }

    let cancelled = false;
    const requestKey = routeSuggestionRequestKey;
    lastAutoAppliedRef.current = null;
    const timer = window.setTimeout(() => {
      void suggestHandoffRoute({
        sourceAgentId: effectiveSourceId,
        title,
        task,
      })
        .then((suggestion) => {
          if (!cancelled) {
            setRouteSuggestionResult({ requestKey, suggestion });
          }
        })
        .catch(() => {
          if (!cancelled) {
            setRouteSuggestionResult({ requestKey, suggestion: null });
          }
        });
    }, 250);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [effectiveSourceId, routeSuggestionRequestKey, task, title]);

  const applyRouteSuggestion = useCallback(
    async (mode: "manual" | "auto" = "manual"): Promise<void> => {
      if (!routeSuggestion) {
        return;
      }
      const targetProvider = providers.find(
        (provider) => provider.id === routeSuggestion.targetProviderId,
      );
      if (!targetProvider) {
        setStatus(
          `Router target ${routeSuggestion.targetProviderId} is no longer available. Update the rule in Settings.`,
        );
        return;
      }
      setSelectedProviderId(routeSuggestion.targetProviderId);
      const refreshedProvider = await refreshProviderModels(
        routeSuggestion.targetProviderId,
        providers,
      );
      if (refreshedProvider?.verifiedAvailable) {
        setSelectedModelId(
          resolveSuggestedHandoffModel(
            refreshedProvider,
            routeSuggestion.targetModelId,
          ),
        );
      }
      const prefix = mode === "auto" ? "Auto-applied" : "Applied";
      setStatus(
        `${prefix} router rule "${routeSuggestion.ruleName}" (${routeSuggestion.reason})`,
      );
    },
    [providers, refreshProviderModels, routeSuggestion],
  );

  useEffect(() => {
    if (
      !shouldAutoApplyRouter(
        routerAutoApply,
        routeSuggestion,
        routeSuggestionRequestKey,
        lastAutoAppliedRef.current,
      )
    ) {
      return;
    }
    const nextKey = routerAutoApplyKey(routeSuggestionRequestKey, routeSuggestion);
    lastAutoAppliedRef.current = nextKey;
    void applyRouteSuggestion("auto").then(() => {
      setDisplayAutoAppliedKey(nextKey);
    });
  }, [
    applyRouteSuggestion,
    routeSuggestion,
    routeSuggestionRequestKey,
    routerAutoApply,
  ]);

  async function refreshSourceAgents(): Promise<void> {
    setScanningSources(true);
    setStatus("Scanning local environment for source agents...");
    try {
      const nextScan = await scanEnvironment();
      const nextAgents = nextScan.entities.filter(
        (entity) => entity.entityType === "agent",
      );
      setLocalScan(nextScan);
      setSelectedSourceId((current) =>
        current && nextAgents.some((agent) => agent.id === current)
          ? current
          : nextAgents[0]?.id ?? "",
      );
      setStatus(`Loaded ${nextAgents.length} source agents from environment scan.`);
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      setStatus(`Source agent scan failed: ${detail}`);
    } finally {
      setScanningSources(false);
    }
  }

  async function approveHandoff(): Promise<void> {
    const snapshot = approvalSnapshot;

    if (
      !snapshot ||
      snapshot.sourceAgentId.trim() === "" ||
      snapshot.providerId.trim() === "" ||
      snapshot.modelId.trim() === "" ||
      title.trim() === "" ||
      task.trim() === ""
    ) {
      setApprovalError("Select a source, target provider, and model first.");
      return;
    }

    if (providerDispatchBlocked(dispatchTargetProvider)) {
      setApprovalError(
        `${snapshot.providerName} must have a verified credential and live model before this handoff can dispatch.`,
      );
      return;
    }

    setDispatching(true);
    setApprovalError(null);
    setStatus("Dispatching approved handoff...");
    try {
      const request = buildHandoffRequestFromTarget({
        projectId: activeProject?.id ?? null,
        sourceAgentId: snapshot.sourceAgentId,
        sourceAgentName: snapshot.sourceAgentName,
        targetProviderId: snapshot.providerId,
        targetProviderName: snapshot.providerName,
        targetModelId: snapshot.modelId,
        title,
        task,
        context,
        approvals: [buildApprovalRecord()],
      });
      const nextRun = await runHandoff(request);
      setRuns((current) => [nextRun, ...current]);
      setStatus(
        nextRun.status === "completed"
          ? `Handoff completed: ${nextRun.title}`
          : `Handoff failed: ${nextRun.error ?? "unknown error"}`,
      );
      setApprovalOpen(false);
      setApprovalSnapshot(null);
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      setApprovalError(detail);
      setStatus(`Dispatch failed: ${detail}`);
    } finally {
      setDispatching(false);
    }
  }

  const dispatchTargetProvider =
    approvalSnapshot !== null
      ? providers.find((provider) => provider.id === approvalSnapshot.providerId) ??
        null
      : selectedProvider;
  const isLocalTarget = dispatchTargetProvider
    ? dispatchTargetProvider.baseUrl.includes("localhost") ||
      dispatchTargetProvider.baseUrl.includes("127.0.0.1")
    : false;
  const previewRisk = isLocalTarget ? "low" : "medium";
  const previewBlocked = providerCredentialBlocked(dispatchTargetProvider);
  const previewDispatchBlocked = providerDispatchBlocked(dispatchTargetProvider);
  const providerCredentialNote = selectedProvider
    ? `${selectedProvider.name} target provider needs an API key before models can load or a handoff can dispatch. Grok can still be available as a source agent from your subscription setting.`
    : "Select a target provider before loading models.";

  return (
    <section className="workspace handoff-workspace">
      <header>
        <div>
          <p className="eyebrow">Phase 6 / Handoffs</p>
          <h2>Manual Handoffs</h2>
          <p>
            Build a preview, approve it explicitly, and dispatch the task to a
            chosen provider model. The run record and result stay local.
          </p>
          <p className="workspace-context">
            {activeProject
              ? `Scoped to ${activeProject.name} at ${activeProject.path}`
              : "No active project. Handoffs will use global context."}
          </p>
        </div>
        <button
          className="refresh-button"
          disabled={loading}
          onClick={onRefreshScan}
          type="button"
        >
          Refresh environment
        </button>
      </header>

      <div className="handoff-status" role="status">
        <span className={dispatching || loading ? "pulse indicator" : "indicator"} />
        <span>{status}</span>
        <span className="handoff-source-count">
          {agents.length} source agents
        </span>
        <span className="handoff-run-count">{scopedRuns.length} runs</span>
      </div>

      <section className="handoff-layout">
        <article className="handoff-form">
          <div className="handoff-section-heading">
            <div>
              <p className="eyebrow">Draft</p>
              <h3>Handoff details</h3>
            </div>
            {scan ? <span>{scan.scannedAt}</span> : null}
          </div>

          <div className="handoff-grid">
            <label>
              <span className="source-agent-label">
                Source agent
                {agents.length === 0 ? (
                  <button
                    disabled={scanningSources}
                    onClick={() => {
                      void refreshSourceAgents();
                    }}
                    type="button"
                  >
                    {scanningSources ? "Scanning..." : "Scan source agents"}
                  </button>
                ) : null}
              </span>
              <select
                disabled={agents.length === 0 || scanningSources}
                onChange={(event) => setSelectedSourceId(event.target.value)}
                value={effectiveSourceId}
              >
                {agents.length > 0 ? (
                  agents.map((agent) => (
                    <option key={agent.id} value={agent.id}>
                      {agent.name} - {agent.status}
                    </option>
                  ))
                ) : (
                  <option value="">Run environment scan first</option>
                )}
              </select>
            </label>

            <label>
              <span>Target provider</span>
              <select
                disabled={providers.length === 0}
                onChange={(event) => {
                  const nextProviderId = event.target.value;
                  setSelectedProviderId(nextProviderId);
                  const nextProvider = providers.find(
                    (provider) => provider.id === nextProviderId,
                  );
                  setSelectedModelId((current) =>
                    resolvePreferredHandoffModel(nextProvider ?? null, current),
                  );
                  void refreshProviderModels(nextProviderId, providers);
                }}
                value={selectedProviderId}
              >
                {providers.length > 0 ? (
                  providers.map((provider) => (
                    <option key={provider.id} value={provider.id}>
                      {providerTargetLabel(provider)}
                    </option>
                  ))
                ) : (
                  <option value="">Loading providers...</option>
                )}
              </select>
            </label>

            <label>
              <span>Target model</span>
              <select
                disabled={!selectedProvider || modelOptions.length === 0}
                onChange={(event) => setSelectedModelId(event.target.value)}
                value={selectedModelId}
              >
                {modelOptions.length ? (
                  modelOptions.map((model) => (
                    <option key={model.id} value={model.id}>
                      {model.id}
                    </option>
                  ))
                ) : (
                  <option value="">Load models for the selected provider</option>
                )}
              </select>
            </label>

            <div className="handoff-actions">
              <button
                disabled={!selectedProvider || refreshingModels || previewBlocked}
                onClick={() => {
                  if (selectedProvider) {
                    void refreshProviderModels(selectedProvider.id, providers);
                  }
                }}
                type="button"
              >
                {refreshingModels ? "Loading models..." : "Load target models"}
              </button>
              <button
                disabled={!canDispatch}
                onClick={() => {
                  if (!selectedSource || !selectedProvider) {
                    return;
                  }
                  setApprovalError(null);
                  setApprovalSnapshot({
                    sourceAgentId: selectedSource.id,
                    sourceAgentName: selectedSource.name,
                    providerId: selectedProvider.id,
                    providerName: selectedProvider.name,
                    modelId: selectedModelId,
                  });
                  setApprovalOpen(true);
                }}
                type="button"
              >
                Review handoff
              </button>
            </div>

            {routeSuggestion ? (
              <div className="handoff-router-suggestion handoff-wide">
                <div>
                  <strong>
                    Router suggestion: {routeSuggestion.ruleName}
                    {displayAutoAppliedKey ===
                    routerAutoApplyKey(
                      routeSuggestionRequestKey,
                      routeSuggestion,
                    ) ? (
                      <span className="router-auto-badge">Auto-applied</span>
                    ) : null}
                  </strong>
                  <p>
                    Route to {routeSuggestion.targetProviderId}
                    {routeSuggestion.targetModelId
                      ? ` / ${routeSuggestion.targetModelId}`
                      : ""}
                    . {routeSuggestion.reason}
                  </p>
                </div>
                <button
                  disabled={refreshingModels}
                  onClick={() => void applyRouteSuggestion("manual")}
                  type="button"
                >
                  Apply suggestion
                </button>
              </div>
            ) : null}

            <label className="handoff-wide">
              <span>Title</span>
              <input
                onChange={(event) => setTitle(event.target.value)}
                placeholder="Review, summarize, or continue work"
                value={title}
              />
            </label>

            <label className="handoff-wide">
              <span>Task</span>
              <textarea
                onChange={(event) => setTask(event.target.value)}
                placeholder="What should the target do?"
                rows={4}
                value={task}
              />
            </label>

            <label className="handoff-wide">
              <span>Context</span>
              <textarea
                onChange={(event) => setContext(event.target.value)}
                placeholder="Relevant background, constraints, or links"
                rows={5}
                value={context}
              />
            </label>
          </div>

          <p className="handoff-note">
            {previewBlocked
              ? providerCredentialNote
              : previewDispatchBlocked
                ? "Check this provider successfully before reviewing or dispatching a handoff."
              : "Approval is required before any provider call is made."}
          </p>
          {previewBlocked ? (
            <button
              className="inline-link-button"
              onClick={onOpenProviders}
              type="button"
            >
              Open Providers to save API key
            </button>
          ) : null}
          {approvalError ? <p className="handoff-error">{approvalError}</p> : null}
        </article>

        <aside className="handoff-history">
          <div className="handoff-section-heading">
            <div>
              <p className="eyebrow">Runs</p>
              <h3>Recent handoffs</h3>
            </div>
            <span>{scopedRuns.length} stored</span>
          </div>

          <div className="handoff-run-list">
            {scopedRuns.length > 0 ? (
              scopedRuns.map((run, index) => (
                <article
                  className={
                    highlightedRunIndex === index
                      ? "handoff-run-card highlighted"
                      : "handoff-run-card"
                  }
                  key={run.id}
                  ref={(element) => {
                    if (highlightedRunIndex === index && element) {
                      element.scrollIntoView({ block: "nearest", behavior: "smooth" });
                    }
                  }}
                >
                  <div className="handoff-run-top">
                    <div>
                      <strong>{run.title}</strong>
                      <span>{run.status}</span>
                    </div>
                    <small>{run.updatedAt}</small>
                  </div>
                  <p>
                    {run.sourceAgentName} to {run.targetProviderName} /{" "}
                    {run.targetModelId}
                  </p>
                  <pre>{recentOutput(run)}</pre>
                </article>
              ))
            ) : (
              <div className="handoff-empty">
                <h3>No runs yet</h3>
                <p>Approve a handoff to create the first run record.</p>
              </div>
            )}
          </div>
        </aside>
      </section>

      {approvalOpen && approvalSnapshot ? (
        <div className="handoff-modal-backdrop" role="presentation">
          <section
            aria-modal="true"
            className="handoff-modal"
            role="dialog"
            aria-labelledby="handoff-preview-title"
          >
            <div className="handoff-section-heading">
              <div>
                <p className="eyebrow">Approval required</p>
                <h3 id="handoff-preview-title">Handoff preview</h3>
              </div>
              <span className={`risk-pill ${previewRisk}`}>
                {previewRisk} risk
              </span>
            </div>

            <dl className="handoff-preview-grid">
              <div>
                <dt>Project</dt>
                <dd>{activeProject?.name ?? "Global"}</dd>
              </div>
              <div>
                <dt>Source</dt>
                <dd>{approvalSnapshot.sourceAgentName}</dd>
              </div>
              <div>
                <dt>Target</dt>
                <dd>{approvalSnapshot.providerName}</dd>
              </div>
              <div>
                <dt>Model</dt>
                <dd>{approvalSnapshot.modelId}</dd>
              </div>
              <div>
                <dt>Title</dt>
                <dd>{title}</dd>
              </div>
            </dl>

            <div className="handoff-preview-block">
              <dt>Task</dt>
              <dd>{task}</dd>
            </div>

            <div className="handoff-preview-block">
              <dt>Context</dt>
              <dd>{context || "No additional context provided."}</dd>
            </div>

            {approvalError ? (
              <p className="handoff-error">{approvalError}</p>
            ) : previewBlocked ? (
              <p className="handoff-error">
                {approvalSnapshot.providerName} needs an API key before this
                handoff can dispatch.{" "}
                <button
                  className="inline-link-button"
                  onClick={onOpenProviders}
                  type="button"
                >
                  Open Providers
                </button>
              </p>
            ) : null}

            <div className="handoff-modal-actions">
              <button
                className="secondary-button"
                onClick={() => {
                  setApprovalOpen(false);
                  setApprovalSnapshot(null);
                  setApprovalError(null);
                }}
                type="button"
              >
                Cancel
              </button>
              <button
                disabled={dispatching || previewDispatchBlocked}
                onClick={() => void approveHandoff()}
                type="button"
              >
                {dispatching ? "Dispatching..." : "Approve and send"}
              </button>
            </div>
          </section>
        </div>
      ) : null}
    </section>
  );
}
