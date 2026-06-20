import {
  providerCredentialBlocked,
  providerHasDispatchableModels,
  providerReadyForChat,
} from "../providers/providerModel";
import type {
  ChatMessage,
  ChatMessageInput,
  ChatRequest,
  LocalModel,
  ProviderAdapterStatus,
} from "../../lib/types";

const EMBEDDING_MODEL_PATTERN = /(?:embed|embedding)/i;

export function selectDefaultModel(models: LocalModel[]): string {
  return (
    models.find((model) => !EMBEDDING_MODEL_PATTERN.test(model.id))?.id ??
    models[0]?.id ??
    ""
  );
}

export function resolvePreferredModel(
  models: LocalModel[],
  preferredId?: string,
): string {
  return preferredId && models.some((model) => model.id === preferredId)
    ? preferredId
    : selectDefaultModel(models);
}

export function resolveSuggestedChatModel(
  models: LocalModel[],
  suggestedId: string | null,
): string {
  return suggestedId && models.some((model) => model.id === suggestedId)
    ? suggestedId
    : selectDefaultModel(models);
}

export function toChatHistory(messages: ChatMessage[]): ChatMessageInput[] {
  return messages.map((message) => ({
    role: message.role,
    content: message.content,
  }));
}

const SUBSCRIPTION_CLI_PROVIDER_ORDER = ["claude-code", "codex"] as const;

export function selectDefaultProvider(
  providers: ProviderAdapterStatus[],
  preferredId?: string,
): string {
  if (preferredId && providers.some((provider) => provider.id === preferredId)) {
    return preferredId;
  }
  for (const providerId of SUBSCRIPTION_CLI_PROVIDER_ORDER) {
    const provider = providers.find((entry) => entry.id === providerId);
    if (provider && providerReadyForChat(provider)) {
      return providerId;
    }
  }
  return (
    providers.find((provider) => provider.id === "lm-studio")?.id ??
    providers[0]?.id ??
    ""
  );
}

export function formatMessageTimestamp(value: string | null): string {
  if (!value) {
    return "Sending...";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString();
}

export function formatBubbleTimestamp(value: string | null): string {
  if (!value || value === "streaming") {
    return "";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "";
  }
  return date.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
}

export function visibleChatMessages(messages: ChatMessage[]): ChatMessage[] {
  return messages.filter((message) => message.content.trim() !== "");
}

export function describeChatSendBlock(args: {
  selectedProviderId: string;
  selectedModel: string;
  draft: string;
  sending: boolean;
  provider: ProviderAdapterStatus | null;
}): string | null {
  if (!args.draft.trim()) {
    return null;
  }
  if (args.sending) {
    return "Wait for the current response to finish.";
  }
  if (!args.selectedProviderId) {
    return "Select a provider before sending.";
  }
  if (!args.provider) {
    return "The selected provider is unavailable.";
  }
  if (providerCredentialBlocked(args.provider)) {
    return "Add or import credentials for this provider in Providers.";
  }
  if (!providerHasDispatchableModels(args.provider)) {
    return args.provider.verifiedAvailable
      ? "Load models for the selected provider."
      : "Run Load models to verify this provider before sending.";
  }
  if (!args.selectedModel) {
    return "Choose a model before sending.";
  }
  return null;
}

export function buildChatRequest(
  conversationId: string,
  projectId: string | null,
  providerId: string,
  model: string,
  messages: ChatMessage[],
  nextContent: string,
  enableAgentTools = false,
): ChatRequest {
  return {
    conversationId,
    projectId,
    providerId,
    model,
    enableAgentTools,
    messages: [
      ...toChatHistory(messages),
      {
        role: "user",
        content: nextContent.trim(),
      },
    ],
  };
}
