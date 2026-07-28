import type { ProjectSummaryDto } from "@/shared/api/discovery";

export interface ProjectListProps {
  projects: ProjectSummaryDto[];
  selectedId: string | null;
  onSelect: (p: ProjectSummaryDto) => void;
  onRescan: (p: ProjectSummaryDto) => void;
  onAdd: () => void;
  onRebuild: () => void;
  offline: boolean;
}

export function ProjectList({
  projects,
  selectedId,
  onSelect,
  onRescan,
  onAdd,
  onRebuild,
  offline,
}: ProjectListProps) {
  return (
    <aside
      className="flex h-full w-56 shrink-0 flex-col border-r text-xs"
      style={{ background: "var(--panel)", borderColor: "var(--border)" }}
    >
      <div
        className="flex items-center justify-between border-b px-2 py-2"
        style={{ borderColor: "var(--border)" }}
      >
        <span className="font-semibold">Projects</span>
        <div className="flex gap-1">
          <button type="button" className="link" onClick={onRebuild} title="Rebuild discovery index">
            Rebuild
          </button>
          <button type="button" className="link" onClick={onAdd} title="Add existing project">
            Add
          </button>
        </div>
      </div>
      {offline ? (
        <div className="px-2 py-1 text-[10px]" style={{ color: "#b45309" }}>
          Service offline · direct scan
        </div>
      ) : null}
      <div className="moraine-scroll flex-1 overflow-auto p-1">
        {projects.length === 0 ? (
          <p className="px-2 py-3" style={{ color: "var(--muted)" }}>
            No projects yet. Use <strong>Enable Moraine</strong> to connect a folder, or Add to
            select one.
          </p>
        ) : (
          <ul className="grid gap-1">
            {projects.map((p) => (
              <li key={p.projectId + p.rootPath}>
                <button
                  type="button"
                  className="w-full rounded border px-2 py-1.5 text-left"
                  style={{
                    borderColor: "var(--border)",
                    background:
                      selectedId === p.projectId ? "var(--accent-soft)" : "var(--bg)",
                    color: "var(--text)",
                    opacity: p.available ? 1 : 0.7,
                  }}
                  onClick={() => onSelect(p)}
                >
                  <div className="font-medium truncate">{p.name}</div>
                  <div style={{ color: "var(--muted)" }}>
                    {p.runCounts.active} active · {p.runCounts.ready} ready ·{" "}
                    {p.openFindingCount} findings
                  </div>
                  {!p.available ? (
                    <div style={{ color: "#b45309" }}>unavailable</div>
                  ) : null}
                  {p.warning ? (
                    <div className="truncate" style={{ color: "#b45309" }} title={p.warning}>
                      {p.warning}
                    </div>
                  ) : null}
                </button>
                <button
                  type="button"
                  className="link ml-1 mt-0.5"
                  onClick={() => onRescan(p)}
                >
                  Rescan
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
      <style>{`
        .link {
          border: none;
          background: none;
          color: var(--accent);
          font-size: 10px;
          font-weight: 600;
          cursor: pointer;
          padding: 0.1rem 0.25rem;
        }
      `}</style>
    </aside>
  );
}
