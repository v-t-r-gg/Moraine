import type { TimelineEntryDto } from "@/shared/api/discovery";

export interface LedgerTimelineProps {
  entries: TimelineEntryDto[];
}

function when(iso: string): string {
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}

export function LedgerTimeline({ entries }: LedgerTimelineProps) {
  if (entries.length === 0) {
    return (
      <p className="text-xs" style={{ color: "var(--muted)" }}>
        No timeline events.
      </p>
    );
  }
  return (
    <ol className="grid gap-2 text-xs">
      {entries.map((e) => (
        <li
          key={e.id}
          className="rounded border px-2 py-1.5"
          style={{ borderColor: "var(--border)", background: "var(--bg)" }}
        >
          <div className="flex flex-wrap justify-between gap-1" style={{ color: "var(--muted)" }}>
            <span className="font-medium" style={{ color: "var(--accent)" }}>
              {e.kind}
            </span>
            <span>{when(e.timestamp)}</span>
          </div>
          <div style={{ color: "var(--text)", whiteSpace: "pre-wrap" }}>{e.summary}</div>
          {e.actorCategory ? (
            <div style={{ color: "var(--muted)" }}>actor: {e.actorCategory}</div>
          ) : null}
          {e.detail ? (
            <details className="mt-1">
              <summary style={{ color: "var(--muted)", cursor: "pointer" }}>Details</summary>
              <pre
                className="mt-1 whitespace-pre-wrap font-mono text-[10px]"
                style={{ color: "var(--text)" }}
              >
                {e.detail}
              </pre>
            </details>
          ) : null}
        </li>
      ))}
    </ol>
  );
}
