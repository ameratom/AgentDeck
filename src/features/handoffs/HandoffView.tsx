import { useEffect, useMemo, useState } from "react";
import {
  checkProviderAdapter,
  listProviderAdapters,
  loadHandoffRuns,
  loadRouterRules,
  runHandoff,
  scanEnvironment,
} from "../../lib/invoke";
import type {
  EnvironmentScan,
  HandoffRun,
  ProviderAdapterStatus,
} from "../../lib/types";
import {
  buildApprovalRecord,
  buildHandoffRequest,
  recentOutput,
  selectDefaultModel,
  selectDefaultTargetProvider,
} from "./handoffModel";
import { evaluateRouter } from "./routerModel";
import type { RouterRule } from "../../lib/types";
import { providerTargetLabel } from "../providers/providerModel";

interface HandoffViewProps {
  scan: EnvironmentScan | null;
  highlightRunIndex?: number | null;
  onOpenProviders: () => void;
  onRefreshScan: () => void;
}

export function HandoffView({
  scan,
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
  const [routerRules, setRouterRules] = useState<RouterRule[]>([]);

  const activeScan = scan ?? localScan;
  const agents =
    activeScan?.entities.filter((entity) => entity.entityType === "agent") ?? [];
  const effectiveSourceId = selectedSourceId || agents[0]?.id || "";
  const selectedSource =
    agents.find((agent) => agent.id === effectiveSourceId) ?? null;
  const selectedProvider =
    providers.find((provider) => provider.id === selectedProviderId) ?? null;
  const selectedModel =
    selectedProvider?.models.find((model) => model.id === selectedModelId) ?? null;
  const routerSuggestion = useMemo(
    () =>
      evaluateRouter({
        task,
        context,
        sourceAgentId: effectiveSourceId,
        providers,
        rules: routerRules,
      }),
    [task, context, effectiveSourceId, providers, routerRules],
  );

  const canDispatch =
    selectedSource !== null &&
    selectedProvider !== null &&
    selectedModel !== null &&
    title.trim() !== "" &&
    task.trim() !== "";

  useEffect(() => {
    let cancelled = false;

    async function load(): Promise<void> {
      try {
        const [nextProviders, nextRuns, nextRules] = await Promise.all([
          listProviderAdapters(),
          loadHandoffRuns(12),
          loadRouterRules(),
        ]);
        if (cancelled) {
          return;
        }

        setProviders(nextProviders);
        setRuns(nextRuns);
        setRouterRules(nextRules);

        const defaultProvider = selectDefaultTargetProvider(nextProviders);
        if (defaultProvider) {
          setSelectedProviderId(defaultProvider.id);
          setSelectedModelId(selectDefaultModel(defaultProvider));
        }
        setStatus(
          `Loaded ${nextProviders.length} providers and ${nextRuns.length} recent handoff runs.`,
        );

        if (defaultProvider?.id === "lm-studio") {
          void refreshProviderModels(defaultProvider.id, cancelled);
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
  }, [scan]);

  async function refreshProviderModels(
    providerId: string,
    cancelled = false,
  ): Promise<void> {
    const provider = providers.find((candidate) => candidate.id === providerId);
    if (
      provider &&
      provider.authMode !== "none" &&
      provider.credentialStatus === "missing"
    ) {
      setStatus(
        `${provider.name} target provider needs an API key before models can load.`,
      );
      return;
    }

    setRefreshingModels(true);
    setStatus(`Loading models for ${providerId}...`);
    try {
      const nextProvider = await checkProviderAdapter({ providerId });
      if (cancelled) {
        return;
      }
      setProviders((current) =>
        current.map((provider) =>
          provider.id === nextProvider.id ? nextProvider : provider,
        ),
      );
      setSelectedModelId((current) =>
        current && nextProvider.models.some((model) => model.id === current)
          ? current
          : nextProvider.models[0]?.id ?? "",
      );
      setStatus(
        nextProvider.health.available
          ? `${nextProvider.name} is ready with ${nextProvider.models.length} models.`
          : `${nextProvider.name}: ${nextProvider.health.detail}`,
      );
    } catch (error) {
      if (!cancelled) {
        const detail = error instanceof Error ? error.message : String(error);
        setStatus(`Failed to load models: ${detail}`);
      }
    } finally {
      if (!cancelled) {
        setRefreshingModels(false);
      }
    }
  }

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
    if (!canDispatch || !selectedSource || !selectedProvider || !selectedModel) {
      setApprovalError("Select a source, target provider, and model first.");
      return;
    }

    setDispatching(true);
    setApprovalError(null);
    setStatus("Dispatching approved handoff...");
    try {
      const request = buildHandoffRequest({
        sourceAgentId: selectedSource.id,
        sourceAgentName: selectedSource.name,
        provider: selectedProvider,
        modelId: selectedModel.id,
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
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      setApprovalError(detail);
      setStatus(`Dispatch failed: ${detail}`);
    } finally {
      setDispatching(false);
    }
  }

  const isLocalTarget = selectedProvider
    ? selectedProvider.baseUrl.includes("localhost") ||
      selectedProvider.baseUrl.includes("127.0.0.1")
    : false;
  const previewRisk = isLocalTarget ? "low" : "medium";
  const previewBlocked =
    selectedProvider !== null &&
    selectedProvider.authMode !== "none" &&
    selectedProvider.credentialStatus === "missing";
  const providerCredentialNote = selectedProvider
    ? `${selectedProvider.name} target provider needs an API key before models can load or a handoff can dispatch. Grok can still be available as a source agent from your subscription setting.`
    : "Select a target provider before loading models.";

  return (
    <section className="workspace handoff-workspace">
      <header>
        <div>
          <p className="eyebrow">Phase 6 / Handoffs</p>
          <h2>Manual Handoff Router</h2>
          <p>
            Build a preview, approve it explicitly, and dispatch the task to a
            chosen provider model. The run record and result stay local.
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
        <span className="handoff-run-count">{runs.length} runs</span>
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
                  setSelectedModelId(nextProvider?.models[0]?.id ?? "");
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
                disabled={!selectedProvider || selectedProvider.models.length === 0}
                onChange={(event) => setSelectedModelId(event.target.value)}
                value={selectedModelId}
              >
                {selectedProvider?.models.length ? (
                  selectedProvider.models.map((model) => (
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
                    void refreshProviderModels(selectedProvider.id);
                  }
                }}
                type="button"
              >
                {refreshingModels ? "Loading models..." : "Load target models"}
              </button>
              <button
                disabled={!canDispatch}
                onClick={() => {
                  setApprovalError(null);
                  setApprovalOpen(true);
                }}
                type="button"
              >
                Review handoff
              </button>
            </div>

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

          {routerSuggestion.providerId ? (
            <div className="handoff-router-suggestion">
              <div>
                <strong>Router suggestion</strong>
                <p>
                  {routerSuggestion.rule?.id ?? "fallback"} →{" "}
                  {routerSuggestion.providerId}
                  {routerSuggestion.modelId
                    ? ` / ${routerSuggestion.modelId}`
                    : ""}
                </p>
                {routerSuggestion.warning ? (
                  <p className="handoff-error">{routerSuggestion.warning}</p>
                ) : null}
              </div>
              <button
                disabled={!routerSuggestion.providerId}
                onClick={() => {
                  if (routerSuggestion.providerId) {
                    setSelectedProviderId(routerSuggestion.providerId);
                  }
                  if (routerSuggestion.modelId) {
                    setSelectedModelId(routerSuggestion.modelId);
                  }
                }}
                type="button"
              >
                Apply suggestion
              </button>
            </div>
          ) : null}

          <p className="handoff-note">
            {previewBlocked
              ? providerCredentialNote
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
            <span>{runs.length} stored</span>
          </div>

          <div className="handoff-run-list">
            {runs.length > 0 ? (
              runs.map((run, index) => (
                <article
                  className={
                    highlightRunIndex === index
                      ? "handoff-run-card highlighted"
                      : "handoff-run-card"
                  }
                  key={run.id}
                  ref={(element) => {
                    if (highlightRunIndex === index && element) {
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

      {approvalOpen && selectedSource && selectedProvider && selectedModel ? (
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
                <dt>Source</dt>
                <dd>{selectedSource.name}</dd>
              </div>
              <div>
                <dt>Target</dt>
                <dd>{selectedProvider.name}</dd>
              </div>
              <div>
                <dt>Model</dt>
                <dd>{selectedModel.id}</dd>
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

            <div className="handoff-modal-actions">
              <button
                className="secondary-button"
                onClick={() => setApprovalOpen(false)}
                type="button"
              >
                Cancel
              </button>
              <button disabled={dispatching || previewBlocked} onClick={() => void approveHandoff()} type="button">
                {dispatching ? "Dispatching..." : "Approve and send"}
              </button>
            </div>
          </section>
        </div>
      ) : null}
    </section>
  );
}
