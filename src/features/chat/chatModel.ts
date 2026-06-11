import type {
  ChatMessage,
  ChatMessageInput,
  ChatRequest,
  LocalModel,
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

export function toChatHistory(messages: ChatMessage[]): ChatMessageInput[] {
  return messages.map((message) => ({
    role: message.role,
    content: message.content,
  }));
}

export function selectDefaultProvider(
  providers: { id: string }[],
  preferredId?: string,
): string {
  if (preferredId && providers.some((provider) => provider.id === preferredId)) {
    return preferredId;
  }
  return (
    providers.find((provider) => provider.id === "lm-studio")?.id ??
    providers[0]?.id ??
    ""
  );
}

export function buildChatRequest(
  conversationId: string,
  providerId: string,
  model: string,
  messages: ChatMessage[],
  nextContent: string,
  enableAgentTools = false,
): ChatRequest {
  return {
    conversationId,
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
