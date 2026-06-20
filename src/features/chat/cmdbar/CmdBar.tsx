import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type RefObject,
  type SetStateAction,
} from "react";
import {
  loadProjectConnectorSettings,
  saveProjectConnectorSettings,
} from "../../../lib/invoke";
import type { ProjectConnectorSettings, ProjectContext } from "../../../lib/types";
import {
  AgentIcon,
  AppsIcon,
  ArrowUpIcon,
  ChevronDownIcon,
  MicIcon,
  PlusIcon,
  StopIcon,
  VoiceIcon,
} from "./cmdbarIcons";
import { useCmdBarDictation } from "./useCmdBarDictation";

export type ChatEffort = "Low" | "Medium" | "High";

const EFFORT_OPTIONS: ChatEffort[] = ["Low", "Medium", "High"];
const EFFORT_STORAGE_KEY = "agentdeck.chat.effort";

const CONNECTOR_TOGGLES: {
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
  { key: "grokMcpEnabled", label: "Grok MCP" },
  { key: "xaiResearchMcpEnabled", label: "xAI research" },
];

function readStoredEffort(): ChatEffort {
  const stored = localStorage.getItem(EFFORT_STORAGE_KEY);
  if (stored === "Low" || stored === "Medium" || stored === "High") {
    return stored;
  }
  return "Medium";
}

function autogrow(element: HTMLTextAreaElement): void {
  element.style.height = "auto";
  element.style.height = `${Math.min(element.scrollHeight, 200)}px`;
}

function connectorPayload(
  settings: ProjectConnectorSettings,
): Parameters<typeof saveProjectConnectorSettings>[0] {
  return {
    filesystemEnabled: settings.filesystemEnabled,
    gitEnabled: settings.gitEnabled,
    claudeCodeServeEnabled: settings.claudeCodeServeEnabled,
    grokMcpEnabled: settings.grokMcpEnabled,
    xaiResearchMcpEnabled: settings.xaiResearchMcpEnabled,
  };
}

function countEnabledConnectors(settings: ProjectConnectorSettings | null): number {
  if (!settings) {
    return 0;
  }
  return CONNECTOR_TOGGLES.filter((entry) => settings[entry.key]).length;
}

interface CmdBarProps {
  project: ProjectContext | null;
  draft: string;
  setDraft: Dispatch<SetStateAction<string>>;
  composerRef: RefObject<HTMLTextAreaElement | null>;
  loading: boolean;
  previewBlocked: boolean;
  clearing: boolean;
  sending: boolean;
  canSend: boolean;
  composerPlaceholder: string;
  composerHint: string;
  selectedModel: string;
  selectedProviderId: string;
  enableAgentTools: boolean;
  setEnableAgentTools: (value: boolean | ((current: boolean) => boolean)) => void;
  visibleMessageCount: number;
  onSubmit: () => void;
  onStop: () => void;
  onClear: () => void;
}

export function CmdBar({
  project,
  draft,
  setDraft,
  composerRef,
  loading,
  previewBlocked,
  clearing,
  sending,
  canSend,
  composerPlaceholder,
  composerHint,
  selectedModel,
  selectedProviderId,
  enableAgentTools,
  setEnableAgentTools,
  visibleMessageCount,
  onSubmit,
  onStop,
  onClear,
}: CmdBarProps) {
  const [effort, setEffort] = useState<ChatEffort>(readStoredEffort);
  const [addOpen, setAddOpen] = useState(false);
  const [appsOpen, setAppsOpen] = useState(false);
  const [effortOpen, setEffortOpen] = useState(false);
  const [connectors, setConnectors] = useState<ProjectConnectorSettings | null>(
    null,
  );
  const [connectorsLoading, setConnectorsLoading] = useState(false);
  const [connectorsSaving, setConnectorsSaving] = useState(false);
  const saveTimerRef = useRef<number | null>(null);
  const popoverRootRef = useRef<HTMLFormElement | null>(null);

  const appendTranscript = useCallback(
    (text: string) => {
      setDraft((current) => {
        const trimmed = current.trimEnd();
        return trimmed ? `${trimmed} ${text}` : text;
      });
    },
    [setDraft],
  );

  const { speechSupported, listening, toggleDictation } =
    useCmdBarDictation(appendTranscript);

  const enabledConnectorCount = countEnabledConnectors(connectors);
  const agentToolsAvailable = selectedProviderId === "xai";
  const appsAvailable = project !== null;
  const inputPlaceholder =
    loading || previewBlocked || !selectedModel
      ? composerPlaceholder
      : "Describe a task";
  const hasDraft = draft.trim() !== "";

  useEffect(() => {
    localStorage.setItem(EFFORT_STORAGE_KEY, effort);
  }, [effort]);

  useEffect(() => {
    const element = composerRef.current;
    if (element) {
      autogrow(element);
    }
  }, [composerRef, draft]);

  useEffect(() => {
    if (!appsOpen || connectors !== null || !appsAvailable) {
      return;
    }

    let cancelled = false;
    const timer = window.setTimeout(() => {
      setConnectorsLoading(true);
      void loadProjectConnectorSettings()
        .then((settings) => {
          if (!cancelled) {
            setConnectors(settings);
          }
        })
        .catch(() => {
          if (!cancelled) {
            setConnectors(null);
          }
        })
        .finally(() => {
          if (!cancelled) {
            setConnectorsLoading(false);
          }
        });
    }, 0);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [appsAvailable, appsOpen, connectors]);

  useEffect(() => {
    return () => {
      if (saveTimerRef.current !== null) {
        window.clearTimeout(saveTimerRef.current);
      }
    };
  }, []);

  const closePopovers = useCallback(() => {
    setAddOpen(false);
    setAppsOpen(false);
    setEffortOpen(false);
  }, []);

  useEffect(() => {
    if (!addOpen && !appsOpen && !effortOpen) {
      return;
    }

    const handlePointerDown = (event: PointerEvent) => {
      const root = popoverRootRef.current;
      if (!root || root.contains(event.target as Node)) {
        return;
      }
      closePopovers();
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closePopovers();
      }
    };

    document.addEventListener("pointerdown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [addOpen, appsOpen, closePopovers, effortOpen]);

  const scheduleConnectorSave = useCallback((nextSettings: ProjectConnectorSettings) => {
    if (saveTimerRef.current !== null) {
      window.clearTimeout(saveTimerRef.current);
    }
    saveTimerRef.current = window.setTimeout(() => {
      setConnectorsSaving(true);
      void saveProjectConnectorSettings(connectorPayload(nextSettings))
        .then(setConnectors)
        .catch(() => undefined)
        .finally(() => setConnectorsSaving(false));
    }, 400);
  }, []);

  const toggleConnector = useCallback(
    (
      key: (typeof CONNECTOR_TOGGLES)[number]["key"],
      enabled: boolean,
    ) => {
      setConnectors((current) => {
        if (!current) {
          return current;
        }
        const next = { ...current, [key]: enabled };
        scheduleConnectorSave(next);
        return next;
      });
    },
    [scheduleConnectorSave],
  );

  const handleGoClick = () => {
    if (sending) {
      void onStop();
      return;
    }
    if (hasDraft) {
      void onSubmit();
      return;
    }
    toggleDictation();
  };

  const goAriaLabel = sending ? "Stop" : hasDraft ? "Send" : "Voice";

  return (
    <form
      className="cmdbar"
      onSubmit={(event) => {
        event.preventDefault();
        void onSubmit();
      }}
      ref={popoverRootRef}
    >
      {composerHint ? (
        <p
          className="cmdbar-hint"
          id="chat-composer-hint"
          role="status"
        >
          {composerHint}
        </p>
      ) : null}

      <textarea
        aria-describedby={composerHint ? "chat-composer-hint" : undefined}
        aria-label="Message"
        className="cmdbar-input"
        disabled={loading || previewBlocked || clearing}
        onChange={(event) => {
          setDraft(event.target.value);
          autogrow(event.target);
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter" && !event.shiftKey) {
            event.preventDefault();
            void onSubmit();
          }
        }}
        placeholder={inputPlaceholder}
        ref={composerRef}
        rows={1}
        value={draft}
      />

      <div className="cmdbar-row">
        <div className="cmdbar-left">
          <div className="cmdbar-pop">
            <button
              aria-expanded={addOpen}
              aria-haspopup="menu"
              aria-label="Add"
              className="cmdbar-icon"
              onClick={() => setAddOpen((open) => !open)}
              type="button"
            >
              <PlusIcon />
            </button>
            {addOpen ? (
              <div className="cmdbar-menu" role="menu">
                <button
                  className="cmdbar-menu-item"
                  disabled={sending || clearing || visibleMessageCount === 0}
                  onClick={() => {
                    closePopovers();
                    void onClear();
                  }}
                  role="menuitem"
                  type="button"
                >
                  Clear conversation
                </button>
                <button
                  className="cmdbar-menu-item is-disabled"
                  disabled
                  role="menuitem"
                  title="Coming soon"
                  type="button"
                >
                  Attach file…
                </button>
              </div>
            ) : null}
          </div>

          <button
            aria-pressed={enableAgentTools}
            className={`cmdbar-chip${enableAgentTools ? " is-on" : ""}`}
            disabled={!agentToolsAvailable || loading || sending}
            onClick={() => setEnableAgentTools((current) => !current)}
            title={
              agentToolsAvailable
                ? undefined
                : "Agent tools available on Grok (xAI)"
            }
            type="button"
          >
            <AgentIcon />
            <span>Agent</span>
          </button>

          <div className="cmdbar-pop">
            <button
              aria-expanded={appsOpen}
              aria-haspopup="menu"
              className="cmdbar-chip"
              disabled={!appsAvailable}
              onClick={() => {
                if (appsAvailable) {
                  setAppsOpen((open) => !open);
                }
              }}
              title={
                appsAvailable
                  ? undefined
                  : "Select an active project to configure connectors"
              }
              type="button"
            >
              <AppsIcon />
              <span>Apps</span>
              {enabledConnectorCount > 0 ? (
                <em className="cmdbar-badge">{enabledConnectorCount}</em>
              ) : null}
              <ChevronDownIcon />
            </button>
            {appsOpen ? (
              <div className="cmdbar-menu cmdbar-menu--apps" role="menu">
                {connectorsLoading ? (
                  <p className="cmdbar-menu-note">Loading connectors…</p>
                ) : connectors ? (
                  CONNECTOR_TOGGLES.map((entry) => (
                    <label className="cmdbar-menu-toggle" key={entry.key}>
                      <span>{entry.label}</span>
                      <input
                        checked={connectors[entry.key]}
                        disabled={connectorsSaving}
                        onChange={(event) =>
                          toggleConnector(entry.key, event.target.checked)
                        }
                        type="checkbox"
                      />
                    </label>
                  ))
                ) : (
                  <p className="cmdbar-menu-note">Connectors unavailable.</p>
                )}
                {connectorsSaving ? (
                  <p className="cmdbar-menu-note">Saving…</p>
                ) : null}
              </div>
            ) : null}
          </div>
        </div>

        <div className="cmdbar-right">
          <div className="cmdbar-pop">
            <button
              aria-expanded={effortOpen}
              aria-haspopup="menu"
              className="cmdbar-effort"
              onClick={() => setEffortOpen((open) => !open)}
              type="button"
            >
              <span>{effort}</span>
              <ChevronDownIcon />
            </button>
            {effortOpen ? (
              <div className="cmdbar-menu cmdbar-menu--right" role="menu">
                {EFFORT_OPTIONS.map((option) => (
                  <button
                    className={
                      option === effort
                        ? "cmdbar-menu-item is-selected"
                        : "cmdbar-menu-item"
                    }
                    key={option}
                    onClick={() => {
                      setEffort(option);
                      closePopovers();
                    }}
                    role="menuitemradio"
                    aria-checked={option === effort}
                    type="button"
                  >
                    {option}
                  </button>
                ))}
                <p className="cmdbar-menu-note">
                  Saved locally; not sent to the model yet.
                </p>
              </div>
            ) : null}
          </div>

          <button
            aria-label="Dictate"
            className={`cmdbar-icon${listening ? " is-live" : ""}`}
            disabled={!speechSupported}
            onClick={toggleDictation}
            title={
              speechSupported
                ? undefined
                : "Dictation not supported here"
            }
            type="button"
          >
            <MicIcon />
          </button>

          <button
            aria-label={goAriaLabel}
            className="cmdbar-go"
            disabled={!sending && hasDraft && !canSend}
            onClick={handleGoClick}
            type="button"
          >
            {sending ? (
              <StopIcon />
            ) : hasDraft ? (
              <ArrowUpIcon />
            ) : (
              <VoiceIcon />
            )}
          </button>
        </div>
      </div>
    </form>
  );
}
