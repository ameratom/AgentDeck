import { describe, expect, it } from "vitest";
import {
  DEFAULT_PRESENCE_SETTINGS,
  presenceSubSettingsEnabled,
  serviceModeLabel,
  shouldHideOnClose,
  shouldStartHidden,
} from "./presenceModel";

describe("presenceModel", () => {
  it("defaults to menu bar service mode with start hidden", () => {
    expect(DEFAULT_PRESENCE_SETTINGS.menuBarServiceMode).toBe(true);
    expect(DEFAULT_PRESENCE_SETTINGS.startHidden).toBe(true);
    expect(DEFAULT_PRESENCE_SETTINGS.closeHidesToMenuBar).toBe(true);
    expect(DEFAULT_PRESENCE_SETTINGS.launchAtLogin).toBe(false);
  });

  it("requires onboarding before start hidden applies", () => {
    expect(
      shouldStartHidden({
        ...DEFAULT_PRESENCE_SETTINGS,
        onboardingComplete: true,
      }),
    ).toBe(true);
    expect(shouldStartHidden(DEFAULT_PRESENCE_SETTINGS)).toBe(false);
  });

  it("only hides on close in service mode with close-to-tray enabled", () => {
    expect(
      shouldHideOnClose({
        menuBarServiceMode: true,
        closeHidesToMenuBar: true,
      }),
    ).toBe(true);
    expect(
      shouldHideOnClose({
        menuBarServiceMode: false,
        closeHidesToMenuBar: true,
      }),
    ).toBe(false);
    expect(
      shouldHideOnClose({
        menuBarServiceMode: true,
        closeHidesToMenuBar: false,
      }),
    ).toBe(false);
  });

  it("enables presence sub-settings only in service mode", () => {
    expect(presenceSubSettingsEnabled(true)).toBe(true);
    expect(presenceSubSettingsEnabled(false)).toBe(false);
  });

  it("labels service vs application mode", () => {
    expect(serviceModeLabel(true)).toBe("Service");
    expect(serviceModeLabel(false)).toBe("Application");
  });
});
