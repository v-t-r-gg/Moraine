import type { RunReviewDto } from "@/shared/api/runs";

export type RunReview = RunReviewDto;

export interface RunReviewPanelProps {
  review: RunReview | null;
  externalConflict: boolean;
  onReload?: () => void;
}

function shortHash(h: string): string {
  return h.length > 12 ? `${h.slice(0, 12)}…` : h;
}

function when(iso: string): string {
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}

export function RunReviewPanel({ review, externalConflict, onReload }: RunReviewPanelProps) {
  return (
    <section
      className="border-b px-3 py-2 text-xs"
      style={{ background: "var(--panel)", borderColor: "var(--border)" }}
    >
      <div className="mb-1 flex items-center justify-between gap-2">
        <span className="font-semibold">Run ledger</span>
        {review ? (
          <span
            className="rounded px-1.5 py-0.5 font-medium"
            style={{ background: "var(--accent-soft)", color: "var(--muted)" }}
          >
            inspect · no verdict
          </span>
        ) : null}
      </div>
      {!review ? (
        <p style={{ color: "var(--muted)" }}>Open a file to load run identity.</p>
      ) : (
        <div className="grid gap-0.5" style={{ color: "var(--muted)" }}>
          <div title={review.runId}>
            run{" "}
            <span className="font-mono text-[11px]" style={{ color: "var(--text)" }}>
              {review.runId.slice(0, 8)}…
            </span>
          </div>
          <div title={review.contentHash}>
            rev{" "}
            <span className="font-mono text-[11px]" style={{ color: "var(--text)" }}>
              {shortHash(review.contentHash)}
            </span>
            <span style={{ color: "var(--muted)" }}> (saved revision)</span>
          </div>
          {review.latest && review.decisionCount > 0 ? (
            <div className="mt-1" style={{ color: "var(--muted)" }}>
              Legacy decision history (compatibility):{" "}
              <strong style={{ color: "var(--text)" }}>{review.latest.decision}</strong> by{" "}
              {review.latest.reviewerLabel} · {when(review.latest.createdAt)} ·{" "}
              {review.decisionCount} recorded
              {!review.decisionCurrent ? (
                <>
                  {" "}
                  ·{" "}
                  <span style={{ color: "#b45309", fontWeight: 600 }}>
                    stale vs current revision
                  </span>
                </>
              ) : null}
            </div>
          ) : null}
          {review.latest?.reason ? (
            <div style={{ color: "var(--text)" }}>reason: {review.latest.reason}</div>
          ) : null}
          {externalConflict ? (
            <div className="mt-1 font-medium" style={{ color: "#b45309" }}>
              The file changed on disk.
              {onReload ? (
                <button
                  type="button"
                  className="btn ml-1"
                  onClick={() => onReload()}
                  style={{
                    borderRadius: "0.4rem",
                    border: "1px solid var(--border)",
                    background: "var(--bg)",
                    color: "var(--text)",
                    padding: "0.3rem 0.6rem",
                    fontSize: "0.75rem",
                    fontWeight: 600,
                    cursor: "pointer",
                    marginLeft: "0.25rem",
                  }}
                >
                  Reload from disk
                </button>
              ) : null}
            </div>
          ) : null}
        </div>
      )}
    </section>
  );
}
