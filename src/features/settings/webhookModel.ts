import type { WebhookEndpoint } from "../../lib/types";

export const WEBHOOK_EVENT_OPTIONS = [
  { id: "test.ping", label: "Test ping" },
  { id: "handoff.completed", label: "Handoff completed" },
  { id: "handoff.failed", label: "Handoff failed" },
  { id: "skill.completed", label: "Skill completed" },
] as const;

export function createWebhookEndpoint(): WebhookEndpoint {
  const stamp = Date.now().toString(36);
  return {
    id: `webhook:${stamp}`,
    name: "New webhook",
    url: "https://",
    enabled: true,
    eventTypes: ["test.ping"],
    hasSecret: false,
    updatedAt: new Date().toISOString(),
  };
}

export function removeWebhookEndpoint(
  endpoints: WebhookEndpoint[],
  endpointId: string,
): WebhookEndpoint[] {
  return endpoints.filter((endpoint) => endpoint.id !== endpointId);
}

export function updateWebhookEndpoint(
  endpoints: WebhookEndpoint[],
  endpointId: string,
  patch: Partial<WebhookEndpoint>,
): WebhookEndpoint[] {
  return endpoints.map((endpoint) =>
    endpoint.id === endpointId
      ? { ...endpoint, ...patch, id: endpoint.id, hasSecret: endpoint.hasSecret }
      : endpoint,
  );
}

export function toggleWebhookEvent(
  endpoints: WebhookEndpoint[],
  endpointId: string,
  eventType: string,
  enabled: boolean,
): WebhookEndpoint[] {
  return endpoints.map((endpoint) => {
    if (endpoint.id !== endpointId) {
      return endpoint;
    }
    const next = new Set(endpoint.eventTypes);
    if (enabled) {
      next.add(eventType);
    } else {
      next.delete(eventType);
    }
    return {
      ...endpoint,
      eventTypes: WEBHOOK_EVENT_OPTIONS.map((option) => option.id).filter((id) =>
        next.has(id),
      ),
    };
  });
}

export function webhookDispatchApproval(endpointName: string): string {
  return `user-approved:${endpointName}`;
}