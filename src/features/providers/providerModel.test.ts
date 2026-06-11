import { describe, expect, it } from "vitest";
import {
  credentialLabel,
  providerCredentialBlocked,
  providerDispatchBlocked,
  providerTargetLabel,
  replaceProvider,
} from "./providerModel";
import type { ProviderAdapterStatus } from "../../lib/types";

const provider: ProviderAdapterStatus = {
  id: "xai",
  name: "xAI",
  kind: "openai-compatible",
  baseUrl: "https://api.x.ai/v1",
  authMode: "bearer-key",
  credentialStatus: "missing",
  catalogSource: "none",
  verifiedAvailable: false,
  health: {
    name: "xAI",
    endpoint: "https://api.x.ai/v1",
    available: false,
    detail: "Not checked.",
  },
  models: [],
  capabilities: ["models", "chat"],
};

describe("provider model helpers", () => {
  it("uses a clear label for stored credentials", () => {
    expect(credentialLabel("stored")).toBe("Stored (encrypted)");
    expect(credentialLabel("unreadable")).toBe("Unreadable — re-save or import");
    expect(credentialLabel("missing")).toBe("Not configured — add or import key");
  });

  it("uses human credential labels for target provider options", () => {
    expect(providerTargetLabel(provider)).toBe(
      "xAI - Not configured — add or import key",
    );
  });

  it("blocks missing and unreadable cloud credentials", () => {
    expect(providerCredentialBlocked(provider)).toBe(true);
    expect(
      providerCredentialBlocked({
        ...provider,
        credentialStatus: "unreadable",
      }),
    ).toBe(true);
    expect(
      providerCredentialBlocked({
        ...provider,
        credentialStatus: "stored",
      }),
    ).toBe(false);
  });

  it("requires a verified model before dispatch", () => {
    expect(
      providerDispatchBlocked({
        ...provider,
        credentialStatus: "stored",
        models: [{ id: "grok-4", ownedBy: null }],
      }),
    ).toBe(true);
    expect(
      providerDispatchBlocked({
        ...provider,
        credentialStatus: "stored",
        verifiedAvailable: true,
        catalogSource: "live",
        models: [{ id: "grok-4", ownedBy: null }],
      }),
    ).toBe(false);
  });

  it("replaces one provider without reordering the inventory", () => {
    const replacement = {
      ...provider,
      health: { ...provider.health, available: true },
    };

    expect(replaceProvider([provider], replacement)).toEqual([replacement]);
  });
});
