import type { RunSummaryDto } from "@/shared/api/discovery";
import {
  coverageLabel,
  formatWhen,
  integrityLabel,
  lifecycleLabel,
} from "@/features/review-workspace/labels";

export interface RunListProps {
  runs: RunSummaryDto[];
  selectedId: string | null;
  category: string;
  query: string;
  openFindingsOnly: boolean;
  hasRisks: boolean;
  hasQuestions: boolean;
  captureCoverage: string;
  projectHasRuns?: boolean;
  hasProject?: boolean;
  onCategory: (c: string) => void;
  onQuery: (q: string) => void;
  onOpenFindingsOnly: (v: boolean) => void;
  onHasRisks: (v: boolean) => void;
  onHasQuestions: (v: boolean) => void;
  onCaptureCoverage: (v: string) => void;
  onResetFilters?: () => void;
  onSelect: (r: RunSummaryDto) => void;
}

const CATEGORIES: { id: string; label: string }[] = [
  { id: "recent", label: "Recent" },
  { id: "active", label: "Active" },
  { id: "ready", label: "Ready for review" },
];

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
    projectHasRuns = true,
    hasProject = true,
    onCategory,
    onQuery,
    onOpenFindingsOnly,
    onHasRisks,
    onHasQuestions,
    onCaptureCoverage,
    onResetFilters,
    onSelect,
  } = props;

  const filtersActive =
    category !== "recent" ||
    !!query ||
    openFindingsOnly ||
    hasRisks ||
    hasQuestions ||
    !!captureCoverage;

  return (
    <aside
      className="flex h-full w-64 shrink-0 flex-col border-r text-xs"
      style={{ background: "var(--panel)", borderColor: "var(--border)" }}
      data-testid="run-list"
    >
      <div className="border-b px-2 py-2 font-semibold" style={{ borderColor: "var(--border)" }}>
        Runs
      </div>
      <div className="grid gap-1 border-b px-2 py-2" style={{ borderColor: "var(--border)" }}>
        <div className="flex flex-wrap gap-1" role="group" aria-label="Run category">
          {CATEGORIES.map((c) => (
            <button
              key={c.id}
              type="button"
              className="rounded px-2 py-0.5"
              style={{
                background: category === c.id ? "var(--accent-soft)" : "var(--bg)",
                color: category === c.id ? "var(--accent)" : "var(--muted)",
                border: "1px solid var(--border)",
              }}
              onClick={() => onCategory(c.id)}
            >
              {c.label}
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
          Open questions
        </label>
        <label className="flex items-center gap-1" style={{ color: "var(--muted)" }}>
          Capture
          <select
            className="rounded border px-1 py-0.5"
            style={{
              background: "var(--bg)",
              borderColor: "var(--border)",
              color: "var(--text)",
            }}
            value={captureCoverage}
            onChange={(e) => onCaptureCoverage(e.target.value)}
            aria-label="Capture observation filter"
          >
            <option value="">Any observation</option>
            <option value="full">Mechanical + semantic observed</option>
            <option value="mechanical_only">Mechanical observed</option>
            <option value="semantic_only">Semantic observed</option>
            <option value="partial">Partial observation</option>
            <option value="unknown">Coverage unknown</option>
          </select>
        </label>
        {filtersActive && onResetFilters ? (
          <button
            type="button"
            className="text-left underline"
            style={{ color: "var(--accent)" }}
            onClick={onResetFilters}
            data-testid="reset-run-filters"
          >
            Reset filters
          </button>
        ) : null}
      </div>
      <div className="moraine-scroll flex-1 overflow-auto p-1">
        {!hasProject ? (
          <p className="px-2 py-3" style={{ color: "var(--muted)" }}>
            Select a project first.
          </p>
        ) : runs.length === 0 ? (
          <div className="px-2 py-3" style={{ color: "var(--muted)" }} data-testid="runs-empty">
            {!projectHasRuns ? (
              <p>This project has no runs.</p>
            ) : (
              <>
                <p>No runs match the current filters.</p>
                {onResetFilters ? (
                  <button
                    type="button"
                    className="mt-2 underline"
                    style={{ color: "var(--accent)" }}
                    onClick={onResetFilters}
                  >
                    Reset filters
                  </button>
                ) : null}
              </>
            )}
          </div>
        ) : (
          <ul className="grid gap-1">
            {runs.map((r) => {
              const selected = r.runId === selectedId;
              const objective = r.objective?.trim() || "(no objective recorded)";
              return (
                <li key={r.runId}>
                  <button
                    type="button"
                    className="w-full rounded border px-2 py-1.5 text-left"
                    style={{
                      borderColor: "var(--border)",
                      background: selected ? "var(--accent-soft)" : "var(--bg)",
                      color: "var(--text)",
                    }}
                    onClick={() => onSelect(r)}
                    data-testid={`run-${r.runId}`}
                  >
                    <div className="font-medium line-clamp-2">{objective}</div>
                    <div style={{ color: "var(--muted)" }}>
                      {lifecycleLabel(r.lifecycle)}
                      {r.provisional ? " · Provisional" : ""} · {coverageLabel(r.captureCoverage)}
                    </div>
                    <div style={{ color: "var(--muted)" }}>
                      {r.checkpointCount} cp · {r.evidenceCount} evidence · {r.openFindingCount} open
                      findings
                      {r.riskCount > 0 ? ` · ${r.riskCount} risks` : ""}
                      {r.openQuestionCount > 0 ? ` · ${r.openQuestionCount} questions` : ""}
                    </div>
                    <div style={{ color: "var(--muted)" }}>
                      Updated {formatWhen(r.updatedAt)} · {integrityLabel(r.integrity)}
                    </div>
                    {r.recoveryRequired ? (
                      <div style={{ color: "#b45309" }}>Recovery required</div>
                    ) : null}
                    {r.error ? (
                      <div className="truncate" style={{ color: "#b45309" }} title={r.error}>
                        {r.error}
                      </div>
                    ) : null}
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </aside>
  );
}
