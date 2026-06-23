import { describe, expect, it } from "vitest";
import type { ProjectWorkspace } from "../../lib/types";
import {
  defaultProjectName,
  normalizeProjectPath,
  projectFileStateLabel,
  sortProjects,
  validateProjectPath,
} from "./projectModel";

function project(
  id: string,
  name: string,
  active = false,
): ProjectWorkspace {
  return {
    id,
    name,
    description: "",
    path: `/workspace/${id}`,
    exists: true,
    active,
    formatVersion: null,
    projectFileState: "legacy",
    projectFileDigest: null,
    autonomyRestrictions: { askFirst: [], deny: [] },
    createdAt: "2026-06-13T12:00:00Z",
    updatedAt: "2026-06-13T12:00:00Z",
  };
}

describe("project model helpers", () => {
  it("normalizes paths and derives a default name", () => {
    expect(normalizeProjectPath("  /Users/me/AgentDeck/// ")).toBe(
      "/Users/me/AgentDeck",
    );
    expect(defaultProjectName("/Users/me/AgentDeck/")).toBe("AgentDeck");
    expect(defaultProjectName("/")).toBe("Project");
  });

  it("requires an absolute project path", () => {
    expect(validateProjectPath("")).toBe("Enter a project folder path.");
    expect(validateProjectPath("projects/AgentDeck")).toBe(
      "Use an absolute folder path beginning with /.",
    );
    expect(validateProjectPath("/Users/me/AgentDeck")).toBeNull();
  });

  it("sorts the active project first and then by name", () => {
    const sorted = sortProjects([
      project("zulu", "Zulu"),
      project("active", "Current", true),
      project("alpha", "alpha"),
    ]);

    expect(sorted.map(({ id }) => id)).toEqual(["active", "alpha", "zulu"]);
  });

  it("labels project file states for review", () => {
    expect(projectFileStateLabel("legacy")).toBe("Legacy");
    expect(projectFileStateLabel("synced")).toBe("v2 synced");
    expect(projectFileStateLabel("changed")).toBe("Review changes");
    expect(projectFileStateLabel("invalid")).toBe("Invalid file");
    expect(projectFileStateLabel("missing")).toBe("File missing");
  });
});
