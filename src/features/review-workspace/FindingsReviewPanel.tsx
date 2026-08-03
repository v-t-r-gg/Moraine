import { useCallback, useEffect, useMemo, useState } from "react";
import {
  changeFindingState,
  getFinding,
  getRunCheckpoints,
  listFindings,
  type FindingDetailDto,
  type FindingListItemDto,
  type FindingState,
} from "@/shared/api";
import { ReviewNotice } from "./ReviewNotice";
import { findingKindLabel, findingStateLabel, formatWhen } from "./labels";

export interface FindingsReviewPanelProps {
  path: string | null;
  refreshToken: number;
  selectedFindingId: string | null;
  onSelectFinding: (id: string | null) => void;
  onMutated?: () => void;
}

export function FindingsReviewPanel({
  path,
  refreshToken,
  selectedFindingId,
  onSelectFinding,
  onMutated,
}: FindingsReviewPanelProps) {
  const [items, setItems] = useState<FindingListItemDto[]>([]);
  const [thread, setThread] = useState<FindingDetailDto | null>(null);
  const [stateFilter, setStateFilter] = useState<"all" | FindingState>("all");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    if (!path) {
      setItems([]);
      setThread(null);
      return;
    }
    setError(null);
    try {
      // Prefer list via checkpoints payload (includes target context) with fallback.
      try {
        const d = await getRunCheckpoints(path);
        setItems(d.findings);
      } catch {
        const listed = await listFindings(path, false);
        setItems(listed);
      }
    } catch (e) {
      setItems([]);
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [path]);

  useEffect(() => {
    void load();
  }, [load, refreshToken]);

  useEffect(() => {
    if (!path || !selectedFindingId) {
      setThread(null);
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const t = await getFinding(path, selectedFindingId);
        if (!cancelled) setThread(t);
      } catch (e) {
        if (!cancelled) {
          setThread(null);
          setError(e instanceof Error ? e.message : String(e));
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [path, selectedFindingId, refreshToken]);

  const filtered = useMemo(() => {
    if (stateFilter === "all") return items;
    return items.filter((f) => f.state === stateFilter);
  }, [items, stateFilter]);

  async function setState(state: FindingState) {
    if (!path || !selectedFindingId || busy) return;
    setBusy(true);
    setError(null);
    try {
      await changeFindingState(path, selectedFindingId, state);
      await load();
      const t = await getFinding(path, selectedFindingId);
      setThread(t);
      onMutated?.();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  if (!path) {
    return (
      <div className="p-3" data-testid="review-findings">
        <ReviewNotice title="No run selected" body="Select a run to review findings." />
      </div>
    );
  }

  return (
    <div className="grid gap-2 p-3 text-xs md:grid-cols-2" data-testid="review-findings">
      <div>
        <div className="mb-2 flex flex-wrap gap-1" role="group" aria-label="Finding state filter">
          {(
            [
              ["all", "All"],
              ["open", "Open"],
              ["addressed", "Addressed"],
              ["archived", "Archived"],
            ] as const
          ).map(([id, label]) => (
            <button
              key={id}
              type="button"
              className="rounded px-2 py-0.5"
              style={{
                background: stateFilter === id ? "var(--accent-soft)" : "var(--bg)",
                border: "1px solid var(--border)",
              }}
              onClick={() => setStateFilter(id)}
            >
              {label}
            </button>
          ))}
        </div>
        {error && items.length === 0 ? (
          <ReviewNotice tone="error" title="Could not load findings" body={error} />
        ) : null}
        {filtered.length === 0 ? (
          <p style={{ color: "var(--muted)" }}>No findings in this filter.</p>
        ) : (
          <ul className="grid gap-1">
            {filtered.map((f) => {
              const active = f.findingId === selectedFindingId;
              return (
                <li key={f.findingId}>
                  <button
                    type="button"
                    className="w-full rounded border px-2 py-1.5 text-left"
                    style={{
                      borderColor: "var(--border)",
                      background: active ? "var(--accent-soft)" : "var(--bg)",
                    }}
                    onClick={() => onSelectFinding(f.findingId)}
                    data-testid={`finding-item-${f.findingId}`}
                  >
                    <div className="font-medium">
                      {findingKindLabel(String(f.kind))} · {findingStateLabel(String(f.state))}
                    </div>
                    <div className="line-clamp-2" style={{ color: "var(--muted)" }}>
                      {f.body}
                    </div>
                    {f.target?.targetRedacted ? (
                      <div style={{ color: "#b45309" }}>Target checkpoint redacted</div>
                    ) : null}
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>
      <div data-testid="finding-thread">
        {thread ? (
          <>
            <h2 className="font-medium">
              {findingKindLabel(thread.kind)} · {findingStateLabel(thread.state)}
            </h2>
            <p className="mt-1" style={{ color: "var(--muted)" }}>
              Target:{" "}
              {thread.target.targetRedacted
                ? "[REDACTED]"
                : thread.target.checkpointSummary || thread.target.checkpointOpId}
            </p>
            <ul className="mt-2 grid gap-2">
              {thread.thread.map((item) => (
                <li
                  key={item.id}
                  className="rounded border px-2 py-1"
                  style={{ borderColor: "var(--border)" }}
                >
                  <div className="font-medium">
                    {item.itemKind === "response" ? "Agent response" : "Finding"} ·{" "}
                    {formatWhen(item.createdAt)}
                  </div>
                  <div className="whitespace-pre-wrap break-words">{item.body}</div>
                </li>
              ))}
            </ul>
            <div className="mt-3 flex flex-wrap gap-2">
              <button
                type="button"
                className="rounded border px-2 py-1 disabled:opacity-50"
                style={{ borderColor: "var(--border)" }}
                disabled={busy || thread.state === "addressed"}
                onClick={() => void setState("addressed")}
                data-testid="finding-mark-addressed"
              >
                Mark addressed
              </button>
              <button
                type="button"
                className="rounded border px-2 py-1 disabled:opacity-50"
                style={{ borderColor: "var(--border)" }}
                disabled={busy || thread.state === "archived"}
                onClick={() => void setState("archived")}
                data-testid="finding-archive"
              >
                Archive
              </button>
              <button
                type="button"
                className="rounded border px-2 py-1 disabled:opacity-50"
                style={{ borderColor: "var(--border)" }}
                disabled={busy || thread.state === "open"}
                onClick={() => void setState("open")}
                data-testid="finding-reopen"
              >
                Reopen
              </button>
            </div>
            <p className="mt-2 text-[10px]" style={{ color: "var(--muted)" }}>
              State changes are append-only descriptive context. They are not approvals, rejections,
              or merge decisions.
            </p>
          </>
        ) : (
          <p style={{ color: "var(--muted)" }}>Select a finding to read its thread.</p>
        )}
        {error && items.length > 0 ? (
          <p className="mt-2" style={{ color: "#b45309" }}>
            {error}
          </p>
        ) : null}
      </div>
    </div>
  );
}
