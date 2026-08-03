import { useMemo, useState } from "react";
import type { TimelineEntryDto } from "@/shared/api/discovery";
import { ReviewNotice } from "./ReviewNotice";
import { formatWhen, provenanceLabel } from "./labels";

export interface EvidenceReviewPanelProps {
  timeline: TimelineEntryDto[];
  loading?: boolean;
  error?: string | null;
}

function isEvidenceEntry(e: TimelineEntryDto): boolean {
  const k = e.kind.toLowerCase();
  return (
    k.includes("evidence") ||
    k === "invocation_observed" ||
    k === "result_observed" ||
    k === "agent_reported" ||
    (e.provenance != null && e.provenance.length > 0)
  );
}

export function EvidenceReviewPanel({ timeline, loading, error }: EvidenceReviewPanelProps) {
  const [filter, setFilter] = useState<"all" | "observed" | "agent_reported">("all");

  const evidence = useMemo(() => {
    const rows = timeline.filter(isEvidenceEntry);
    return rows.filter((e) => {
      const p = (e.provenance || e.kind || "").toLowerCase();
      if (filter === "agent_reported") {
        return p.includes("agent");
      }
      if (filter === "observed") {
        return (
          p.includes("invocation") ||
          p.includes("result") ||
          p.includes("moraine") ||
          p.includes("observed") ||
          p.includes("captured")
        );
      }
      return true;
    });
  }, [timeline, filter]);

  if (loading) {
    return (
      <div className="p-3 text-xs" data-testid="review-evidence">
        Loading evidence…
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-3" data-testid="review-evidence">
        <ReviewNotice tone="error" title="Evidence view unavailable" body={error} />
      </div>
    );
  }

  return (
    <div className="p-3 text-xs" data-testid="review-evidence">
      <div className="mb-2 flex flex-wrap items-center gap-2">
        <h2 className="font-medium">Evidence</h2>
        <div className="flex gap-1" role="group" aria-label="Evidence filter">
          {(
            [
              ["all", "All"],
              ["observed", "Observed"],
              ["agent_reported", "Agent-reported"],
            ] as const
          ).map(([id, label]) => (
            <button
              key={id}
              type="button"
              className="rounded px-2 py-0.5"
              style={{
                background: filter === id ? "var(--accent-soft)" : "var(--bg)",
                border: "1px solid var(--border)",
                color: filter === id ? "var(--accent)" : "var(--muted)",
              }}
              onClick={() => setFilter(id)}
            >
              {label}
            </button>
          ))}
        </div>
      </div>
      <p className="mb-2" style={{ color: "var(--muted)" }}>
        Agent-reported evidence is a claim from the agent. Observed or Moraine-captured evidence is
        what Moraine recorded mechanically — these are not interchangeable.
      </p>
      {evidence.length === 0 ? (
        <p style={{ color: "var(--muted)" }}>No evidence entries match this filter.</p>
      ) : (
        <ul className="grid gap-2">
          {evidence.map((e) => {
            const prov = e.provenance || e.kind;
            const agentClaim = (prov || "").toLowerCase().includes("agent");
            return (
              <li
                key={e.id}
                className="rounded border px-2 py-1.5"
                style={{ borderColor: "var(--border)" }}
                data-testid="evidence-item"
              >
                <div className="font-medium break-words">{e.summary}</div>
                <div className="mt-0.5" style={{ color: "var(--muted)" }}>
                  {formatWhen(e.timestamp)} · {provenanceLabel(prov)}
                  {agentClaim ? " · claim (not independently verified)" : ""}
                </div>
                {e.detail ? (
                  <pre
                    className="mt-1 max-h-24 overflow-auto whitespace-pre-wrap break-all text-[10px]"
                    style={{ color: "var(--muted)" }}
                  >
                    {e.detail}
                  </pre>
                ) : null}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
