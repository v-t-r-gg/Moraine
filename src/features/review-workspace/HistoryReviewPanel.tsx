import { useState } from "react";
import type { TimelineEntryDto } from "@/shared/api/discovery";
import { ReviewNotice } from "./ReviewNotice";
import { formatWhen } from "./labels";

export interface HistoryReviewPanelProps {
  timeline: TimelineEntryDto[];
  loading?: boolean;
  error?: string | null;
}

export function HistoryReviewPanel({ timeline, loading, error }: HistoryReviewPanelProps) {
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});

  if (loading) {
    return (
      <div className="p-3 text-xs" data-testid="review-history">
        Loading history…
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-3" data-testid="review-history">
        <ReviewNotice tone="error" title="History unavailable" body={error} />
      </div>
    );
  }

  return (
    <div className="p-3 text-xs" data-testid="review-history">
      <h2 className="mb-2 font-medium">Append-only history</h2>
      <p className="mb-2" style={{ color: "var(--muted)" }}>
        Current interpretation is shown first. Historical changes remain accessible; originals are
        never rewritten in place.
      </p>
      {timeline.length === 0 ? (
        <p style={{ color: "var(--muted)" }}>No history entries yet.</p>
      ) : (
        <ol className="grid gap-2">
          {timeline.map((e) => {
            const open = !!expanded[e.id];
            return (
              <li
                key={e.id}
                className="rounded border px-2 py-1.5"
                style={{ borderColor: "var(--border)" }}
                data-testid="history-item"
              >
                <div className="flex flex-wrap items-start justify-between gap-2">
                  <div>
                    <div className="font-medium">
                      {e.kind}
                      {e.actorCategory ? ` · ${e.actorCategory}` : ""}
                    </div>
                    <div style={{ color: "var(--muted)" }}>{formatWhen(e.timestamp)}</div>
                    <div className="mt-0.5 break-words">{e.summary}</div>
                  </div>
                  <button
                    type="button"
                    className="underline shrink-0"
                    onClick={() => setExpanded((m) => ({ ...m, [e.id]: !open }))}
                    data-testid={`history-expand-${e.id}`}
                  >
                    {open ? "Hide technical detail" : "Technical detail"}
                  </button>
                </div>
                {open ? (
                  <dl
                    className="mt-2 grid gap-1 font-mono text-[10px]"
                    style={{ color: "var(--muted)" }}
                    data-testid={`history-detail-${e.id}`}
                  >
                    <div>
                      <dt className="inline font-semibold">id: </dt>
                      <dd className="inline break-all">{e.id}</dd>
                    </div>
                    {e.targetId ? (
                      <div>
                        <dt className="inline font-semibold">target: </dt>
                        <dd className="inline break-all">{e.targetId}</dd>
                      </div>
                    ) : null}
                    {e.provenance ? (
                      <div>
                        <dt className="inline font-semibold">provenance: </dt>
                        <dd className="inline">{e.provenance}</dd>
                      </div>
                    ) : null}
                    {e.detail ? (
                      <div>
                        <dt className="font-semibold">detail</dt>
                        <dd className="whitespace-pre-wrap break-all">{e.detail}</dd>
                      </div>
                    ) : null}
                  </dl>
                ) : null}
              </li>
            );
          })}
        </ol>
      )}
    </div>
  );
}
