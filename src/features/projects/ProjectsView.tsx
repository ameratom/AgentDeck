import { useEffect, useMemo, useState } from "react";
import type { FormEvent } from "react";
import {
  listProjects,
  registerProject,
  removeProject,
  setActiveProject,
} from "../../lib/invoke";
import type {
  ProjectWorkspace,
  ProjectWorkspaceList,
} from "../../lib/types";
import {
  defaultProjectName,
  normalizeProjectPath,
  sortProjects,
  validateProjectPath,
} from "./projectModel";

export function ProjectsView() {
  const [workspaceList, setWorkspaceList] =
    useState<ProjectWorkspaceList | null>(null);
  const [path, setPath] = useState("");
  const [name, setName] = useState("");
  const [pathError, setPathError] = useState<string | null>(null);
  const [busyAction, setBusyAction] = useState<string | null>("load");
  const [status, setStatus] = useState("Loading registered projects.");

  useEffect(() => {
    let cancelled = false;

    async function load(): Promise<void> {
      try {
        const nextList = await listProjects();
        if (!cancelled) {
          setWorkspaceList(nextList);
          setStatus(projectCountStatus(nextList));
        }
      } catch (error) {
        if (!cancelled) {
          setStatus(`Project load failed: ${formatError(error)}`);
        }
      } finally {
        if (!cancelled) {
          setBusyAction(null);
        }
      }
    }

    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  const projects = useMemo(
    () => sortProjects(workspaceList?.projects ?? []),
    [workspaceList],
  );
  const activeProject = projects.find((project) => project.active) ?? null;

  async function addProject(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    const error = validateProjectPath(path);
    setPathError(error);
    if (error) {
      return;
    }

    const normalizedPath = normalizeProjectPath(path);
    const trimmedName = name.trim();
    setBusyAction("register");
    setStatus(`Registering ${trimmedName || defaultProjectName(normalizedPath)}...`);

    try {
      const nextList = await registerProject({
        path: normalizedPath,
        name: trimmedName || null,
      });
      setWorkspaceList(nextList);
      setPath("");
      setName("");
      setStatus("Project registered.");
    } catch (error) {
      setStatus(`Project registration failed: ${formatError(error)}`);
    } finally {
      setBusyAction(null);
    }
  }

  async function activate(project: ProjectWorkspace): Promise<void> {
    setBusyAction(`activate:${project.id}`);
    setStatus(`Activating ${project.name}...`);
    try {
      const nextList = await setActiveProject(project.id);
      setWorkspaceList(nextList);
      setStatus(`${project.name} is now active.`);
    } catch (error) {
      setStatus(`Project activation failed: ${formatError(error)}`);
    } finally {
      setBusyAction(null);
    }
  }

  async function remove(project: ProjectWorkspace): Promise<void> {
    if (
      !window.confirm(
        `Remove "${project.name}" from AgentDeck?\n\nThe folder and its files will not be deleted.`,
      )
    ) {
      return;
    }

    setBusyAction(`remove:${project.id}`);
    setStatus(`Removing ${project.name}...`);
    try {
      const nextList = await removeProject(project.id);
      setWorkspaceList(nextList);
      setStatus(`${project.name} was removed from AgentDeck.`);
    } catch (error) {
      setStatus(`Project removal failed: ${formatError(error)}`);
    } finally {
      setBusyAction(null);
    }
  }

  return (
    <section className="workspace projects-workspace">
      <header>
        <div>
          <p className="eyebrow">Phase 11 / Workspaces</p>
          <h2>Projects</h2>
          <p>
            Register local folders and choose the workspace used by project
            config discovery, chat context, and handoffs. Runtime health
            remains machine-wide, and folder contents stay untouched.
          </p>
        </div>
        <span className="phase-badge">Local only</span>
      </header>

      <div className="project-status" role="status">
        <span className={busyAction ? "pulse indicator" : "indicator"} />
        <span>{status}</span>
        {workspaceList ? (
          <span className="project-loaded-at">
            Updated {workspaceList.loadedAt}
          </span>
        ) : null}
      </div>

      <section className="project-active-panel" aria-labelledby="active-project-heading">
        <div>
          <p className="eyebrow">Current context</p>
          <h3 id="active-project-heading">
            {activeProject?.name ?? "No active project"}
          </h3>
          <p>
            {activeProject
              ? activeProject.path
              : "Register a folder below, then set it as the active project."}
          </p>
        </div>
        <span
          className={
            activeProject?.exists
              ? "project-state active"
              : "project-state missing"
          }
        >
          {activeProject
            ? activeProject.exists
              ? "Active"
              : "Active / Missing"
            : "Not set"}
        </span>
      </section>

      <section className="project-register-panel" aria-labelledby="register-project-heading">
        <div className="section-heading">
          <div>
            <p className="eyebrow">Add workspace</p>
            <h3 id="register-project-heading">Register a Project</h3>
          </div>
          <small>No external configuration changes</small>
        </div>

        <form className="project-form" onSubmit={(event) => void addProject(event)}>
          <label>
            Folder path
            <input
              aria-describedby={pathError ? "project-path-error" : "project-path-hint"}
              aria-invalid={pathError !== null}
              disabled={busyAction !== null}
              onChange={(event) => {
                setPath(event.target.value);
                if (pathError) {
                  setPathError(null);
                }
              }}
              placeholder="/Users/you/Projects/AgentDeck"
              value={path}
            />
            <small id="project-path-hint">Enter an absolute path to a local folder.</small>
            {pathError ? (
              <small className="project-field-error" id="project-path-error">
                {pathError}
              </small>
            ) : null}
          </label>
          <label>
            Display name <span>(optional)</span>
            <input
              disabled={busyAction !== null}
              onChange={(event) => setName(event.target.value)}
              placeholder={path ? defaultProjectName(path) : "Derived from folder name"}
              value={name}
            />
            <small>Used only inside AgentDeck.</small>
          </label>
          <button disabled={busyAction !== null} type="submit">
            {busyAction === "register" ? "Registering..." : "Register project"}
          </button>
        </form>
      </section>

      <section aria-labelledby="registered-projects-heading">
        <div className="section-heading">
          <div>
            <p className="eyebrow">Workspace registry</p>
            <h3 id="registered-projects-heading">Registered Projects</h3>
          </div>
          <small>{projects.length} total</small>
        </div>

        <div className="project-list">
          {projects.length ? (
            projects.map((project) => (
              <article
                className={project.active ? "project-card active" : "project-card"}
                key={project.id}
              >
                <div className="project-card-heading">
                  <div>
                    <h3>{project.name}</h3>
                    <p>{project.path}</p>
                  </div>
                  <span
                    className={
                      project.active
                        ? "project-state active"
                        : project.exists
                          ? "project-state"
                          : "project-state missing"
                    }
                  >
                    {project.active
                      ? "Active"
                      : project.exists
                        ? "Available"
                        : "Folder missing"}
                  </span>
                </div>
                <div className="project-card-actions">
                  <button
                    disabled={busyAction !== null || project.active || !project.exists}
                    onClick={() => void activate(project)}
                    type="button"
                  >
                    {busyAction === `activate:${project.id}`
                      ? "Activating..."
                      : project.active
                        ? "Active project"
                        : "Set active"}
                  </button>
                  <button
                    className="project-remove-button"
                    disabled={busyAction !== null}
                    onClick={() => void remove(project)}
                    type="button"
                  >
                    {busyAction === `remove:${project.id}` ? "Removing..." : "Remove"}
                  </button>
                </div>
              </article>
            ))
          ) : (
            <p className="empty-state">
              {busyAction === "load"
                ? "Loading registered projects..."
                : "No projects registered yet."}
            </p>
          )}
        </div>
      </section>
    </section>
  );
}

function projectCountStatus(list: ProjectWorkspaceList): string {
  const count = list.projects.length;
  return `Loaded ${count} registered project${count === 1 ? "" : "s"}.`;
}

function formatError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
