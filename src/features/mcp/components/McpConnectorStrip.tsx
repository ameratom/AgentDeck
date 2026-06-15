import type { ProjectConnectorSettings } from "../../../lib/types";

type McpConnectorStripProps = {
  settings: ProjectConnectorSettings;
  saving: boolean;
  onToggle: (
    key:
      | "filesystemEnabled"
      | "gitEnabled"
      | "claudeCodeServeEnabled"
      | "grokMcpEnabled"
      | "xaiResearchMcpEnabled",
    enabled: boolean,
  ) => void;
  onSave: () => void;
};

const CONNECTORS: {
  key: keyof Pick<
    ProjectConnectorSettings,
    | "filesystemEnabled"
    | "gitEnabled"
    | "claudeCodeServeEnabled"
    | "grokMcpEnabled"
    | "xaiResearchMcpEnabled"
  >;
  label: string;
}[] = [
  { key: "filesystemEnabled", label: "Filesystem" },
  { key: "gitEnabled", label: "Git" },
  { key: "claudeCodeServeEnabled", label: "Claude serve" },
  { key: "grokMcpEnabled", label: "Grok" },
  { key: "xaiResearchMcpEnabled", label: "xAI" },
];

export function McpConnectorStrip({
  settings,
  saving,
  onToggle,
  onSave,
}: McpConnectorStripProps) {
  return (
    <section
      aria-label="Active project connector profile"
      className="mcp-conn-strip"
    >
      <div className="mcp-conn-strip-label">
        <p className="eyebrow">Active project · Export only</p>
        <strong>{settings.projectName} connector profile</strong>
      </div>

      <div className="mcp-conn-pills" role="group" aria-label="Connector toggles">
        {CONNECTORS.map((connector) => (
          <button
            aria-pressed={settings[connector.key]}
            className={
              settings[connector.key]
                ? "mcp-conn-pill active"
                : "mcp-conn-pill"
            }
            disabled={saving}
            key={connector.key}
            onClick={() => onToggle(connector.key, !settings[connector.key])}
            type="button"
          >
            {connector.label}
          </button>
        ))}
      </div>

      <div className="mcp-conn-actions">
        <span className="mcp-conn-path" title={settings.claudeExportPath}>
          {settings.claudeExportPath}
        </span>
        <button disabled={saving} onClick={onSave} type="button">
          {saving ? "Exporting..." : "Save & export"}
        </button>
      </div>
    </section>
  );
}