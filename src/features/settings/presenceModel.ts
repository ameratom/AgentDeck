import type { AppSettings } from "../../lib/types";

export type PresenceSettings = Pick<
  AppSettings,
  | "menuBarServiceMode"
  | "startHidden"
  | "closeHidesToMenuBar"
  | "onboardingComplete"
  | "launchAtLogin"
>;

export const DEFAULT_PRESENCE_SETTINGS: PresenceSettings = {
  menuBarServiceMode: true,
  startHidden: true,
  closeHidesToMenuBar: true,
  onboardingComplete: false,
  launchAtLogin: false,
};

export function presenceSubSettingsEnabled(menuBarServiceMode: boolean): boolean {
  return menuBarServiceMode;
}

export function shouldStartHidden(settings: PresenceSettings): boolean {
  return (
    settings.menuBarServiceMode &&
    settings.startHidden &&
    settings.onboardingComplete
  );
}

export function shouldHideOnClose(
  settings: Pick<PresenceSettings, "menuBarServiceMode" | "closeHidesToMenuBar">,
): boolean {
  return settings.menuBarServiceMode && settings.closeHidesToMenuBar;
}

export function serviceModeLabel(menuBarServiceMode: boolean): string {
  return menuBarServiceMode ? "Service" : "Application";
}
