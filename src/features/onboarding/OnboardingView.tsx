import { useEffect, useState } from "react";
import {
  checkProviderAdapter,
  completeOnboarding,
  listProviderAdapters,
  runHandoff,
  saveProviderApiKey,
  scanEnvironment,
} from "../../lib/invoke";
import type { EnvironmentScan, HandoffRun, ProviderAdapterStatus } from "../../lib/types";
import { filterAgents } from "../agents/agentModel";
import { recentOutput } from "../handoffs/handoffModel";
import {
  buildOnboardingHandoffRequest,
  grokCredentialReady,
  nextOnboardingStep,
  ONBOARDING_STEP_ORDER,
  selectOnboardingSourceAgent,
  selectTestHandoffTarget,
  stepIndex,
  stepLabel,
  summarizeInventory,
  type OnboardingStepId,
} from "./onboardingModel";

interface OnboardingViewProps {
  initialScan: EnvironmentScan | null;
  onComplete: () => void;
}

export function OnboardingView({ initialScan, onComplete }: OnboardingViewProps) {
  const [step, setStep] = useState<OnboardingStepId>("scan");
  const [scan, setScan] = useState<EnvironmentScan | null>(initialScan);
  const [providers, setProviders] = useState<ProviderAdapterStatus[]>([]);
  const [grokKey, setGrokKey] = useState("");
  const [handoffRun, setHandoffRun] = useState<HandoffRun | null>(null);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("Welcome to AgentDeck.");

  useEffect(() => {
    if (initialScan) {
      setScan(initialScan);
    }
  }, [initialScan]);

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
      setStatus("Environment scan completed.");
      setStep("inventory");
    } catch (error) {
      setStatus(`Scan failed: ${formatError(error)}`);
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
    setStatus("Saving the Grok API key to macOS Keychain...");
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
        setStatus("Grok API key stored in Keychain.");
      } else {
        setStatus("Key saved. Provider check will run on the next step.");
      }
    } catch (error) {
      setStatus(`Keychain save failed: ${formatError(error)}`);
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
              Scan your machine, connect Grok, and confirm one safe handoff before
              entering the control plane.
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
                onClick={() => {
                  setStep("grok-key");
                  void refreshProviders();
                }}
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

        {step === "grok-key" ? (
          <section className="onboarding-card">
            <h3>Connect Grok (xAI API)</h3>
            <p>
              Grok is the recommended first cloud provider because the xAI API free
              tier offers strong value. Keys are stored in macOS Keychain.
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
                ? "A Grok key is already available in Keychain or your environment."
                : "You can skip this step and add the key later in Providers."}
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
                <button disabled={busy} onClick={advanceStep} type="button">
                  Continue
                </button>
              ) : null}
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