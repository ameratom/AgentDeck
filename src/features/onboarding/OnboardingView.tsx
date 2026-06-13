import { useState } from "react";
import {
  checkProviderAdapter,
  completeOnboarding,
  listProviderAdapters,
  loadProjectConnectorSettings,
  registerProject,
  runHandoff,
  saveProjectConnectorSettings,
  saveProviderApiKey,
  scanEnvironment,
} from "../../lib/invoke";
import type {
  EnvironmentScan,
  HandoffRun,
  ProjectConnectorSettings,
  ProviderAdapterStatus,
} from "../../lib/types";
import { filterAgents } from "../agents/agentModel";
import { recentOutput } from "../handoffs/handoffModel";
import {
  validateProjectPath,
} from "../projects/projectModel";
import {
  buildConnectorExportRequest,
  buildOnboardingHandoffRequest,
  buildProjectRegistration,
  connectorExportSummary,
  grokCredentialReady,
  nextOnboardingStep,
  ONBOARDING_STEP_ORDER,
  selectOnboardingSourceAgent,
  selectTestHandoffTarget,
  stepIndex,
  stepLabel,
  suggestConnectorDefaults,
  suggestedProjectPath,
  summarizeInventory,
  type ConnectorExportDefaults,
  type OnboardingStepId,
} from "./onboardingModel";

interface OnboardingViewProps {
  initialScan: EnvironmentScan | null;
  onComplete: () => void;
}

export function OnboardingView({ initialScan, onComplete }: OnboardingViewProps) {
  const [step, setStep] = useState<OnboardingStepId>("scan");
  const [localScan, setScan] = useState<EnvironmentScan | null>(null);
  const [providers, setProviders] = useState<ProviderAdapterStatus[]>([]);
  const [grokKey, setGrokKey] = useState("");
  const [handoffRun, setHandoffRun] = useState<HandoffRun | null>(null);
  const [projectPath, setProjectPath] = useState("");
  const [projectName, setProjectName] = useState("");
  const [projectPathError, setProjectPathError] = useState<string | null>(null);
  const [connectorDefaults, setConnectorDefaults] =
    useState<ConnectorExportDefaults | null>(null);
  const [exportedConnectors, setExportedConnectors] =
    useState<ProjectConnectorSettings | null>(null);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("Welcome to AgentDeck.");

  const scan = localScan ?? initialScan;

  async function refreshProviders(): Promise<ProviderAdapterStatus[]> {
    const nextProviders = await listProviderAdapters();
    setProviders(nextProviders);
    return nextProviders;
  }

  async function runScan(): Promise<void> {
    setBusy(true);
    setStatus("Scanning your local agent environment...");
    try {
      const nextScan = await scanEnvironment();
      setScan(nextScan);
      setProjectPath(suggestedProjectPath(nextScan));
      setConnectorDefaults(suggestConnectorDefaults(nextScan));
      setStatus("Environment scan completed.");
      setStep("inventory");
    } catch (error) {
      setStatus(`Scan failed: ${formatError(error)}`);
    } finally {
      setBusy(false);
    }
  }

  async function registerWorkspace(): Promise<void> {
    const error = validateProjectPath(projectPath);
    setProjectPathError(error);
    if (error) {
      return;
    }

    setBusy(true);
    setStatus("Registering your project workspace...");
    try {
      const request = buildProjectRegistration(projectPath, projectName);
      await registerProject(request);
      const nextScan = await scanEnvironment();
      setScan(nextScan);
      setConnectorDefaults(suggestConnectorDefaults(nextScan));
      setStatus(`Registered ${request.name ?? request.path} as the active project.`);
      setStep("grok-key");
      void refreshProviders();
    } catch (registerError) {
      setStatus(`Project registration failed: ${formatError(registerError)}`);
    } finally {
      setBusy(false);
    }
  }

  async function exportConnectors(): Promise<void> {
    const defaults = connectorDefaults ?? {
      filesystemEnabled: true,
      gitEnabled: false,
      claudeCodeServeEnabled: false,
    };

    setBusy(true);
    setStatus("Exporting project MCP connector profile...");
    try {
      const settings = await saveProjectConnectorSettings(
        buildConnectorExportRequest(defaults),
      );
      setExportedConnectors(settings);
      setStatus(
        `Exported ${connectorExportSummary(settings).join(", ")} for ${settings.projectName}.`,
      );
    } catch (error) {
      setStatus(`Connector export failed: ${formatError(error)}`);
    } finally {
      setBusy(false);
    }
  }

  async function prepareConnectorStep(): Promise<void> {
    setBusy(true);
    setStatus("Loading project connector defaults...");
    try {
      const settings = await loadProjectConnectorSettings();
      setConnectorDefaults({
        filesystemEnabled: settings.filesystemEnabled,
        gitEnabled: settings.gitEnabled,
        claudeCodeServeEnabled: settings.claudeCodeServeEnabled,
      });
      setExportedConnectors(settings);
      setStatus(`Ready to export connectors for ${settings.projectName}.`);
      setStep("connectors");
    } catch {
      const activeScan = scan ?? (await scanEnvironment());
      setScan(activeScan);
      setConnectorDefaults(suggestConnectorDefaults(activeScan));
      setStatus(
        "Register a project workspace first, or skip connector export for now.",
      );
      setStep("connectors");
    } finally {
      setBusy(false);
    }
  }

  async function saveGrokKey(): Promise<void> {
    const apiKey = grokKey.trim();
    if (apiKey === "") {
      setStatus("Enter a Grok API key or skip this step.");
      return;
    }

    setBusy(true);
    setStatus("Saving the Grok API key...");
    try {
      await saveProviderApiKey({ providerId: "xai", apiKey });
      setGrokKey("");
      const nextProviders = await refreshProviders();
      await checkProviderAdapter({ providerId: "xai" }).then((checked) => {
        setProviders((current) =>
          current.map((provider) =>
            provider.id === checked.id ? checked : provider,
          ),
        );
      });
      if (grokCredentialReady(nextProviders)) {
        setStatus("Grok API key saved (encrypted on this device).");
      } else {
        setStatus("Key saved. Provider check will run on the next step.");
      }
    } catch (error) {
      setStatus(`Save failed: ${formatError(error)}`);
    } finally {
      setBusy(false);
    }
  }

  async function runTestHandoff(): Promise<void> {
    setBusy(true);
    setStatus("Running the onboarding handoff smoke test...");
    try {
      const nextProviders =
        providers.length > 0 ? providers : await refreshProviders();
      const activeScan = scan ?? (await scanEnvironment());
      setScan(activeScan);

      const agents = filterAgents(activeScan.entities);
      const sourceAgent = selectOnboardingSourceAgent(agents);
      const targetProvider = selectTestHandoffTarget(nextProviders);

      if (!sourceAgent) {
        setStatus("No source agent available for the test handoff.");
        return;
      }
      if (!targetProvider) {
        setStatus(
          "No healthy provider with models found. Add a Grok key or start LM Studio, then retry.",
        );
        return;
      }

      const request = buildOnboardingHandoffRequest({
        sourceAgent,
        provider: targetProvider,
      });
      const run = await runHandoff(request);
      setHandoffRun(run);
      setStatus(
        run.status === "completed"
          ? "Test handoff completed successfully."
          : `Test handoff finished with status: ${run.status}`,
      );
    } catch (error) {
      setStatus(`Test handoff failed: ${formatError(error)}`);
    } finally {
      setBusy(false);
    }
  }

  async function finishOnboarding(): Promise<void> {
    setBusy(true);
    setStatus("Saving onboarding completion...");
    try {
      await completeOnboarding();
      onComplete();
    } catch (error) {
      setStatus(`Could not complete onboarding: ${formatError(error)}`);
    } finally {
      setBusy(false);
    }
  }

  function advanceStep(): void {
    const next = nextOnboardingStep(step);
    if (next) {
      setStep(next);
    }
  }

  function skipStep(): void {
    if (step === "done") {
      void finishOnboarding();
      return;
    }
    advanceStep();
  }

  const inventory = scan ? summarizeInventory(scan) : null;
  const progress = `${stepIndex(step) + 1} / ${ONBOARDING_STEP_ORDER.length}`;

  return (
    <div className="onboarding-backdrop" role="dialog" aria-modal="true" aria-label="AgentDeck onboarding">
      <section className="onboarding-panel workspace">
        <header className="onboarding-header">
          <div>
            <p className="eyebrow">First run</p>
            <h2>Set up AgentDeck</h2>
            <p>
              Scan your machine, register a project workspace, export MCP connectors,
              connect Grok, and confirm one safe handoff before entering the control
              plane.
            </p>
          </div>
          <span className="phase-badge">{progress}</span>
        </header>

        <ol className="onboarding-steps" aria-label="Onboarding progress">
          {ONBOARDING_STEP_ORDER.map((stepId) => (
            <li
              className={
                stepId === step
                  ? "onboarding-step active"
                  : stepIndex(stepId) < stepIndex(step)
                    ? "onboarding-step complete"
                    : "onboarding-step"
              }
              key={stepId}
            >
              {stepLabel(stepId)}
            </li>
          ))}
        </ol>

        <p className="onboarding-status" role="status">
          <span className={busy ? "pulse indicator" : "indicator"} />
          {status}
        </p>

        {step === "scan" ? (
          <section className="onboarding-card">
            <h3>Run an environment scan</h3>
            <p>
              AgentDeck will inventory local agents, CLI tools, MCP configs, and
              provider endpoints without changing anything on disk.
            </p>
            <div className="onboarding-actions">
              <button disabled={busy} onClick={() => void runScan()} type="button">
                {busy ? "Scanning..." : "Run scan"}
              </button>
              <button
                className="secondary-button"
                disabled={busy}
                onClick={skipStep}
                type="button"
              >
                Skip
              </button>
            </div>
          </section>
        ) : null}

        {step === "inventory" && inventory ? (
          <section className="onboarding-card">
            <h3>What AgentDeck found</h3>
            <div className="onboarding-metrics">
              <div>
                <strong>{inventory.agentCount}</strong>
                <span>Agents</span>
              </div>
              <div>
                <strong>{inventory.availableTools}</strong>
                <span>Tools</span>
              </div>
              <div>
                <strong>{inventory.validMcpConfigs}</strong>
                <span>MCP configs</span>
              </div>
            </div>
            {inventory.highlights.length > 0 ? (
              <ul className="onboarding-list">
                {inventory.highlights.map((item) => (
                  <li key={item}>{item}</li>
                ))}
              </ul>
            ) : null}
            {inventory.gaps.length > 0 ? (
              <ul className="onboarding-list gaps">
                {inventory.gaps.map((item) => (
                  <li key={item}>{item}</li>
                ))}
              </ul>
            ) : null}
            <div className="onboarding-actions">
              <button
                disabled={busy}
                onClick={() => setStep("project")}
                type="button"
              >
                Continue
              </button>
              <button
                className="secondary-button"
                disabled={busy}
                onClick={skipStep}
                type="button"
              >
                Skip
              </button>
            </div>
          </section>
        ) : null}

        {step === "project" ? (
          <section className="onboarding-card">
            <h3>Register a project workspace</h3>
            <p>
              AgentDeck scopes discovery, chat, handoffs, and MCP exports to one
              active project at a time. Register the repo you want to work in.
            </p>
            <label className="onboarding-field">
              <span>Project folder path</span>
              <input
                disabled={busy}
                onChange={(event) => {
                  setProjectPath(event.target.value);
                  setProjectPathError(null);
                }}
                placeholder="/Users/you/projects/my-app"
                type="text"
                value={projectPath}
              />
            </label>
            {projectPathError ? (
              <p className="onboarding-hint">{projectPathError}</p>
            ) : null}
            <label className="onboarding-field">
              <span>Display name (optional)</span>
              <input
                disabled={busy}
                onChange={(event) => setProjectName(event.target.value)}
                placeholder="My App"
                type="text"
                value={projectName}
              />
            </label>
            <div className="onboarding-actions">
              <button disabled={busy} onClick={() => void registerWorkspace()} type="button">
                {busy ? "Registering..." : "Register project"}
              </button>
              <button
                className="secondary-button"
                disabled={busy}
                onClick={() => {
                  setStep("grok-key");
                  void refreshProviders();
                }}
                type="button"
              >
                Skip
              </button>
            </div>
          </section>
        ) : null}

        {step === "grok-key" ? (
          <section className="onboarding-card">
            <h3>Connect Grok (xAI API)</h3>
            <p>
              Grok is the recommended first cloud provider because the xAI API free
              tier offers strong value. Keys are encrypted on this device.
            </p>
            <label className="onboarding-field">
              <span>xAI API key</span>
              <input
                autoComplete="off"
                disabled={busy}
                onChange={(event) => setGrokKey(event.target.value)}
                placeholder="xai-..."
                type="password"
                value={grokKey}
              />
            </label>
            <p className="onboarding-hint">
              {grokCredentialReady(providers)
                ? "A Grok key is already saved on this device or in your environment."
                : "You can skip this step, save a key here, or use Providers → Import existing Keychain keys to migrate a legacy xAI key."}
            </p>
            <div className="onboarding-actions">
              <button disabled={busy} onClick={() => void saveGrokKey()} type="button">
                {busy ? "Saving..." : "Save key"}
              </button>
              <button
                className="secondary-button"
                disabled={busy}
                onClick={() => {
                  advanceStep();
                  void refreshProviders();
                }}
                type="button"
              >
                {grokCredentialReady(providers) ? "Continue" : "Skip"}
              </button>
            </div>
          </section>
        ) : null}

        {step === "test-handoff" ? (
          <section className="onboarding-card">
            <h3>Confirm one test handoff</h3>
            <p>
              AgentDeck will route a short smoke-test task through your best
              available provider (LM Studio first, then Grok).
            </p>
            {handoffRun ? (
              <article className="onboarding-handoff-result">
                <strong>{handoffRun.title}</strong>
                <span>
                  {handoffRun.status} · {handoffRun.targetProviderName} /{" "}
                  {handoffRun.targetModelId}
                </span>
                <p>{recentOutput(handoffRun)}</p>
              </article>
            ) : null}
            <div className="onboarding-actions">
              <button disabled={busy} onClick={() => void runTestHandoff()} type="button">
                {busy ? "Dispatching..." : handoffRun ? "Retry test" : "Run test handoff"}
              </button>
              <button
                className="secondary-button"
                disabled={busy}
                onClick={skipStep}
                type="button"
              >
                Skip
              </button>
              {handoffRun?.status === "completed" ? (
                <button disabled={busy} onClick={() => void prepareConnectorStep()} type="button">
                  Continue
                </button>
              ) : null}
            </div>
          </section>
        ) : null}

        {step === "connectors" && connectorDefaults ? (
          <section className="onboarding-card">
            <h3>Export project MCP connectors</h3>
            <p>
              Generate validated Claude JSON and Codex TOML snippets for AgentDeck,
              optional filesystem/git launchers, and Claude Code MCP serve. AgentDeck
              does not modify third-party configs automatically.
            </p>
            <div className="mcp-project-connector-options">
              <label>
                <input
                  checked={connectorDefaults.filesystemEnabled}
                  disabled={busy}
                  onChange={(event) =>
                    setConnectorDefaults((current) =>
                      current
                        ? { ...current, filesystemEnabled: event.target.checked }
                        : current,
                    )
                  }
                  type="checkbox"
                />
                <span>
                  <strong>Filesystem MCP</strong>
                  <small>Project-scoped read access.</small>
                </span>
              </label>
              <label>
                <input
                  checked={connectorDefaults.gitEnabled}
                  disabled={busy}
                  onChange={(event) =>
                    setConnectorDefaults((current) =>
                      current
                        ? { ...current, gitEnabled: event.target.checked }
                        : current,
                    )
                  }
                  type="checkbox"
                />
                <span>
                  <strong>Git MCP</strong>
                  <small>Requires a Git repository.</small>
                </span>
              </label>
              <label>
                <input
                  checked={connectorDefaults.claudeCodeServeEnabled}
                  disabled={busy}
                  onChange={(event) =>
                    setConnectorDefaults((current) =>
                      current
                        ? {
                            ...current,
                            claudeCodeServeEnabled: event.target.checked,
                          }
                        : current,
                    )
                  }
                  type="checkbox"
                />
                <span>
                  <strong>Claude Code MCP serve</strong>
                  <small>Expose Claude Code to other MCP clients.</small>
                </span>
              </label>
            </div>
            {exportedConnectors ? (
              <ul className="onboarding-list">
                {connectorExportSummary(exportedConnectors).map((item) => (
                  <li key={item}>{item}</li>
                ))}
                <li>Claude: {exportedConnectors.claudeExportPath}</li>
                <li>Codex: {exportedConnectors.codexExportPath}</li>
              </ul>
            ) : null}
            <div className="onboarding-actions">
              <button disabled={busy} onClick={() => void exportConnectors()} type="button">
                {busy ? "Exporting..." : exportedConnectors ? "Re-export" : "Export profile"}
              </button>
              <button
                className="secondary-button"
                disabled={busy}
                onClick={advanceStep}
                type="button"
              >
                {exportedConnectors ? "Continue" : "Skip"}
              </button>
            </div>
          </section>
        ) : null}

        {step === "done" ? (
          <section className="onboarding-card">
            <h3>AgentDeck is ready</h3>
            <p>
              Your local control plane is configured. Use the menu bar tray for quick
              handoffs, or open Chat and Handoffs from the sidebar.
            </p>
            <div className="onboarding-actions">
              <button disabled={busy} onClick={() => void finishOnboarding()} type="button">
                {busy ? "Finishing..." : "Enter AgentDeck"}
              </button>
              <button
                className="secondary-button"
                disabled={busy}
                onClick={() => void finishOnboarding()}
                type="button"
              >
                Skip
              </button>
            </div>
          </section>
        ) : null}
      </section>
    </div>
  );
}

function formatError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
