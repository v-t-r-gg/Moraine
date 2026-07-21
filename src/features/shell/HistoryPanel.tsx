import type { HistoryEntryMeta } from "@/shared/types";

export interface HistoryPanelProps {
  entries: HistoryEntryMeta[];
  loading: boolean;
  onRestore: (id: string) => void;
  onRefresh: () => void;
  onClose: () => void;
}

function formatWhen(iso: string): string {
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}

export function HistoryPanel({
  entries,
  loading,
  onRestore,
  onRefresh,
  onClose,
}: HistoryPanelProps) {
  return (
    <aside
      className="flex h-full w-72 shrink-0 flex-col border-l"
      style={{ background: "var(--panel)", borderColor: "var(--border)" }}
    >
      <div
        className="flex items-center justify-between border-b px-3 py-2"
        style={{ borderColor: "var(--border)" }}
      >
        <h2 className="text-sm font-semibold">History</h2>
        <div className="flex gap-1">
          <button type="button" className="icon-btn" onClick={onRefresh} title="Refresh">
            ↻
          </button>
          <button type="button" className="icon-btn" onClick={onClose} title="Close">
            ✕
          </button>
        </div>
      </div>
      <div className="moraine-scroll flex-1 overflow-auto p-2">
        {loading ? (
          <p className="px-2 text-xs" style={{ color: "var(--muted)" }}>
            Loading…
          </p>
        ) : entries.length === 0 ? (
          <p className="px-2 text-xs" style={{ color: "var(--muted)" }}>
            No snapshots yet. Saves create history entries automatically.
          </p>
        ) : (
          <ul className="space-y-1">
            {entries.map((entry) => (
              <li
                key={entry.id}
                className="rounded-lg border p-2 text-xs"
                style={{ borderColor: "var(--border)" }}
              >
                <div className="font-medium">{formatWhen(entry.createdAt)}</div>
                <div style={{ color: "var(--muted)" }}>
                  {entry.source}
                  {entry.label ? ` · ${entry.label}` : ""} · {entry.byteLen} B
                </div>
                <button
                  type="button"
                  className="mt-1.5 text-[11px] font-semibold"
                  style={{ color: "var(--accent)" }}
                  onClick={() => onRestore(entry.id)}
                >
                  Restore
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
      <style>{`
        .icon-btn {
          border: none;
          background: transparent;
          color: var(--muted);
          cursor: pointer;
          padding: 0.2rem 0.4rem;
          border-radius: 0.35rem;
        }
        .icon-btn:hover {
          background: var(--accent-soft);
          color: var(--accent);
        }
      `}</style>
    </aside>
  );
}
