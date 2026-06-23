import { Channel } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  buildChatRequest,
  describeChatSendBlock,
  formatBubbleTimestamp,
  resolvePreferredModel,
  resolveSuggestedChatModel,
  selectDefaultProvider,
  visibleChatMessages,
} from "./chatModel";
import {
  credentialLabel,
  providerCredentialBlocked,
  providerDispatchBlocked,
  providerHasDispatchableModels,
} from "../providers/providerModel";
import {
  cancelStreamChat,
  checkProviderAdapter,
  clearChatMessages,
  listProviderAdapters,
  loadAppSettings,
  loadChatMessages,
  loadChatPreferences,
  saveChatPreferences,
  streamChatMessage,
  suggestHandoffRoute,
} from "../../lib/invoke";
import { routerAutoApplyKey } from "../settings/routerAutoApplyModel";
import {
  shouldAutoApplyRouterSuggestion,
  shouldShowRouterSuggestion,
} from "../settings/routerSuggestionModel";
import type {
  ChatMessage,
  ChatStreamEvent,
  HandoffRouteSuggestion,
  ProjectContext,
  ProviderAdapterStatus,
} from "../../lib/types";
import { CmdBar } from "./cmdbar/CmdBar";

interface ChatViewProps {
  project: ProjectContext | null;
  onOpenProviders: () => void;
}

interface RouteSuggestionResult {
  requestKey: string;
  suggestion: HandoffRouteSuggestion | null;
}

export function ChatView({ project, onOpenProviders }: ChatViewProps) {
  const conversationId = project
    ? `conversation:${project.id}`
    : "conversation:agentdeck-local";
  const [providers, setProviders] = useState<ProviderAdapterStatus[]>([]);
  const [selectedProviderId, setSelectedProviderId] = useState("");
  const [selectedModel, setSelectedModel] = useState("");
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [draft, setDraft] = useState("");
  const [streamingContent, setStreamingContent] = useState("");
  const [status, setStatus] = useState("");
  const [loading, setLoading] = useState(true);
  const [sending, setSending] = useState(false);
  const [refreshingModels, setRefreshingModels] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [clearConfirmOpen, setClearConfirmOpen] = useState(false);
  const [enableAgentTools, setEnableAgentTools] = useState(false);
  const [routeSuggestionResult, setRouteSuggestionResult] =
    useState<RouteSuggestionResult | null>(null);
  const [routerAutoApply, setRouterAutoApply] = useState(true);
  const [displayAutoAppliedKey, setDisplayAutoAppliedKey] = useState<
    string | null
  >(null);
  const lastAutoAppliedRef = useRef<string | null>(null);
  const userOverrodeProviderRef = useRef(false);
  const messageListRef = useRef<HTMLDivElement | null>(null);
  const composerRef = useRef<HTMLTextAreaElement | null>(null);
  const [dismissedSuggestionKey, setDismissedSuggestionKey] = useState<
    string | null
  >(null);

  const selectedProvider =
    providers.find((provider) => provider.id === selectedProviderId) ?? null;
  const modelOptions = useMemo(
    () =>
      (selectedProvider?.models ?? []).filter(
        (model) => !model.id.toLowerCase().includes("embed"),
      ),
    [selectedProvider],
  );
  const previewBlocked = providerCredentialBlocked(selectedProvider);
  const routeSuggestionRequestKey = draft.trim()
    ? JSON.stringify(["agent:agentdeck", draft.trim()])
    : "";
  const routeSuggestion =
    routeSuggestionResult?.requestKey === routeSuggestionRequestKey
      ? routeSuggestionResult.suggestion
      : null;
  const showRouterSuggestion = shouldShowRouterSuggestion(
    routeSuggestion,
    selectedProviderId,
    selectedModel,
    dismissedSuggestionKey,
    routeSuggestionRequestKey,
  );
  const sendBlockReason = describeChatSendBlock({
    selectedProviderId,
    selectedModel,
    draft,
    sending,
    provider: selectedProvider,
  });
  const canSend = sendBlockReason === null && draft.trim() !== "";
  const composerPlaceholder = loading
    ? "Loading chat..."
    : previewBlocked
      ? "Add provider credentials — open More or Providers."
      : !selectedProvider
        ? "Select a provider in the top bar."
        : !providerHasDispatchableModels(selectedProvider)
          ? "Load models from the top bar."
          : selectedModel === ""
            ? "Choose a model in the top bar."
            : "Message Grok…";
  const composerHint = sending
    ? "Streaming response…"
    : sendBlockReason && draft.trim()
      ? sendBlockReason
      : status.startsWith("Chat failed") ||
          status.startsWith("Model load failed") ||
          status.startsWith("Chat setup failed")
        ? status
        : "";
  const headerStatus =
    sending || loading
      ? composerHint || status || "Working…"
      : composerHint || status || "Ready";

  const refreshProviderModels = useCallback(async (
    providerId: string,
  ): Promise<ProviderAdapterStatus | null> => {
    setRefreshingModels(true);
    try {
      const nextProvider = await checkProviderAdapter({ providerId });
      setProviders((current) =>
        current.map((provider) =>
          provider.id === nextProvider.id ? nextProvider : provider,
        ),
      );
      setSelectedModel((current) =>
        nextProvider.models.length > 0
          ? resolvePreferredModel(nextProvider.models, current)
          : current,
      );
      setStatus("");
      return nextProvider;
    } catch (error) {
      setStatus(`Model load failed: ${formatError(error)}`);
      return null;
    } finally {
      setRefreshingModels(false);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    void loadAppSettings()
      .then((settings) => {
        if (!cancelled) {
          setRouterAutoApply(settings.routerAutoApply);
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    async function loadChatState(): Promise<void> {
      setLoading(true);
      try {
        const [nextProviders, nextMessages, preferences] = await Promise.all([
          listProviderAdapters(),
          loadChatMessages(conversationId),
          loadChatPreferences(),
        ]);
        if (cancelled) {
          return;
        }

        setProviders(nextProviders);
        setMessages(nextMessages);

        const providerId = selectDefaultProvider(
          nextProviders,
          preferences.lastProviderId,
        );
        setSelectedProviderId(providerId);
        const provider =
          nextProviders.find((entry) => entry.id === providerId) ?? null;
        const modelId = resolvePreferredModel(
          provider?.models ?? [],
          preferences.lastModelId,
        );
        setSelectedModel(modelId);
        setStatus("");
      } catch (error) {
        if (!cancelled) {
          setStatus(`Chat setup failed: ${formatError(error)}`);
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void loadChatState();
    return () => {
      cancelled = true;
    };
  }, [conversationId]);

  useEffect(() => {
    if (!selectedProviderId || !selectedModel) {
      return;
    }
    void saveChatPreferences({
      lastProviderId: selectedProviderId,
      lastModelId: selectedModel,
    });
  }, [selectedProviderId, selectedModel]);

  useEffect(() => {
    if (!selectedProviderId || loading) {
      return;
    }
    const timer = window.setTimeout(() => {
      void refreshProviderModels(selectedProviderId);
    }, 0);
    return () => window.clearTimeout(timer);
  }, [loading, refreshProviderModels, selectedProviderId]);

  useEffect(() => {
    if (loading || sending || refreshingModels || !selectedProvider) {
      return;
    }
    if (modelOptions.length === 0) {
      return;
    }

    const hasSelectedModel = modelOptions.some(
      (model) => model.id === selectedModel,
    );
    if (!hasSelectedModel) {
      const timer = window.setTimeout(() => {
        setSelectedModel(resolvePreferredModel(modelOptions, selectedModel));
      }, 0);
      return () => window.clearTimeout(timer);
    }
  }, [
    loading,
    modelOptions,
    refreshingModels,
    selectedModel,
    selectedProvider,
    sending,
  ]);

  useEffect(() => {
    userOverrodeProviderRef.current = false;
    lastAutoAppliedRef.current = null;
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setDismissedSuggestionKey(null);
  }, [routeSuggestionRequestKey]);

  useEffect(() => {
    if (!routeSuggestionRequestKey) {
      return;
    }

    let cancelled = false;
    const requestKey = routeSuggestionRequestKey;
    const timer = window.setTimeout(() => {
      void suggestHandoffRoute({
        sourceAgentId: "agent:agentdeck",
        title: "Chat prompt",
        task: draft,
      })
        .then((suggestion) => {
          if (!cancelled) {
            setRouteSuggestionResult({ requestKey, suggestion });
          }
        })
        .catch(() => {
          if (!cancelled) {
            setRouteSuggestionResult({ requestKey, suggestion: null });
          }
        });
    }, 250);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [draft, routeSuggestionRequestKey]);

  const applyRouteSuggestion = useCallback(
    async (mode: "manual" | "auto" = "manual"): Promise<void> => {
      if (!routeSuggestion) {
        return;
      }
      const targetProvider = providers.find(
        (provider) => provider.id === routeSuggestion.targetProviderId,
      );
      if (!targetProvider) {
        setStatus(
          `Router target ${routeSuggestion.targetProviderId} is no longer available.`,
        );
        return;
      }
      if (providerCredentialBlocked(targetProvider)) {
        setStatus(
          `${targetProvider.name} needs credentials before this suggestion can be applied.`,
        );
        return;
      }

      setSelectedProviderId(targetProvider.id);
      const refreshedProvider = await refreshProviderModels(targetProvider.id);
      if (!refreshedProvider || providerDispatchBlocked(refreshedProvider)) {
        setStatus(`${targetProvider.name} is not ready to chat yet.`);
        return;
      }
      setSelectedModel(
        resolveSuggestedChatModel(
          refreshedProvider.models,
          routeSuggestion.targetModelId,
        ),
      );
      userOverrodeProviderRef.current = false;
      setStatus(
        mode === "auto"
          ? `Auto-applied router rule "${routeSuggestion.ruleName}".`
          : `Applied router rule "${routeSuggestion.ruleName}".`,
      );
    },
    [providers, refreshProviderModels, routeSuggestion],
  );

  useEffect(() => {
    if (
      !shouldAutoApplyRouterSuggestion(
        routerAutoApply,
        routeSuggestion,
        routeSuggestionRequestKey,
        lastAutoAppliedRef.current,
        selectedProviderId,
        selectedModel,
        userOverrodeProviderRef.current,
      )
    ) {
      return;
    }
    const nextKey = routerAutoApplyKey(routeSuggestionRequestKey, routeSuggestion);
    lastAutoAppliedRef.current = nextKey;
    void applyRouteSuggestion("auto").then(() => {
      setDisplayAutoAppliedKey(nextKey);
    });
  }, [
    applyRouteSuggestion,
    routeSuggestion,
    routeSuggestionRequestKey,
    routerAutoApply,
    selectedModel,
    selectedProviderId,
  ]);

  async function submitMessage(): Promise<void> {
    const content = draft.trim();
    const blockReason = describeChatSendBlock({
      selectedProviderId,
      selectedModel,
      draft: content,
      sending,
      provider: selectedProvider,
    });
    if (content === "" || blockReason) {
      if (blockReason) {
        setStatus(blockReason);
      }
      return;
    }

    const optimisticUser: ChatMessage = {
      id: null,
      conversationId,
      role: "user",
      content,
      model: selectedModel,
      createdAt: null,
    };
    const currentMessages = messages;
    setMessages([...currentMessages, optimisticUser]);
    setDraft("");
    setSending(true);
    setStreamingContent("");
    setStatus("");

    const channel = new Channel<ChatStreamEvent>();
    channel.onmessage = (event) => {
      if (event.event === "token") {
        setStreamingContent((current) => current + event.data.content);
      }
      if (event.event === "done") {
        setStreamingContent("");
      }
      if (event.event === "error") {
        setStatus(`Chat failed: ${event.data.message}`);
      }
    };

    try {
      await streamChatMessage(
        buildChatRequest(
          conversationId,
          project?.id ?? null,
          selectedProviderId,
          selectedModel,
          currentMessages,
          content,
          selectedProviderId === "xai" && enableAgentTools,
        ),
        channel,
      );
      const refreshedMessages = await loadChatMessages(conversationId);
      setMessages(refreshedMessages);
      setStreamingContent("");
      setStatus("");
    } catch (error) {
      const detail = formatError(error);
      setMessages(currentMessages);
      setStreamingContent("");
      setStatus(`Chat failed: ${detail}`);
      setDraft(content);
    } finally {
      setSending(false);
    }
  }

  async function stopStreaming(): Promise<void> {
    await cancelStreamChat();
    setSending(false);
    setStatus("Chat stream cancelled.");
  }

  const requestClearConversation = useCallback(() => {
    if (sending || clearing || visibleChatMessages(messages).length === 0) {
      return;
    }
    setClearConfirmOpen(true);
  }, [clearing, messages, sending]);

  async function clearConversation(): Promise<void> {
    if (sending || clearing) {
      return;
    }
    setClearing(true);
    try {
      await clearChatMessages(conversationId);
      const refreshedMessages = await loadChatMessages(conversationId);
      setMessages(refreshedMessages);
      setStreamingContent("");
      setDraft("");
      setRouteSuggestionResult(null);
      setDismissedSuggestionKey(null);
      setStatus("Conversation cleared.");
      setClearConfirmOpen(false);
      composerRef.current?.focus();
    } catch (error) {
      setStatus(`Clear failed: ${formatError(error)}`);
    } finally {
      setClearing(false);
    }
  }

  const renderedMessages =
    streamingContent.trim() !== ""
      ? [
          ...messages,
          {
            id: "streaming",
            conversationId,
            role: "assistant" as const,
            content: streamingContent,
            model: selectedModel,
            createdAt: "streaming",
          },
        ]
      : messages;
  const visibleMessages = visibleChatMessages(renderedMessages);

  useEffect(() => {
    const list = messageListRef.current;
    if (!list) {
      return;
    }
    const frame = window.requestAnimationFrame(() => {
      list.scrollTop = list.scrollHeight;
    });
    return () => window.cancelAnimationFrame(frame);
  }, [visibleMessages.length, streamingContent]);

  useEffect(() => {
    if (!clearConfirmOpen) {
      return;
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !clearing) {
        setClearConfirmOpen(false);
      }
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [clearConfirmOpen, clearing]);

  return (
    <section className="workspace chat-workspace chat-workspace--conversation chat-workspace--compact">
      <header className="ch-compact-header">
        <div>
          <p className="eyebrow">Phase 2 / Chat</p>
          <h2>Local Chat</h2>
          <p className="ch-compact-subtitle">
            Send messages to a local or cloud provider. Conversation history
            stays on this Mac.
          </p>
        </div>
        <div className="ch-compact-header-meta">
          <div className="ch-summary" role="status">
            <div className="ch-scan-state">
              <span
                aria-hidden="true"
                className={
                  loading || sending || refreshingModels
                    ? "pulse indicator"
                    : "indicator"
                }
              />
              <span>{headerStatus}</span>
            </div>
            <span className="ch-pill">
              {project?.name ?? "Global"}
            </span>
            <span className="ch-pill">
              <b>{visibleMessages.length}</b> msgs
            </span>
          </div>
        </div>
      </header>

      <div className="ch-body">
      <section className="chat-conversation-panel">
        <header className="chat-conversation-topbar">
          <div className="ch-topbar-controls">
            <label className="chat-field chat-field--provider ch-topbar-field">
              <span>Provider</span>
              <select
                aria-label="Chat provider"
                disabled={loading || sending || providers.length === 0}
                title={
                  providers.length === 0
                    ? "No providers configured"
                    : selectedProvider?.name
                }
                onChange={(event) => {
                  const nextProviderId = event.target.value;
                  userOverrodeProviderRef.current = true;
                  setSelectedProviderId(nextProviderId);
                  const nextProvider = providers.find(
                    (provider) => provider.id === nextProviderId,
                  );
                  setSelectedModel((current) =>
                    resolvePreferredModel(nextProvider?.models ?? [], current),
                  );
                }}
                value={selectedProviderId}
              >
                {providers.map((provider) => (
                  <option key={provider.id} value={provider.id}>
                    {provider.name}
                  </option>
                ))}
              </select>
            </label>

            <label className="chat-field chat-field--model ch-topbar-field">
              <span>Model</span>
              <select
                aria-label="Chat model"
                disabled={
                  loading ||
                  sending ||
                  !selectedProvider ||
                  modelOptions.length === 0
                }
                title={
                  selectedModel ||
                  (modelOptions.length === 0
                    ? "Load models for the selected provider"
                    : "Select a model")
                }
                onChange={(event) => {
                  userOverrodeProviderRef.current = true;
                  setSelectedModel(event.target.value);
                }}
                value={selectedModel}
              >
                {modelOptions.length > 0 ? (
                  modelOptions.map((model) => (
                    <option key={model.id} value={model.id}>
                      {model.id}
                    </option>
                  ))
                ) : (
                  <option value="">Load models</option>
                )}
              </select>
            </label>

            <button
              className="ch-topbar-load-btn"
              disabled={!selectedProvider || refreshingModels || sending}
              onClick={() => {
                if (selectedProvider) {
                  void refreshProviderModels(selectedProvider.id);
                }
              }}
              type="button"
            >
              {refreshingModels ? "Loading…" : "Load models"}
            </button>
          </div>

          <div className="chat-conversation-topbar-actions">
            <button
              className="chat-clear-chat-btn"
              disabled={
                sending || clearing || visibleMessages.length === 0
              }
              onClick={requestClearConversation}
              type="button"
            >
              {clearing ? "Clearing…" : "Clear chat"}
            </button>
            <details className="chat-settings-menu">
              <summary aria-label="Chat settings">More</summary>
              <div className="chat-settings-body">
                {previewBlocked ? (
                  <button
                    className="inline-link-button chat-settings-providers-link"
                    onClick={onOpenProviders}
                    type="button"
                  >
                    Open Providers
                  </button>
                ) : null}

                {selectedProvider ? (
                  <span className="chat-settings-meta">
                    {credentialLabel(selectedProvider.credentialStatus)}
                    {selectedProvider.catalogSource !== "none"
                      ? ` · ${selectedProvider.catalogSource}`
                      : ""}
                    {modelOptions.length === 0 && selectedProvider.health.detail
                      ? ` · ${selectedProvider.health.detail}`
                      : ""}
                  </span>
                ) : null}
              </div>
            </details>
          </div>
        </header>

        <div
          aria-label="Chat messages"
          aria-live="polite"
          className="chat-thread"
          ref={messageListRef}
        >
          {visibleMessages.length > 0 ? (
            visibleMessages.map((message, index) => (
              <div
                className={`chat-bubble-row chat-bubble-row--${message.role}`}
                key={message.id ?? `message-${index}`}
              >
                <div className={`chat-bubble chat-bubble--${message.role}`}>
                  <p>{message.content}</p>
                </div>
                {formatBubbleTimestamp(message.createdAt) ? (
                  <span className="chat-bubble-time">
                    {formatBubbleTimestamp(message.createdAt)}
                  </span>
                ) : null}
              </div>
            ))
          ) : (
            <div className="chat-empty-state">
              <h3>Start a conversation</h3>
              <p>Messages are saved locally on this Mac.</p>
            </div>
          )}
        </div>

        <div className="chat-command-area">
          {showRouterSuggestion && routeSuggestion ? (
            <div className="chat-router-suggestion ch-router-bar">
              <div className="ch-router-copy">
                <strong>Router: {routeSuggestion.ruleName}</strong>
                {displayAutoAppliedKey ===
                routerAutoApplyKey(
                  routeSuggestionRequestKey,
                  routeSuggestion,
                ) ? (
                  <span className="router-auto-badge">Auto-applied</span>
                ) : null}
                <span>
                  → {routeSuggestion.targetProviderId}
                  {routeSuggestion.targetModelId
                    ? ` / ${routeSuggestion.targetModelId}`
                    : ""}
                </span>
              </div>
              <div className="router-suggestion-actions">
                <button
                  className="secondary-button router-dismiss-btn"
                  disabled={refreshingModels || sending}
                  onClick={() =>
                    setDismissedSuggestionKey(
                      routerAutoApplyKey(
                        routeSuggestionRequestKey,
                        routeSuggestion,
                      ),
                    )
                  }
                  type="button"
                >
                  Dismiss
                </button>
                <button
                  disabled={refreshingModels || sending}
                  onClick={() => void applyRouteSuggestion("manual")}
                  type="button"
                >
                  Apply
                </button>
              </div>
            </div>
          ) : null}

          <CmdBar
            canSend={canSend}
            clearing={clearing}
            composerHint={composerHint}
            composerPlaceholder={composerPlaceholder}
            composerRef={composerRef}
            draft={draft}
            enableAgentTools={enableAgentTools}
            loading={loading}
            onClear={requestClearConversation}
            onStop={() => void stopStreaming()}
            onSubmit={() => void submitMessage()}
            previewBlocked={previewBlocked}
            project={project}
            selectedModel={selectedModel}
            selectedProviderId={selectedProviderId}
            sending={sending}
            setDraft={setDraft}
            setEnableAgentTools={setEnableAgentTools}
            visibleMessageCount={visibleMessages.length}
          />
        </div>
      </section>
      </div>

      {clearConfirmOpen ? (
        <div
          className="chat-clear-confirm-backdrop"
          onClick={() => {
            if (!clearing) {
              setClearConfirmOpen(false);
            }
          }}
          role="presentation"
        >
          <div
            aria-labelledby="chat-clear-confirm-title"
            aria-modal="true"
            className="chat-clear-confirm-dialog"
            onClick={(event) => event.stopPropagation()}
            role="dialog"
          >
            <h3 id="chat-clear-confirm-title">Clear this conversation?</h3>
            <p>This cannot be undone. Messages are removed from this Mac only.</p>
            <div className="chat-clear-confirm-actions">
              <button
                className="secondary-button"
                disabled={clearing}
                onClick={() => setClearConfirmOpen(false)}
                type="button"
              >
                Cancel
              </button>
              <button
                className="chat-clear-confirm-danger"
                disabled={clearing}
                onClick={() => void clearConversation()}
                type="button"
              >
                {clearing ? "Clearing…" : "Clear"}
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </section>
  );
}

function formatError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
