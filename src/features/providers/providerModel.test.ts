import { describe, expect, it } from "vitest";
import {
  credentialLabel,
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
  it("uses a clear label for Keychain credentials", () => {
    expect(credentialLabel("keychain")).toBe("Stored in Keychain");
  });

  it("uses human credential labels for target provider options", () => {
    expect(providerTargetLabel(provider)).toBe("xAI - API key required");
  });

  it("replaces one provider without reordering the inventory", () => {
    const replacement = {
      ...provider,
      health: { ...provider.health, available: true },
    };

    expect(replaceProvider([provider], replacement)).toEqual([replacement]);
  });
});
