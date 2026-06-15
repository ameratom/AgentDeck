import { useMemo, useState } from "react";
import type { PluginDefinition, SkillDefinition } from "../../../lib/types";
import { requiredPluginNames } from "../pluginModel";
import {
  filterSkills,
  type SkillFilter,
} from "../registryTableModel";

const FILTER_CHIPS: { id: SkillFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "ready", label: "Ready" },
];

type SkillTableProps = {
  skills: SkillDefinition[];
  plugins: PluginDefinition[];
  busyId: string | null;
  onRun: (skillId: string) => void;
  onRowClick: (skillId: string) => void;
};

export function SkillTable({
  skills,
  plugins,
  busyId,
  onRun,
  onRowClick,
}: SkillTableProps) {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<SkillFilter>("all");

  const filteredSkills = useMemo(
    () => filterSkills(skills, plugins, query, filter),
    [skills, plugins, query, filter],
  );

  function handleRowKeyDown(
    event: React.KeyboardEvent<HTMLDivElement>,
    skillId: string,
  ): void {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onRowClick(skillId);
    }
  }

  return (
    <section
      aria-label="Skill library"
      className="reg-pane skills"
    >
      <div className="reg-pane-head">
        <div>
          <p className="reg-pane-eyebrow">Reusable workflows</p>
          <h3>Skill Library</h3>
        </div>
        <span className="reg-pane-meta">Execution writes an audit record</span>
      </div>

      <div className="reg-pane-toolbar">
        <div className="reg-filters" role="group" aria-label="Skill filters">
          {FILTER_CHIPS.map((chip) => (
            <button
              aria-pressed={filter === chip.id}
              className={
                filter === chip.id ? "reg-chip active" : "reg-chip"
              }
              key={chip.id}
              onClick={() => setFilter(chip.id)}
              type="button"
            >
              {chip.label}
            </button>
          ))}
        </div>
        <label className="reg-search">
          <span className="sr-only">Search skills</span>
          <svg aria-hidden viewBox="0 0 24 24">
            <path
              d="M10.5 18a7.5 7.5 0 1 1 0-15 7.5 7.5 0 0 1 0 15Zm5.2-1.3 4.3 4.3"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            />
          </svg>
          <input
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search..."
            type="search"
            value={query}
          />
        </label>
      </div>

      <div className="reg-thead" role="row">
        <span aria-hidden />
        <span>Skill</span>
        <span>Tags</span>
        <span>Required plugins</span>
        <span>Run</span>
      </div>

      <div className="reg-tbody">
        {filteredSkills.length ? (
          filteredSkills.map((skill) => {
            const reqNames = requiredPluginNames(skill, plugins);
            return (
              <div
                className={`reg-trow ${skill.available ? "" : "off"}`}
                key={skill.id}
                onClick={() => onRowClick(skill.id)}
                onKeyDown={(event) => handleRowKeyDown(event, skill.id)}
                role="button"
                tabIndex={0}
              >
                <div className="reg-cell c-status">
                  <span
                    className={`reg-sdot ${skill.available ? "on" : "off"}`}
                  />
                </div>
                <div className="reg-cell c-name" title={skill.name}>
                  {skill.name}
                </div>
                <div className="reg-cell">
                  <div className="caps">
                    {skill.tags.map((tag) => (
                      <span className="cap" key={tag}>
                        {tag}
                      </span>
                    ))}
                  </div>
                </div>
                <div className="reg-cell">
                  <span className="reqp" title={reqNames.join(", ")}>
                    {reqNames.join(" · ")}
                  </span>
                </div>
                <div className="reg-cell c-action">
                  <button
                    className="reg-runbtn"
                    disabled={busyId !== null || !skill.available}
                    onClick={(event) => {
                      event.stopPropagation();
                      onRun(skill.id);
                    }}
                    type="button"
                  >
                    <svg aria-hidden viewBox="0 0 24 24">
                      <path d="M8 5v14l11-7z" fill="currentColor" />
                    </svg>
                    {busyId === skill.id ? "…" : "Run"}
                  </button>
                </div>
              </div>
            );
          })
        ) : (
          <div className="reg-empty">
            <h3>No matching skills</h3>
            <p>Adjust search or filters to see skill definitions.</p>
          </div>
        )}
      </div>
    </section>
  );
}