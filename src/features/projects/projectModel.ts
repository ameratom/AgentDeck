import type { ProjectWorkspace } from "../../lib/types";

export function normalizeProjectPath(path: string): string {
  const trimmed = path.trim();
  if (trimmed === "/") {
    return trimmed;
  }
  return trimmed.replace(/\/+$/, "");
}

export function validateProjectPath(path: string): string | null {
  const normalized = normalizeProjectPath(path);
  if (!normalized) {
    return "Enter a project folder path.";
  }
  if (!normalized.startsWith("/")) {
    return "Use an absolute folder path beginning with /.";
  }
  return null;
}

export function defaultProjectName(path: string): string {
  const normalized = normalizeProjectPath(path);
  if (!normalized || normalized === "/") {
    return "Project";
  }
  return normalized.split("/").at(-1) || "Project";
}

export function sortProjects(
  projects: ProjectWorkspace[],
): ProjectWorkspace[] {
  return [...projects].sort((left, right) => {
    if (left.active !== right.active) {
      return left.active ? -1 : 1;
    }
    return left.name.localeCompare(right.name, undefined, {
      sensitivity: "base",
    });
  });
}
