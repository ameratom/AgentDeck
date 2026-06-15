import type { ProviderAdapterStatus } from "../../../lib/types";
import {
  credentialLabel,
  credentialStatusClass,
} from "../providerModel";

interface ProviderDetailProps {
  provider: ProviderAdapterStatus | null;
  keyValue: string;
  busy: boolean;
  importOutcome: { slotId: string; status: string; detail: string } | null;
  onKeyChange: (value: string) => void;
  onSaveKey: () => void;
  onRemoveKey: () => void;
  onCheck: () => void;
}

function providerHealth(provider: ProviderAdapterStatus): {
  cls: string;
  label: string;
} {
  if (provider.verifiedAvailable) {
    return { cls: "online", label: "Online" };
  }
  if (
    provider.credentialStatus === "unreadable" ||
    provider.credentialStatus === "import-failed"
  ) {
    return { cls: "warn", label: "Needs attention" };
  }
  return { cls: "unchecked", label: "Unchecked" };
}

function usesKey(provider: ProviderAdapterStatus): boolean {
  return provider.authMode !== "none";
}

function isLocalProvider(provider: ProviderAdapterStatus): boolean {
  return provider.authMode === "none";
}

function credentialTitle(authMode: string): string {
  return authMode === "x-api-key" ? "x-api-key header" : "Bearer API key";
}

function noKeyMessage(provider: ProviderAdapterStatus): string {
  if (provider.baseUrl.startsWith("stdio://")) {
    return "This adapter needs no API key — it connects over a local stdio bridge.";
  }
  return "This adapter needs no API key — it runs against a local endpoint.";
}

function checkHint(provider: ProviderAdapterStatus): string {
  return isLocalProvider(provider)
    ? "Probes the local endpoint"
    : "Runs a live request to the cloud endpoint";
}

export function ProviderDetail({
  provider,
  keyValue,
  busy,
  importOutcome,
  onKeyChange,
  onSaveKey,
  onRemoveKey,
  onCheck,
}: ProviderDetailProps) {
  if (provider === null) {
    return (
      <section className="pv-panel pv-detail" aria-label="Provider detail">
        <div className="pv-detail-empty">Select an adapter to inspect.</div>
      </section>
    );
  }

  const health = providerHealth(provider);
  const keyProvider = usesKey(provider);
  const sharedKey =
    provider.id === "codex" || provider.id === "openai-compatible";

  return (
    <section className="pv-panel pv-detail" aria-label="Provider detail">
      <div className="pv-detail-head" aria-live="polite">
        <div>
          <p className="eyebrow">{provider.kind}</p>
          <h3>{provider.name}</h3>
        </div>
        <span className={`pv-hbadge ${health.cls}`}>
          <span className="dot" aria-hidden="true" />
          {health.label}
        </span>
      </div>

      <div className="pv-detail-body">
        <dl className="pv-dgrid">
          <div className="full">
            <dt>Endpoint</dt>
            <dd>{provider.baseUrl}</dd>
          </div>
          <div>
            <dt>Credential</dt>
            <dd className={credentialStatusClass(provider.credentialStatus)}>
              {credentialLabel(provider.credentialStatus)}
            </dd>
          </div>
          <div>
            <dt>Catalog</dt>
            <dd>
              {provider.catalogSource === "none" ? (
                <span className="pv-muted">Not loaded</span>
              ) : (
                `${provider.catalogSource}${provider.verifiedAvailable ? " (verified)" : " (unverified)"}`
              )}
            </dd>
          </div>
          <div className="full">
            <dt>Capabilities</dt>
            <dd>
              <div className="pv-capchips">
                {provider.capabilities.map((capability) => (
                  <span className="pv-cap" key={capability}>
                    {capability}
                  </span>
                ))}
              </div>
            </dd>
          </div>
          <div className="full">
            <dt>Models</dt>
            <dd>
              {provider.models.length > 0 ? (
                provider.models.map((model) => model.id).join(", ")
              ) : (
                <span className="pv-muted">{provider.health.detail}</span>
              )}
            </dd>
          </div>
          {importOutcome ? (
            <div className="full">
              <dt>Last import</dt>
              <dd className={`import-outcome ${importOutcome.status}`}>
                {importOutcome.detail}
              </dd>
            </div>
          ) : null}
        </dl>

        {keyProvider ? (
          <div className="pv-cred">
            <p className="pv-t-eyebrow">Credential</p>
            <p className="pv-cred-title">{credentialTitle(provider.authMode)}</p>
            {sharedKey ? (
              <p className="pv-shared">
                OpenAI-compatible and Codex share this encrypted key — saving or
                removing here affects both.
              </p>
            ) : null}
            <div className="pv-cred-row">
              <input
                aria-label={`${provider.name} API key`}
                autoComplete="off"
                disabled={busy}
                onChange={(event) => onKeyChange(event.target.value)}
                placeholder={
                  provider.credentialStatus === "stored"
                    ? "Replace stored API key…"
                    : "Enter API key…"
                }
                type="password"
                value={keyValue}
              />
              <button
                className="pv-btn"
                disabled={busy || !keyValue.trim()}
                onClick={onSaveKey}
                type="button"
              >
                {busy ? "Saving…" : "Save key"}
              </button>
            </div>
            <div className="pv-cred-foot">
              {provider.credentialStatus === "stored" ? (
                <button
                  className="pv-btn pv-btn--danger secondary-button"
                  disabled={busy}
                  onClick={onRemoveKey}
                  type="button"
                >
                  Remove key
                </button>
              ) : null}
              <span className="pv-stored-note">
                <svg
                  aria-hidden="true"
                  className="pv-lock"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  viewBox="0 0 24 24"
                >
                  <rect height="10" rx="2" width="14" x="5" y="11" />
                  <path d="M8 11V7a4 4 0 0 1 8 0v4" />
                </svg>
                {provider.credentialStatus === "stored"
                  ? "Encrypted on this device"
                  : "Stored encrypted after save"}
              </span>
            </div>
          </div>
        ) : (
          <div className="pv-nokey">{noKeyMessage(provider)}</div>
        )}
      </div>

      <div className="pv-detail-foot">
        <button
          className="pv-btn"
          disabled={busy}
          onClick={onCheck}
          type="button"
        >
          {busy ? "Checking…" : "Check provider"}
        </button>
        <p className="pv-foot-note">{checkHint(provider)}</p>
      </div>
    </section>
  );
}