import { useCallback, useEffect, useMemo, useState } from "react";
import type { FormEvent } from "react";
import {
  applyProjectFile,
  listProjects,
  previewProjectFile,
  registerProject,
  removeProject,
  saveProjectFile,
  saveProjectRestrictions,
  setActiveProject,
} from "../../lib/invoke";
import type {
  ProjectFilePreview,
  ProjectWorkspace,
  ProjectWorkspaceList,
} from "../../lib/types";
import {
  defaultProjectName,
  normalizeProjectPath,
  projectFileStateLabel,
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
  const [preview, setPreview] = useState<ProjectFilePreview | null>(null);
  const [restrictionDrafts, setRestrictionDrafts] = useState<
    Record<string, { askFirst: string; deny: string }>
  >({});
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(
    null,
  );
  const [registerOpen, setRegisterOpen] = useState(false);

  async function saveRestrictions(project: ProjectWorkspace): Promise<void> {
    const draft = restrictionDrafts[project.id] ?? {
      askFirst: project.autonomyRestrictions.askFirst.join(", "),
      deny: project.autonomyRestrictions.deny.join(", "),
    };
    const split = (value: string) =>
      [...new Set(value.split(",").map((item) => item.trim()).filter(Boolean))];
    setBusyAction("restrictions:" + project.id);
    try {
      const next = await saveProjectRestrictions(project.id, {
        askFirst: split(draft.askFirst),
        deny: split(draft.deny),
      });
      setWorkspaceList(next);
      setStatus("Restriction-only project policy saved.");
    } catch (error) {
      setStatus("Project policy save failed: " + formatError(error));
    } finally {
      setBusyAction(null);
    }
  }

  const refreshProjects = useCallback(async (showStatus = true): Promise<void> => {
    try {
      const nextList = await listProjects();
      setWorkspaceList(nextList);
      if (showStatus) {
        setStatus(projectCountStatus(nextList));
      }
    } catch (error) {
      setStatus(`Project load failed: ${formatError(error)}`);
    } finally {
      setBusyAction(null);
    }
  }, []);

  useEffect(() => {
    const initialLoad = window.setTimeout(() => void refreshProjects(), 0);
    const handleFocus = () => void refreshProjects(false);
    window.addEventListener("focus", handleFocus);
    return () => {
      window.clearTimeout(initialLoad);
      window.removeEventListener("focus", handleFocus);
    };
  }, [refreshProjects]);

  const projects = useMemo(
    () => sortProjects(workspaceList?.projects ?? []),
    [workspaceList],
  );
  const activeProject = projects.find((project) => project.active) ?? null;
  const effectiveSelectedId = useMemo(() => {
    if (!projects.length) {
      return null;
    }
    if (
      selectedProjectId &&
      projects.some((project) => project.id === selectedProjectId)
    ) {
      return selectedProjectId;
    }
    return activeProject?.id ?? projects[0].id;
  }, [projects, activeProject, selectedProjectId]);
  const selectedProject =
    projects.find((project) => project.id === effectiveSelectedId) ?? null;

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
      const registered =
        nextList.projects.find(
          (project) => project.path === normalizedPath,
        ) ?? null;
      if (registered) {
        setSelectedProjectId(registered.id);
      }
      setPath("");
      setName("");
      setRegisterOpen(false);
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

  async function saveFormat(project: ProjectWorkspace): Promise<void> {
    setBusyAction(`save:${project.id}`);
    setPreview(null);
    setStatus(`Saving ${project.name} as Project Format v2...`);
    try {
      const nextList = await saveProjectFile(project.id);
      setWorkspaceList(nextList);
      setStatus(`Saved ${project.name} to .agentdeck/project.json.`);
    } catch (error) {
      setStatus(`Project file save failed: ${formatError(error)}`);
    } finally {
      setBusyAction(null);
    }
  }

  async function reviewFormat(project: ProjectWorkspace): Promise<void> {
    setBusyAction(`preview:${project.id}`);
    setPreview(null);
    setStatus(`Reviewing ${project.name} project file...`);
    try {
      const nextPreview = await previewProjectFile(project.id);
      setPreview(nextPreview);
      setStatus(
        nextPreview.valid
          ? `Project file review ready with ${nextPreview.changes.length} change${nextPreview.changes.length === 1 ? "" : "s"}.`
          : `Project file needs attention: ${nextPreview.error ?? "invalid file"}`,
      );
    } catch (error) {
      setStatus(`Project file preview failed: ${formatError(error)}`);
    } finally {
      setBusyAction(null);
    }
  }

  async function applyFormat(project: ProjectWorkspace): Promise<void> {
    if (!preview?.fileDigest || preview.projectId !== project.id) {
      return;
    }
    setBusyAction(`apply:${project.id}`);
    setStatus(`Applying reviewed changes for ${project.name}...`);
    try {
      const nextList = await applyProjectFile(project.id, preview.fileDigest);
      setWorkspaceList(nextList);
      setPreview(null);
      setStatus(`Applied Project Format v2 changes for ${project.name}.`);
    } catch (error) {
      setStatus(`Project file apply failed: ${formatError(error)}`);
      await reviewFormat(project);
    } finally {
      setBusyAction(null);
    }
  }

  return (
    <section className="workspace projects-workspace projects-workspace--compact">
      <header className="pr-compact-header">
        <div>
          <p className="eyebrow">Phase 11 / Workspaces</p>
          <h2>Projects</h2>
          <p className="pr-compact-subtitle">
            Register local folders and choose the workspace used by project
            config discovery, chat context, and handoffs. Runtime health
            remains machine-wide, and folder contents stay untouched.
          </p>
        </div>
        <div className="pr-compact-header-meta">
          <button
            className="pr-register-toggle"
            disabled={busyAction !== null}
            onClick={() => setRegisterOpen((open) => !open)}
            type="button"
          >
            {registerOpen ? "Close register" : "Register project"}
          </button>
          <span className="pr-pill">Local only</span>
          <div className="pr-summary">
            <div className="pr-scan-state" role="status">
              <span
                aria-hidden="true"
                className={busyAction ? "pulse indicator" : "indicator"}
              />
              <span>{status}</span>
            </div>
            {workspaceList ? (
              <>
                <span className="pr-pill">
                  <b>{projects.length}</b> registered
                </span>
                <span className="pr-pill">
                  Updated {workspaceList.loadedAt}
                </span>
              </>
            ) : null}
          </div>
        </div>
      </header>

      <div className="pr-body">
        {registerOpen ? (
          <section
            className="project-register-panel pr-register-popover"
            aria-labelledby="register-project-heading"
          >
            <div className="section-heading">
              <div>
                <p className="eyebrow">Add workspace</p>
                <h3 id="register-project-heading">Register a Project</h3>
              </div>
              <small>No external configuration changes</small>
            </div>

            <form
              className="project-form"
              onSubmit={(event) => void addProject(event)}
            >
              <label>
                Folder path
                <input
                  aria-describedby={
                    pathError ? "project-path-error" : "project-path-hint"
                  }
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
                <small id="project-path-hint">
                  Enter an absolute path to a local folder.
                </small>
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
                  placeholder={
                    path ? defaultProjectName(path) : "Derived from folder name"
                  }
                  value={name}
                />
                <small>Used only inside AgentDeck.</small>
              </label>
              <button disabled={busyAction !== null} type="submit">
                {busyAction === "register" ? "Registering..." : "Register project"}
              </button>
            </form>
          </section>
        ) : null}

        <div className="pr-master-detail">
          <section
            aria-labelledby="registered-projects-heading"
            className="pr-registry"
          >
            <div className="section-heading">
              <div>
                <p className="eyebrow">Workspace registry</p>
                <h3 id="registered-projects-heading">Registered Projects</h3>
              </div>
              <small>{projects.length} total</small>
            </div>

            <div className="pr-registry-list" role="list">
              {projects.length ? (
                projects.map((project) => (
                  <button
                    aria-pressed={project.id === effectiveSelectedId}
                    className={
                      project.id === effectiveSelectedId
                        ? "pr-registry-item selected"
                        : project.active
                          ? "pr-registry-item active"
                          : "pr-registry-item"
                    }
                    key={project.id}
                    onClick={() => setSelectedProjectId(project.id)}
                    role="listitem"
                    type="button"
                  >
                    <span
                      aria-hidden="true"
                      className={
                        project.active
                          ? "compact-status-dot active"
                          : project.exists
                            ? "compact-status-dot"
                            : "compact-status-dot missing"
                      }
                    />
                    <span className="pr-registry-copy">
                      <span className="pr-registry-name">{project.name}</span>
                      <span className="pr-registry-path">{project.path}</span>
                    </span>
                    <span className="pr-registry-badges">
                      {project.active ? (
                        <span className="pr-registry-marker">Active</span>
                      ) : null}
                      <span
                        className={`project-format-state ${project.projectFileState}`}
                      >
                        {projectFileStateLabel(project.projectFileState)}
                      </span>
                    </span>
                  </button>
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

          <section
            aria-labelledby="project-detail-heading"
            className="pr-detail-pane"
          >
            {selectedProject ? (
              <article className="project-card">
                <div className="project-card-heading">
                  <div>
                    <p className="eyebrow">Selected workspace</p>
                    <h3 id="project-detail-heading">{selectedProject.name}</h3>
                    <p>{selectedProject.path}</p>
                    {selectedProject.description ? (
                      <p className="project-description">
                        {selectedProject.description}
                      </p>
                    ) : null}
                    {selectedProject.autonomyRestrictions.askFirst.length ||
                    selectedProject.autonomyRestrictions.deny.length ? (
                      <span className="project-state restricted">
                        Custom restrictions
                      </span>
                    ) : null}
                  </div>
                  <div className="project-card-states">
                    <span
                      className={`project-format-state ${selectedProject.projectFileState}`}
                    >
                      {projectFileStateLabel(selectedProject.projectFileState)}
                    </span>
                    <span
                      className={
                        selectedProject.active
                          ? "project-state active"
                          : selectedProject.exists
                            ? "project-state"
                            : "project-state missing"
                      }
                    >
                      {selectedProject.active
                        ? "Active"
                        : selectedProject.exists
                          ? "Available"
                          : "Folder missing"}
                    </span>
                  </div>
                </div>
                <div className="project-card-actions">
                  <button
                    disabled={
                      busyAction !== null ||
                      selectedProject.active ||
                      !selectedProject.exists
                    }
                    onClick={() => void activate(selectedProject)}
                    type="button"
                  >
                    {busyAction === `activate:${selectedProject.id}`
                      ? "Activating..."
                      : selectedProject.active
                        ? "Active project"
                        : "Set active"}
                  </button>
                  {selectedProject.projectFileState !== "changed" &&
                  selectedProject.projectFileState !== "invalid" ? (
                    <button
                      disabled={busyAction !== null || !selectedProject.exists}
                      onClick={() => void saveFormat(selectedProject)}
                      type="button"
                    >
                      {busyAction === `save:${selectedProject.id}`
                        ? "Saving..."
                        : selectedProject.projectFileState === "legacy"
                          ? "Save as v3"
                          : "Save project file"}
                    </button>
                  ) : null}
                  {selectedProject.projectFileState !== "legacy" ? (
                    <button
                      disabled={busyAction !== null || !selectedProject.exists}
                      onClick={() => void reviewFormat(selectedProject)}
                      type="button"
                    >
                      {busyAction === `preview:${selectedProject.id}`
                        ? "Reviewing..."
                        : "Review file"}
                    </button>
                  ) : null}
                  <button
                    className="project-remove-button"
                    disabled={busyAction !== null}
                    onClick={() => void remove(selectedProject)}
                    type="button"
                  >
                    {busyAction === `remove:${selectedProject.id}`
                      ? "Removing..."
                      : "Remove"}
                  </button>
                </div>
                <section className="project-restrictions">
                  <strong>Autonomy restrictions</strong>
                  <p>
                    Project files may require approval or deny actions. They cannot
                    grant authority.
                  </p>
                  <div>
                    <label>
                      Ask first
                      <input
                        onChange={(event) =>
                          setRestrictionDrafts((current) => ({
                            ...current,
                            [selectedProject.id]: {
                              askFirst: event.target.value,
                              deny:
                                current[selectedProject.id]?.deny ??
                                selectedProject.autonomyRestrictions.deny.join(
                                  ", ",
                                ),
                            },
                          }))
                        }
                        placeholder="run_shell, modify_git"
                        value={
                          restrictionDrafts[selectedProject.id]?.askFirst ??
                          selectedProject.autonomyRestrictions.askFirst.join(
                            ", ",
                          )
                        }
                      />
                    </label>
                    <label>
                      Deny
                      <input
                        onChange={(event) =>
                          setRestrictionDrafts((current) => ({
                            ...current,
                            [selectedProject.id]: {
                              askFirst:
                                current[selectedProject.id]?.askFirst ??
                                selectedProject.autonomyRestrictions.askFirst.join(
                                  ", ",
                                ),
                              deny: event.target.value,
                            },
                          }))
                        }
                        placeholder="network, dependency_change"
                        value={
                          restrictionDrafts[selectedProject.id]?.deny ??
                          selectedProject.autonomyRestrictions.deny.join(", ")
                        }
                      />
                    </label>
                    <button
                      disabled={busyAction !== null}
                      onClick={() => void saveRestrictions(selectedProject)}
                      type="button"
                    >
                      {busyAction === "restrictions:" + selectedProject.id
                        ? "Saving..."
                        : "Save restrictions"}
                    </button>
                  </div>
                </section>
                {preview?.projectId === selectedProject.id ? (
                  <details
                    aria-label="Project file review"
                    className="st-expandable-row pr-file-review"
                    open={
                      Boolean(preview.error) ||
                      !preview.valid ||
                      preview.changes.length > 0
                    }
                  >
                    <summary className="st-expandable-summary pr-file-review-summary">
                      <span
                        aria-hidden="true"
                        className={
                          preview.error || !preview.valid
                            ? "compact-status-dot missing"
                            : preview.changes.length > 0
                              ? "compact-status-dot pending"
                              : "compact-status-dot active"
                        }
                      />
                      <span className="st-expandable-title">
                        Project Format v{preview.detectedFormat ?? "?"}
                      </span>
                      <span className="st-expandable-meta">
                        {preview.changes.length
                          ? `${preview.changes.length} change${preview.changes.length === 1 ? "" : "s"}`
                          : preview.error
                            ? "Review error"
                            : "Synced"}
                      </span>
                      <button
                        className="secondary-button pr-file-review-close"
                        onClick={(event) => {
                          event.preventDefault();
                          event.stopPropagation();
                          setPreview(null);
                        }}
                        type="button"
                      >
                        Close
                      </button>
                    </summary>
                    <div className="st-expandable-body project-file-preview">
                      <small className="pr-file-review-path">{preview.path}</small>
                      {preview.error ? (
                        <p className="project-file-error">{preview.error}</p>
                      ) : null}
                      {preview.warnings.map((warning) => (
                        <p className="project-file-warning" key={warning}>
                          {warning}
                        </p>
                      ))}
                      {preview.valid && preview.changes.length === 0 ? (
                        <p className="project-file-empty">
                          The project file matches local configuration.
                        </p>
                      ) : null}
                      {preview.changes.length ? (
                        <div className="project-file-changes">
                          {preview.changes.map((change) => (
                            <div key={change.field}>
                              <strong>{change.field}</strong>
                              <span>{change.currentValue || "(empty)"}</span>
                              <span>{change.fileValue || "(empty)"}</span>
                            </div>
                          ))}
                        </div>
                      ) : null}
                      {preview.valid && preview.changes.length > 0 ? (
                        <button
                          disabled={
                            busyAction !== null ||
                            !preview.canApply ||
                            !preview.fileDigest
                          }
                          onClick={() => void applyFormat(selectedProject)}
                          type="button"
                        >
                          {busyAction === `apply:${selectedProject.id}`
                            ? "Applying..."
                            : "Apply reviewed changes"}
                        </button>
                      ) : null}
                    </div>
                  </details>
                ) : null}
              </article>
            ) : (
              <p className="empty-state">
                Select a registered project to review restrictions and file
                state.
              </p>
            )}
          </section>
        </div>
      </div>
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