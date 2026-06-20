import { useEffect, useMemo, useState } from "react";
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
  ProviderList,
  type ProviderScopeFilter,
} from "./components/ProviderList";
import { ProviderDetail } from "./components/ProviderDetail";
import {
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
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [scopeFilter, setScopeFilter] = useState<ProviderScopeFilter>("all");

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

  const filteredProviders = useMemo(() => {
    if (scopeFilter === "all") {
      return providers;
    }
    return providers.filter((provider) =>
      scopeFilter === "local"
        ? provider.authMode === "none"
        : provider.authMode !== "none",
    );
  }, [providers, scopeFilter]);

  useEffect(() => {
    if (providers.length === 0) {
      return;
    }
    if (selectedId === null) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setSelectedId(providers[0].id);
    } else {
      const selectedVisible = filteredProviders.some(
        (provider) => provider.id === selectedId,
      );
      if (!selectedVisible && filteredProviders.length > 0) {
        setSelectedId(filteredProviders[0].id);
      }
    }
  }, [providers, filteredProviders, selectedId]);

  const selectedProvider = useMemo(
    () => providers.find((provider) => provider.id === selectedId) ?? null,
    [providers, selectedId],
  );

  const onlineCount = providers.filter(
    (provider) => provider.verifiedAvailable,
  ).length;
  const storedCount = providers.filter(
    (provider) => provider.credentialStatus === "stored",
  ).length;
  const needKeyCount = providers.filter(
    (provider) =>
      provider.authMode !== "none" &&
      (provider.credentialStatus === "missing" ||
        provider.credentialStatus === "unreadable" ||
        provider.credentialStatus === "import-failed"),
  ).length;

  const selectedBusy =
    selectedProvider !== null && busyProvider === selectedProvider.id;
  const importBusy = busyProvider === "legacy-import";
  const anyBusy = busyProvider !== null;

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
      setStatus(
        providerId === "xai"
          ? "API key saved (encrypted). Grok MCP bridge synced for shell launchers."
          : "API key saved (encrypted on this device).",
      );
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

  const selectedImportOutcome =
    selectedProvider === null
      ? null
      : importOutcomeForProvider(selectedProvider.id, importOutcomes);

  return (
    <section className="workspace providers-workspace providers-workspace--compact">
      <header className="pv-compact-header">
        <div>
          <p className="eyebrow">Phase 4 / Adapters</p>
          <h2>Provider Adapters</h2>
          <p className="pv-compact-subtitle">
            Inspect local and cloud model endpoints. Cloud checks run only when
            you select Check, and API keys are encrypted on this device.
          </p>
        </div>
        <div className="pv-compact-header-meta">
          <div className="provider-header-actions">
            <button
              className="secondary-button"
              disabled={anyBusy}
              onClick={() => void importLegacyKeys()}
              type="button"
            >
              {importBusy ? "Importing..." : "Import existing Keychain keys"}
            </button>
            <span className="phase-badge">Encrypted on device</span>
          </div>
          <div className="pv-summary">
            <div className="pv-scan-state" role="status">
              <span
                className={anyBusy ? "pulse indicator" : "indicator"}
                aria-hidden="true"
              />
              <span>
                {providers.length > 0
                  ? `${providers.length} provider adapters`
                  : status}
              </span>
            </div>
            {providers.length > 0 ? (
              <>
                <span className="pv-pill pv-pill--on">
                  <b>{onlineCount}</b> online
                </span>
                <span className="pv-pill">
                  <b>{storedCount}</b> keys stored
                </span>
                {needKeyCount > 0 ? (
                  <span className="pv-pill pv-pill--warn">
                    <b>{needKeyCount}</b> need key
                  </span>
                ) : null}
              </>
            ) : null}
          </div>
        </div>
      </header>

      <div className="pv-body">
        <ProviderList
          filter={scopeFilter}
          onFilter={setScopeFilter}
          onSelect={setSelectedId}
          providers={filteredProviders}
          selectedId={selectedId}
        />
        <ProviderDetail
          busy={selectedBusy}
          importOutcome={selectedImportOutcome}
          keyValue={
            selectedProvider === null ? "" : (keys[selectedProvider.id] ?? "")
          }
          onCheck={() => {
            if (selectedProvider !== null) {
              void checkProvider(selectedProvider.id);
            }
          }}
          onKeyChange={(value) => {
            if (selectedProvider !== null) {
              setKeys((current) => ({
                ...current,
                [selectedProvider.id]: value,
              }));
            }
          }}
          onRemoveKey={() => {
            if (selectedProvider !== null) {
              void removeKey(selectedProvider);
            }
          }}
          onSaveKey={() => {
            if (selectedProvider !== null) {
              void saveKey(selectedProvider.id);
            }
          }}
          provider={selectedProvider}
        />
      </div>
    </section>
  );
}

function formatError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}