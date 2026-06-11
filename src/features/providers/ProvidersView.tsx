import { useEffect, useState } from "react";
import {
  checkProviderAdapter,
  deleteProviderApiKey,
  listProviderAdapters,
  saveProviderApiKey,
} from "../../lib/invoke";
import type { ProviderAdapterStatus } from "../../lib/types";
import { credentialLabel, replaceProvider } from "./providerModel";

export function ProvidersView() {
  const [providers, setProviders] = useState<ProviderAdapterStatus[]>([]);
  const [keys, setKeys] = useState<Record<string, string>>({});
  const [busyProvider, setBusyProvider] = useState<string | null>(null);
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
    setStatus("Saving the API key to macOS Keychain...");
    try {
      await saveProviderApiKey({ providerId, apiKey });
      setKeys((current) => ({ ...current, [providerId]: "" }));
      await refreshProviders();
      setStatus("API key stored in macOS Keychain.");
    } catch (error) {
      setStatus(`Keychain save failed: ${formatError(error)}`);
    } finally {
      setBusyProvider(null);
    }
  }

  async function removeKey(provider: ProviderAdapterStatus): Promise<void> {
    if (
      !window.confirm(
        `Remove the ${provider.name} API key from macOS Keychain?`,
      )
    ) {
      return;
    }
    setBusyProvider(provider.id);
    setStatus("Removing the API key from macOS Keychain...");
    try {
      await deleteProviderApiKey(provider.id);
      await refreshProviders();
      setStatus("Keychain credential removed.");
    } catch (error) {
      setStatus(`Keychain removal failed: ${formatError(error)}`);
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
            you select Check, and API keys are stored in macOS Keychain.
          </p>
        </div>
        <span className="phase-badge">Keychain backed</span>
      </header>

      <p className="provider-page-status" role="status">
        <span className={busyProvider ? "pulse indicator" : "indicator"} />
        {status}
      </p>

      <section className="provider-grid" aria-label="Provider adapters">
        {providers.map((provider) => {
          const busy = busyProvider === provider.id;
          const usesKey = provider.authMode !== "none";
          return (
            <article className="provider-card" key={provider.id}>
              <div className="provider-heading">
                <div>
                  <p className="eyebrow">{provider.kind}</p>
                  <h3>{provider.name}</h3>
                </div>
                <span
                  className={
                    provider.health.available
                      ? "provider-health online"
                      : "provider-health"
                  }
                >
                  {provider.health.available ? "Online" : "Unchecked"}
                </span>
              </div>

              <dl>
                <div>
                  <dt>Endpoint</dt>
                  <dd>{provider.baseUrl}</dd>
                </div>
                <div>
                  <dt>Credential</dt>
                  <dd>{credentialLabel(provider.credentialStatus)}</dd>
                </div>
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
              </dl>

              {usesKey ? (
                <div className="credential-controls">
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
                  {provider.credentialStatus === "keychain" ? (
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
