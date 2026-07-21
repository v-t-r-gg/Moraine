import type { RunSummaryDto } from "@/shared/api/discovery";

export interface RunListProps {
  runs: RunSummaryDto[];
  selectedId: string | null;
  category: string;
  query: string;
  openFindingsOnly: boolean;
  hasRisks: boolean;
  hasQuestions: boolean;
  captureCoverage: string;
  onCategory: (c: string) => void;
  onQuery: (q: string) => void;
  onOpenFindingsOnly: (v: boolean) => void;
  onHasRisks: (v: boolean) => void;
  onHasQuestions: (v: boolean) => void;
  onCaptureCoverage: (v: string) => void;
  onSelect: (r: RunSummaryDto) => void;
}

export function RunList(props: RunListProps) {
  const {
    runs,
    selectedId,
    category,
    query,
    openFindingsOnly,
    hasRisks,
    hasQuestions,
    captureCoverage,
    onCategory,
    onQuery,
    onOpenFindingsOnly,
    onHasRisks,
    onHasQuestions,
    onCaptureCoverage,
    onSelect,
  } = props;

  return (
    <aside
      className="flex h-full w-64 shrink-0 flex-col border-r text-xs"
      style={{ background: "var(--panel)", borderColor: "var(--border)" }}
    >
      <div className="border-b px-2 py-2 font-semibold" style={{ borderColor: "var(--border)" }}>
        Runs
      </div>
      <div className="grid gap-1 border-b px-2 py-2" style={{ borderColor: "var(--border)" }}>
        <div className="flex flex-wrap gap-1" role="group" aria-label="Run category">
          {(["recent", "active", "ready"] as const).map((c) => (
            <button
              key={c}
              type="button"
              className="rounded px-2 py-0.5"
              style={{
                background: category === c ? "var(--accent-soft)" : "var(--bg)",
                color: category === c ? "var(--accent)" : "var(--muted)",
                border: "1px solid var(--border)",
              }}
              onClick={() => onCategory(c)}
            >
              {c}
            </button>
          ))}
        </div>
        <input
          className="rounded border px-2 py-1"
          style={{
            background: "var(--bg)",
            borderColor: "var(--border)",
            color: "var(--text)",
          }}
          placeholder="Search objective, id, path"
          value={query}
          onChange={(e) => onQuery(e.target.value)}
          aria-label="Search runs"
        />
        <label className="flex items-center gap-1" style={{ color: "var(--muted)" }}>
          <input
            type="checkbox"
            checked={openFindingsOnly}
            onChange={(e) => onOpenFindingsOnly(e.target.checked)}
          />
          Open findings
        </label>
        <label className="flex items-center gap-1" style={{ color: "var(--muted)" }}>
          <input type="checkbox" checked={hasRisks} onChange={(e) => onHasRisks(e.target.checked)} />
          Has risks
        </label>
        <label className="flex items-center gap-1" style={{ color: "var(--muted)" }}>
          <input
            type="checkbox"
            checked={hasQuestions}
            onChange={(e) => onHasQuestions(e.target.checked)}
          />
          Has questions
        </label>
        <label className="flex items-center gap-1" style={{ color: "var(--muted)" }}>
          Coverage
          <select
            className="rounded border px-1 py-0.5"
            style={{
              background: "var(--bg)",
              borderColor: "var(--border)",
              color: "var(--text)",
            }}
            value={captureCoverage}
            onChange={(e) => onCaptureCoverage(e.target.value)}
            aria-label="Capture coverage filter"
          >
            <option value="">any</option>
            <option value="full">full</option>
            <option value="semantic_only">semantic_only</option>
            <option value="partial">partial</option>
            <option value="unknown">unknown</option>
          </select>
        </label>
      </div>
      <div className="moraine-scroll flex-1 overflow-auto p-1">
        {runs.length === 0 ? (
          <p className="px-2 py-3" style={{ color: "var(--muted)" }}>
            No runs match filters.
          </p>
        ) : (
          <ul className="grid gap-1">
            {runs.map((r) => (
              <li key={r.runId}>
                <button
                  type="button"
                  className="w-full rounded border px-2 py-1.5 text-left"
                  style={{
                    borderColor:
                      r.integrity !== "current" || r.recoveryRequired
                        ? "#b45309"
                        : "var(--border)",
                    background:
                      selectedId === r.runId ? "var(--accent-soft)" : "var(--bg)",
                    color: "var(--text)",
                  }}
                  onClick={() => onSelect(r)}
                >
                  <div className="font-medium line-clamp-2">{r.objective || "(no objective)"}</div>
                  <div style={{ color: "var(--muted)" }}>
                    {r.lifecycle}
                    {r.provisional ? " · provisional" : ""} · {r.captureCoverage}
                  </div>
                  <div style={{ color: "var(--muted)" }}>
                    {r.checkpointCount} cp · {r.evidenceCount} ev · {r.openFindingCount} findings
                    {r.riskCount ? ` · ${r.riskCount} risks` : ""}
                  </div>
                  {r.integrity !== "current" ? (
                    <div style={{ color: "#b45309" }}>{r.integrity}</div>
                  ) : null}
                  {r.error ? (
                    <div className="truncate" style={{ color: "#b45309" }} title={r.error}>
                      {r.error}
                    </div>
                  ) : null}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </aside>
  );
}
