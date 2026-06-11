import { Channel } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import {
  buildChatRequest,
  selectDefaultModel,
  selectDefaultProvider,
  toChatHistory,
} from "./chatModel";
import { credentialLabel } from "../providers/providerModel";
import {
  cancelStreamChat,
  checkProviderAdapter,
  listProviderAdapters,
  loadChatMessages,
  loadChatPreferences,
  saveChatPreferences,
  streamChatMessage,
} from "../../lib/invoke";
import type {
  ChatMessage,
  ChatStreamEvent,
  ProviderAdapterStatus,
} from "../../lib/types";

const CONVERSATION_ID = "conversation:agentdeck-local";

export function ChatView() {
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

  const selectedProvider =
    providers.find((provider) => provider.id === selectedProviderId) ?? null;
  const modelOptions = useMemo(
    () =>
      (selectedProvider?.models ?? []).filter(
        (model) => !model.id.toLowerCase().includes("embed"),
      ),
    [selectedProvider],
  );
  const previewBlocked =
    selectedProvider !== null &&
    selectedProvider.authMode !== "none" &&
    selectedProvider.credentialStatus === "missing";
  const canSend =
    selectedProviderId !== "" &&
    selectedModel !== "" &&
    draft.trim() !== "" &&
    !sending &&
    !previewBlocked;

  useEffect(() => {
    let cancelled = false;

    async function loadChatState(): Promise<void> {
      setLoading(true);
      try {
        const [nextProviders, nextMessages, preferences] = await Promise.all([
          listProviderAdapters(),
          loadChatMessages(CONVERSATION_ID),
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
        const modelId =
          preferences.lastModelId ||
          selectDefaultModel(provider?.models ?? []);
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
  }, []);

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
    if (!selectedProviderId || loading || sending) {
      return;
    }
    void refreshProviderModels(selectedProviderId);
  }, [selectedProviderId]);

  async function refreshProviderModels(providerId: string): Promise<void> {
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
        current && nextProvider.models.some((model) => model.id === current)
          ? current
          : selectDefaultModel(nextProvider.models),
      );
      setStatus(
        nextProvider.health.available
          ? `${nextProvider.name} ready with ${nextProvider.models.length} models.`
          : `${nextProvider.name}: ${nextProvider.health.detail}`,
      );
    } catch (error) {
      setStatus(`Model load failed: ${formatError(error)}`);
    } finally {
      setRefreshingModels(false);
    }
  }

  async function submitMessage(): Promise<void> {
    const content = draft.trim();
    if (!canSend || content === "") {
      return;
    }

    const optimisticUser: ChatMessage = {
      id: null,
      conversationId: CONVERSATION_ID,
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
          CONVERSATION_ID,
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
            conversationId: CONVERSATION_ID,
            role: "assistant" as const,
            content: streamingContent,
            model: selectedModel,
            createdAt: "streaming",
          },
        ]
      : messages;

  return (
    <section className="workspace chat-workspace">
      <header>
        <div>
          <p className="eyebrow">Phase 4 / Agentic OS Chat</p>
          <h2>Unified Chat</h2>
          <p>
            Route conversation across LM Studio, Grok, Claude, Codex, and Claude
            Code with streaming responses and persisted local history.
          </p>
        </div>
        <span className="phase-badge">Multi-provider</span>
      </header>

      <section className="chat-panel">
        <div className="chat-toolbar">
          <label>
            <span>Provider</span>
            <select
              disabled={loading || sending || providers.length === 0}
              onChange={(event) => {
                const nextProviderId = event.target.value;
                setSelectedProviderId(nextProviderId);
                const nextProvider = providers.find(
                  (provider) => provider.id === nextProviderId,
                );
                setSelectedModel(selectDefaultModel(nextProvider?.models ?? []));
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

          <label>
            <span>Model</span>
            <select
              disabled={
                loading || sending || !selectedProvider || modelOptions.length === 0
              }
              onChange={(event) => setSelectedModel(event.target.value)}
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

          {selectedProvider ? (
            <span className="provider-health">
              {credentialLabel(selectedProvider.credentialStatus)}
            </span>
          ) : null}

          {selectedProviderId === "xai" ? (
            <label className="chat-toggle">
              <input
                checked={enableAgentTools}
                disabled={loading || sending}
                onChange={(event) => setEnableAgentTools(event.target.checked)}
                type="checkbox"
              />
              <span>AgentDeck tools (non-streaming)</span>
            </label>
          ) : null}

          <p className="chat-status" role="status">
            <span className={sending || loading ? "pulse indicator" : "indicator"} />
            {status}
          </p>
        </div>

        <div className="message-list" aria-label="Chat messages">
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

        <form
          className="composer"
          onSubmit={(event) => {
            event.preventDefault();
            void submitMessage();
          }}
        >
          <textarea
            aria-label="Message"
            disabled={loading || selectedModel === "" || previewBlocked}
            onChange={(event) => setDraft(event.target.value)}
            placeholder={
              previewBlocked
                ? "Add provider credentials before chatting with this provider."
                : "Ask any configured agent something..."
            }
            rows={4}
            value={draft}
          />
          <div>
            <span>
              {toChatHistory(messages).length} stored messages in this
              conversation
            </span>
            <div className="chat-actions">
              {sending ? (
                <button
                  className="secondary-button"
                  onClick={() => void stopStreaming()}
                  type="button"
                >
                  Stop
                </button>
              ) : null}
              <button disabled={!canSend} type="submit">
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