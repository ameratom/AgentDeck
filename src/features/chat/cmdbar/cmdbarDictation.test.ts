import { describe, expect, it } from "vitest";
import { formatDictationError } from "./useCmdBarDictation";

describe("formatDictationError", () => {
  it("maps common speech recognition failures", () => {
    expect(formatDictationError("not-allowed")).toContain("Microphone access denied");
    expect(formatDictationError("no-speech")).toContain("No speech detected");
    expect(formatDictationError("audio-capture")).toContain("No microphone available");
  });

  it("falls back for unknown errors", () => {
    expect(formatDictationError("custom-error")).toBe(
      "Dictation failed (custom-error).",
    );
    expect(formatDictationError(undefined)).toBe("Dictation failed. Try again.");
  });
});