import { useEffect, useState } from "react";
import {
  checkProviderAdapter,
  deleteProviderApiKey,
  importLegacyProviderCredentials,
  listProviderAdapters,
  saveProviderApiKey,
} from "../../lib/invoke";
import type {
  LegacyCredentialImportOutcome,
  ProviderAdapterStatus,
} from "../../lib/types";
import {
  credentialLabel,
  credentialStatusClass,
  importOutcomeForProvider,
  replaceProvider,
} from "./providerModel";

export function ProvidersView() {
  const [providers, setProviders] = useState<ProviderAdapterStatus[]>([]);
  const [keys, setKeys] = useState<Record<string, string>>({});
  const [busyProvider, setBusyProvider] = useState<string | null>(null);
  const [importOutcomes, setImportOutcomes] = useState<
    LegacyCredentialImportOutcome[]
  >([]);
  const [status, setStatus] = useState("Loading provider adapter inventory.");

  async function refreshProviders(): Promise<void> {
    try {
      const nextProviders = await listProviderAdapters();
      setProviders(nextProviders);
      setStatus(`${nextProviders.length} provider adapters available.`);
    } catch (error) {
      setStatus(`Provider inventory failed: ${formatError(error)}`);
    }
  }

  useEffect(() => {
    let cancelled = false;

    async function loadProviders(): Promise<void> {
      try {
        const nextProviders = await listProviderAdapters();
        if (!cancelled) {
          setProviders(nextProviders);
          setStatus(`${nextProviders.length} provider adapters available.`);
        }
      } catch (error) {
        if (!cancelled) {
          setStatus(`Provider inventory failed: ${formatError(error)}`);
        }
      }
    }

    void loadProviders();

    return () => {
      cancelled = true;
    };
  }, []);

  async function checkProvider(providerId: string): Promise<void> {
    setBusyProvider(providerId);
    setStatus("Checking the selected provider endpoint...");
    try {
      const checked = await checkProviderAdapter({ providerId });
      setProviders((current) => replaceProvider(current, checked));
      setStatus(`${checked.name}: ${checked.health.detail}`);
    } catch (error) {
      setStatus(`Provider check failed: ${formatError(error)}`);
    } finally {
      setBusyProvider(null);
    }
  }

  async function saveKey(providerId: string): Promise<void> {
    const apiKey = keys[providerId]?.trim() ?? "";
    if (apiKey === "") {
      setStatus("Enter an API key before saving.");
      return;
    }
    setBusyProvider(providerId);
    setStatus("Saving the API key...");
    try {
      await saveProviderApiKey({ providerId, apiKey });
      setKeys((current) => ({ ...current, [providerId]: "" }));
      await refreshProviders();
      setStatus("API key saved (encrypted on this device).");
    } catch (error) {
      setStatus(`Save failed: ${formatError(error)}`);
    } finally {
      setBusyProvider(null);
    }
  }

  async function removeKey(provider: ProviderAdapterStatus): Promise<void> {
    const sharedWarning =
      provider.id === "codex" || provider.id === "openai-compatible"
        ? " This also removes the shared credential from OpenAI-compatible and Codex."
        : "";
    if (!window.confirm(`Remove the ${provider.name} API key?${sharedWarning}`)) {
      return;
    }
    setBusyProvider(provider.id);
    setStatus("Removing the API key...");
    try {
      await deleteProviderApiKey(provider.id);
      await refreshProviders();
      setStatus("API key removed.");
    } catch (error) {
      setStatus(`Removal failed: ${formatError(error)}`);
    } finally {
      setBusyProvider(null);
    }
  }

  async function importLegacyKeys(): Promise<void> {
    setBusyProvider("legacy-import");
    setStatus("Importing legacy Keychain entries. macOS may ask for approval once.");
    try {
      const result = await importLegacyProviderCredentials();
      setImportOutcomes(result.outcomes);
      await refreshProviders();
      const parts = [
        result.imported.length > 0
          ? `Imported ${result.imported.join(", ")}.`
          : "No keys imported.",
        result.verified.length > 0
          ? `Verified ${result.verified.join(", ")}.`
          : "",
        result.conflicts.join(" "),
        result.errors.join(" "),
        ...result.outcomes.map((outcome) => outcome.detail),
      ].filter(Boolean);
      setStatus([...new Set(parts)].join(" "));
    } catch (error) {
      setStatus(`Legacy Keychain import failed: ${formatError(error)}`);
    } finally {
      setBusyProvider(null);
    }
  }

  return (
    <section className="workspace providers-workspace">
      <header>
        <div>
          <p className="eyebrow">Phase 4 / Adapters</p>
          <h2>Provider Adapters</h2>
          <p>
            Inspect local and cloud model endpoints. Cloud checks run only when
            you select Check, and API keys are encrypted on this device.
          </p>
        </div>
        <div className="provider-header-actions">
          <button
            className="secondary-button"
            disabled={busyProvider !== null}
            onClick={() => void importLegacyKeys()}
            type="button"
          >
            {busyProvider === "legacy-import"
              ? "Importing..."
              : "Import existing Keychain keys"}
          </button>
          <span className="phase-badge">Encrypted on device</span>
        </div>
      </header>

      <p className="provider-page-status" role="status">
        <span className={busyProvider ? "pulse indicator" : "indicator"} />
        {status}
      </p>

      <section className="provider-grid" aria-label="Provider adapters">
        {providers.map((provider) => {
          const busy = busyProvider !== null;
          const usesKey = provider.authMode !== "none";
          const importOutcome = importOutcomeForProvider(
            provider.id,
            importOutcomes,
          );
          return (
            <article className="provider-card" key={provider.id}>
              <div className="provider-heading">
                <div>
                  <p className="eyebrow">{provider.kind}</p>
                  <h3>{provider.name}</h3>
                </div>
                <span
                  className={
                    provider.verifiedAvailable
                      ? "provider-health online"
                      : "provider-health"
                  }
                >
                  {provider.verifiedAvailable
                    ? "Online"
                    : provider.credentialStatus === "unreadable" ||
                        provider.credentialStatus === "import-failed"
                      ? "Needs attention"
                      : "Unchecked"}
                </span>
              </div>

              <dl>
                <div>
                  <dt>Endpoint</dt>
                  <dd>{provider.baseUrl}</dd>
                </div>
                <div>
                  <dt>Credential</dt>
                  <dd className={credentialStatusClass(provider.credentialStatus)}>
                    {credentialLabel(provider.credentialStatus)}
                  </dd>
                </div>
                {importOutcome ? (
                  <div>
                    <dt>Last import</dt>
                    <dd className={`import-outcome ${importOutcome.status}`}>
                      {importOutcome.detail}
                    </dd>
                  </div>
                ) : null}
                <div>
                  <dt>Capabilities</dt>
                  <dd>{provider.capabilities.join(", ")}</dd>
                </div>
                <div>
                  <dt>Models</dt>
                  <dd>
                    {provider.models.length > 0
                      ? provider.models.map((model) => model.id).join(", ")
                      : provider.health.detail}
                  </dd>
                </div>
                <div>
                  <dt>Catalog</dt>
                  <dd>
                    {provider.catalogSource === "none"
                      ? "Not loaded"
                      : `${provider.catalogSource}${provider.verifiedAvailable ? " (verified)" : " (unverified)"}`}
                  </dd>
                </div>
              </dl>

              {usesKey ? (
                <div className="credential-controls">
                  {provider.id === "codex" ||
                  provider.id === "openai-compatible" ? (
                    <p>OpenAI-compatible and Codex share this encrypted key.</p>
                  ) : null}
                  <input
                    aria-label={`${provider.name} API key`}
                    autoComplete="off"
                    disabled={busy}
                    onChange={(event) =>
                      setKeys((current) => ({
                        ...current,
                        [provider.id]: event.target.value,
                      }))
                    }
                    placeholder="API key"
                    type="password"
                    value={keys[provider.id] ?? ""}
                  />
                  <button
                    disabled={busy || !(keys[provider.id]?.trim())}
                    onClick={() => void saveKey(provider.id)}
                    type="button"
                  >
                    Save key
                  </button>
                  {provider.credentialStatus === "stored" ? (
                    <button
                      className="secondary-button"
                      disabled={busy}
                      onClick={() => void removeKey(provider)}
                      type="button"
                    >
                      Remove
                    </button>
                  ) : null}
                </div>
              ) : null}

              <button
                className="provider-check"
                disabled={busy}
                onClick={() => void checkProvider(provider.id)}
                type="button"
              >
                {busy ? "Checking..." : "Check provider"}
              </button>
            </article>
          );
        })}
      </section>
    </section>
  );
}

function formatError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
