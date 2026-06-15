import type { ProviderAdapterStatus } from "../../../lib/types";
import { credentialLabel } from "../providerModel";

export type ProviderScopeFilter = "all" | "local" | "cloud";

interface ProviderListProps {
  providers: ProviderAdapterStatus[];
  selectedId: string | null;
  filter: ProviderScopeFilter;
  onSelect: (id: string) => void;
  onFilter: (filter: ProviderScopeFilter) => void;
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

function kindClass(kind: string): string {
  if (kind === "anthropic") {
    return "anthropic";
  }
  if (kind === "claude-code-mcp") {
    return "cc";
  }
  return "";
}

function shortCredentialLabel(provider: ProviderAdapterStatus): string {
  return credentialLabel(provider.credentialStatus).replace(/ —.*/, "");
}

const FILTERS: ProviderScopeFilter[] = ["all", "local", "cloud"];

export function ProviderList({
  providers,
  selectedId,
  filter,
  onSelect,
  onFilter,
}: ProviderListProps) {
  return (
    <section className="pv-panel pv-list" aria-label="Provider adapters">
      <div className="pv-list-head">
        <div>
          <p className="pv-t-eyebrow">Adapters</p>
          <h3>Local &amp; cloud endpoints</h3>
        </div>
        <div className="pv-filters" role="group" aria-label="Adapter scope">
          {FILTERS.map((scope) => (
            <button
              key={scope}
              className={`pv-chip ${filter === scope ? "active" : ""}`}
              onClick={() => onFilter(scope)}
              type="button"
            >
              {scope === "all" ? "All" : scope === "local" ? "Local" : "Cloud"}
            </button>
          ))}
        </div>
      </div>
      <div className="pv-plist">
        {providers.map((provider) => {
          const health = providerHealth(provider);
          return (
            <button
              key={provider.id}
              aria-pressed={provider.id === selectedId}
              className={`pv-prow ${provider.id === selectedId ? "selected" : ""}`}
              onClick={() => onSelect(provider.id)}
              type="button"
            >
              <span className={`pv-pdot ${health.cls}`} aria-hidden="true" />
              <div className="pv-pinfo">
                <div className="pv-ptop">
                  <span className="pv-pname">{provider.name}</span>
                  <span className={`pv-pkind ${kindClass(provider.kind)}`}>
                    {provider.kind}
                  </span>
                </div>
                <div className="pv-pbot">
                  {shortCredentialLabel(provider)} · {provider.baseUrl}
                </div>
              </div>
              <span className={`pv-phealth ${health.cls}`}>{health.label}</span>
            </button>
          );
        })}
      </div>
    </section>
  );
}