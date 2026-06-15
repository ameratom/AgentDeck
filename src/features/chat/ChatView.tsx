import { Channel } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  buildChatRequest,
  resolvePreferredModel,
  resolveSuggestedChatModel,
  selectDefaultProvider,
  toChatHistory,
} from "./chatModel";
import {
  credentialLabel,
  providerCredentialBlocked,
  providerDispatchBlocked,
} from "../providers/providerModel";
import {
  cancelStreamChat,
  checkProviderAdapter,
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
  const [status, setStatus] = useState("Loading provider adapters...");
  const [loading, setLoading] = useState(true);
  const [sending, setSending] = useState(false);
  const [refreshingModels, setRefreshingModels] = useState(false);
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
  const dispatchBlocked = providerDispatchBlocked(selectedProvider);
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
  const canSend =
    selectedProviderId !== "" &&
    selectedModel !== "" &&
    draft.trim() !== "" &&
    !sending &&
    !dispatchBlocked;

  const refreshProviderModels = useCallback(async (
    providerId: string,
  ): Promise<ProviderAdapterStatus | null> => {
    setRefreshingModels(true);
    setStatus(`Loading models for ${providerId}...`);
    try {
      const nextProvider = await checkProviderAdapter({ providerId });
      setProviders((current) =>
        current.map((provider) =>
          provider.id === nextProvider.id ? nextProvider : provider,
        ),
      );
      setSelectedModel((current) =>
        nextProvider.verifiedAvailable
          ? resolvePreferredModel(nextProvider.models, current)
          : current,
      );
      setStatus(
        nextProvider.verifiedAvailable
          ? `${nextProvider.name} ready with ${nextProvider.models.length} models.`
          : `${nextProvider.name}: ${nextProvider.health.detail}`,
      );
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
        setStatus(`Loaded ${nextProviders.length} provider adapters.`);
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
    userOverrodeProviderRef.current = false;
    setDismissedSuggestionKey(null);
    lastAutoAppliedRef.current = null;
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
          `Router target ${routeSuggestion.targetProviderId} is no longer available. Update the rule in Settings.`,
        );
        return;
      }
      if (providerCredentialBlocked(targetProvider)) {
        setStatus(
          `${targetProvider.name} needs usable credentials before this suggestion can be applied.`,
        );
        return;
      }

      setSelectedProviderId(targetProvider.id);
      const refreshedProvider = await refreshProviderModels(targetProvider.id);
      if (!refreshedProvider?.verifiedAvailable) {
        return;
      }
      setSelectedModel(
        resolveSuggestedChatModel(
          refreshedProvider.models,
          routeSuggestion.targetModelId,
        ),
      );
      userOverrodeProviderRef.current = false;
      const prefix = mode === "auto" ? "Auto-applied" : "Applied";
      setStatus(
        `${prefix} router rule "${routeSuggestion.ruleName}" (${routeSuggestion.reason})`,
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
    if (!canSend || content === "") {
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
    setStatus("Streaming response...");

    const channel = new Channel<ChatStreamEvent>();
    channel.onmessage = (event) => {
      if (event.event === "token") {
        setStreamingContent((current) => current + event.data.content);
      }
      if (event.event === "error") {
        setStatus(`Chat failed: ${event.data.message}`);
      }
    };

    try {
      const response = await streamChatMessage(
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
      setMessages([
        ...currentMessages,
        { ...optimisticUser, id: `pending:${Date.now()}` },
        response.message,
      ]);
      setStreamingContent("");
      setStatus(
        response.finishReason
          ? `Response complete (${response.finishReason}).`
          : "Response complete.",
      );
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
  const storedMessageCount = toChatHistory(messages).length;

  useEffect(() => {
    const list = messageListRef.current;
    if (!list) {
      return;
    }
    list.scrollTop = list.scrollHeight;
  }, [renderedMessages.length, streamingContent]);

  return (
    <section className="workspace chat-workspace chat-workspace--compact">
      <header className="chat-compact-header">
        <div>
          <p className="eyebrow">Phase 4 / Agentic OS Chat</p>
          <h2>Unified Chat</h2>
          <p className="chat-compact-subtitle">
            Route conversation across LM Studio, Grok, Claude, Codex, and Claude
            Code with streaming responses and persisted local history.
          </p>
          <p className="chat-compact-scope">
            {project
              ? `Scoped to ${project.name} at ${project.path}`
              : "No active project. Chat is using the global conversation."}
          </p>
        </div>
        <span className="phase-badge">Multi-provider</span>
      </header>

      <section className="chat-panel">
        <div className="chat-toolbar">
          <label className="chat-field chat-field--provider">
            <span>Provider</span>
            <select
              disabled={loading || sending || providers.length === 0}
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

          <label className="chat-field chat-field--model">
            <span>Model</span>
            <select
              disabled={
                loading || sending || !selectedProvider || modelOptions.length === 0
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
                <option value="">Load models for provider</option>
              )}
            </select>
          </label>

          <button
            className="chat-toolbar-btn"
            disabled={!selectedProvider || refreshingModels || sending}
            onClick={() => {
              if (selectedProvider) {
                void refreshProviderModels(selectedProvider.id);
              }
            }}
            type="button"
          >
            {refreshingModels ? "Loading..." : "Load models"}
          </button>

          {selectedProviderId === "xai" ? (
            <label className="chat-tools-toggle">
              <input
                checked={enableAgentTools}
                disabled={loading || sending}
                onChange={(event) => setEnableAgentTools(event.target.checked)}
                type="checkbox"
              />
              <span>AgentDeck tools</span>
            </label>
          ) : null}

          {previewBlocked ? (
            <button
              className="inline-link-button chat-toolbar-link"
              onClick={onOpenProviders}
              type="button"
            >
              Open Providers
            </button>
          ) : null}

          {selectedProvider ? (
            <span className="provider-health chat-toolbar-health">
              {credentialLabel(selectedProvider.credentialStatus)}
              {selectedProvider.catalogSource !== "none"
                ? ` / ${selectedProvider.catalogSource} catalog`
                : ""}
            </span>
          ) : null}
        </div>

        <p className="chat-status-strip" role="status">
          <span className={sending || loading ? "pulse indicator" : "indicator"} />
          <span className="chat-status-text">{status}</span>
          <span className="chat-status-count">
            {storedMessageCount} stored message{storedMessageCount === 1 ? "" : "s"}
          </span>
        </p>

        <div className="chat-panel-body">
          {showRouterSuggestion && routeSuggestion ? (
            <div className="chat-router-suggestion">
              <div>
                <strong>
                  Router suggestion: {routeSuggestion.ruleName}
                  {displayAutoAppliedKey ===
                  routerAutoApplyKey(routeSuggestionRequestKey, routeSuggestion) ? (
                    <span className="router-auto-badge">Auto-applied</span>
                  ) : null}
                </strong>
                <p>
                  Route to {routeSuggestion.targetProviderId}
                  {routeSuggestion.targetModelId
                    ? ` / ${routeSuggestion.targetModelId}`
                    : ""}
                  . {routeSuggestion.reason}
                </p>
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
                  Apply suggestion
                </button>
              </div>
            </div>
          ) : null}

          <div
            className="message-list"
            aria-label="Chat messages"
            ref={messageListRef}
          >
            {renderedMessages.length > 0 ? (
              renderedMessages.map((message, index) => (
                <article
                  className={`message-card ${message.role}`}
                  key={message.id ?? index}
                >
                  <div>
                    <strong>{message.role}</strong>
                    <span>{message.createdAt ?? "pending"}</span>
                  </div>
                  <p>{message.content}</p>
                </article>
              ))
            ) : (
              <div className="empty-chat">
                <h3>No messages yet</h3>
                <p>
                  Pick a provider and model, then send a prompt. AgentDeck stores
                  the exchange locally after the stream completes.
                </p>
              </div>
            )}
          </div>
        </div>

        <form
          className="composer"
          onSubmit={(event) => {
            event.preventDefault();
            void submitMessage();
          }}
        >
          <textarea
            aria-label="Message"
            disabled={loading || selectedModel === "" || dispatchBlocked}
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault();
                void submitMessage();
              }
            }}
            placeholder={
              previewBlocked
                ? "Import or save provider credentials before chatting."
                : selectedProvider && !selectedProvider.verifiedAvailable
                  ? "Check this provider successfully before chatting."
                : "Ask any configured agent something..."
            }
            rows={2}
            value={draft}
          />
          <div className="composer-foot">
            <span className="composer-count">
              {storedMessageCount} stored message
              {storedMessageCount === 1 ? "" : "s"} in this conversation
            </span>
            <div className="chat-actions">
              {sending ? (
                <button
                  className="secondary-button chat-stop-btn"
                  onClick={() => void stopStreaming()}
                  type="button"
                >
                  Stop
                </button>
              ) : null}
              <button className="chat-send-btn" disabled={!canSend} type="submit">
                {sending ? "Streaming..." : "Send"}
              </button>
            </div>
          </div>
        </form>
      </section>
    </section>
  );
}

function formatError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
