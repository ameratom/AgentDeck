import { describe, expect, it } from "vitest";
import {
  createWebhookEndpoint,
  removeWebhookEndpoint,
  toggleWebhookEvent,
  updateWebhookEndpoint,
  webhookDispatchApproval,
} from "./webhookModel";

describe("webhookModel", () => {
  it("creates deterministic webhook ids", () => {
    const endpoint = createWebhookEndpoint();
    expect(endpoint.id.startsWith("webhook:")).toBe(true);
    expect(endpoint.eventTypes).toEqual(["test.ping"]);
  });

  it("updates and removes endpoints", () => {
    const endpoint = createWebhookEndpoint();
    const updated = updateWebhookEndpoint([endpoint], endpoint.id, {
      name: "Ops hook",
    });
    expect(updated[0]?.name).toBe("Ops hook");
    expect(removeWebhookEndpoint(updated, endpoint.id)).toEqual([]);
  });

  it("toggles subscribed events", () => {
    const endpoint = createWebhookEndpoint();
    const withHandoff = toggleWebhookEvent(
      [endpoint],
      endpoint.id,
      "handoff.completed",
      true,
    );
    expect(withHandoff[0]?.eventTypes).toContain("handoff.completed");
    const withoutPing = toggleWebhookEvent(
      withHandoff,
      endpoint.id,
      "test.ping",
      false,
    );
    expect(withoutPing[0]?.eventTypes).not.toContain("test.ping");
  });

  it("builds approval tokens from endpoint names", () => {
    expect(webhookDispatchApproval("Ops hook")).toBe("user-approved:Ops hook");
  });
});