import { useCallback, useEffect, useState, type FormEvent } from "react";
import {
  createFinding,
  getRunCheckpoints,
  type FindingKind,
  type FindingListItemDto,
  type RunCheckpointsDetailDto,
} from "@/shared/api";
import { ReviewNotice } from "./ReviewNotice";
import { findingKindLabel, formatWhen } from "./labels";

const KINDS: { value: FindingKind; label: string }[] = [
  { value: "clarification", label: "Clarification" },
  { value: "inconsistency", label: "Inconsistency" },
  { value: "missing_evidence", label: "Missing evidence" },
  { value: "risk_concern", label: "Risk concern" },
  { value: "factual_correction", label: "Factual correction" },
  { value: "other", label: "Other" },
];

export interface CheckpointsReviewPanelProps {
  path: string | null;
  refreshToken: number;
  selectedCheckpointId: string | null;
  onSelectCheckpoint: (id: string | null) => void;
  onMutated?: () => void;
}

export function CheckpointsReviewPanel({
  path,
  refreshToken,
  selectedCheckpointId,
  onSelectCheckpoint,
  onMutated,
}: CheckpointsReviewPanelProps) {
  const [detail, setDetail] = useState<RunCheckpointsDetailDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [formKind, setFormKind] = useState<FindingKind>("clarification");
  const [formBody, setFormBody] = useState("");
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!path) {
      setDetail(null);
      return;
    }
    setError(null);
    try {
      const d = await getRunCheckpoints(path);
      setDetail(d);
      if (selectedCheckpointId && !d.checkpoints.some((c) => c.opId === selectedCheckpointId)) {
        onSelectCheckpoint(d.checkpoints[0]?.opId ?? null);
      } else if (!selectedCheckpointId && d.checkpoints[0]) {
        onSelectCheckpoint(d.checkpoints[0].opId);
      }
    } catch (e) {
      setDetail(null);
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [path, selectedCheckpointId, onSelectCheckpoint]);

  useEffect(() => {
    void load();
  }, [load, refreshToken]);

  function findingsFor(opId: string): FindingListItemDto[] {
    return detail?.findings.filter((f) => f.target.checkpointOpId === opId) ?? [];
  }

  async function submitFinding(e: FormEvent) {
    e.preventDefault();
    if (!path || !selectedCheckpointId || busy) return;
    const body = formBody.trim();
    if (!body) return;
    setBusy(true);
    setError(null);
    try {
      await createFinding(path, selectedCheckpointId, formKind, body);
      setFormBody("");
      setStatus("Finding recorded (descriptive context, not a verdict).");
      await load();
      onMutated?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  if (!path) {
    return (
      <div className="p-3" data-testid="review-checkpoints">
        <ReviewNotice title="No run selected" body="Select a run to inspect checkpoints." />
      </div>
    );
  }

  if (error && !detail) {
    return (
      <div className="p-3" data-testid="review-checkpoints">
        <ReviewNotice tone="error" title="Could not load checkpoints" body={error} />
      </div>
    );
  }

  const selected = detail?.checkpoints.find((c) => c.opId === selectedCheckpointId) ?? null;

  return (
    <div className="grid gap-2 p-3 text-xs md:grid-cols-2" data-testid="review-checkpoints">
      <div>
        <h2 className="mb-2 font-medium">Checkpoints</h2>
        {!detail || detail.checkpoints.length === 0 ? (
          <p style={{ color: "var(--muted)" }}>No checkpoints recorded for this run.</p>
        ) : (
          <ul className="grid gap-1" role="listbox" aria-label="Checkpoint list">
            {detail.checkpoints.map((cp, i) => {
              const active = cp.opId === selectedCheckpointId;
              return (
                <li key={cp.opId}>
                  <button
                    type="button"
                    role="option"
                    aria-selected={active}
                    className="w-full rounded border px-2 py-1.5 text-left"
                    style={{
                      borderColor: "var(--border)",
                      background: active ? "var(--accent-soft)" : "var(--bg)",
                    }}
                    onClick={() => onSelectCheckpoint(cp.opId)}
                    data-testid={`checkpoint-item-${cp.opId}`}
                  >
                    <div className="font-medium">
                      #{i + 1} · {cp.summary || "(empty summary)"}
                    </div>
                    <div style={{ color: "var(--muted)" }}>
                      {formatWhen(cp.createdAt)} · {cp.openFindingCount} open / {cp.findingCount}{" "}
                      findings
                    </div>
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>
      <div>
        {selected ? (
          <div data-testid="checkpoint-detail">
            <h2 className="mb-1 font-medium">Checkpoint detail</h2>
            <p>{selected.summary}</p>
            <p className="mt-1" style={{ color: "var(--muted)" }}>
              {formatWhen(selected.createdAt)}
            </p>
            <div className="mt-2">
              <div className="font-medium">Findings on this checkpoint</div>
              <ul className="mt-1 list-disc pl-4">
                {findingsFor(selected.opId).map((f) => (
                  <li key={f.findingId}>
                    {findingKindLabel(String(f.kind))} · {f.state}: {f.body}
                  </li>
                ))}
                {findingsFor(selected.opId).length === 0 ? (
                  <li style={{ color: "var(--muted)" }}>None yet</li>
                ) : null}
              </ul>
            </div>
            <form className="mt-3 grid gap-2" onSubmit={submitFinding} data-testid="add-finding-form">
              <div className="font-medium">Add descriptive finding</div>
              <select
                value={formKind}
                onChange={(e) => setFormKind(e.target.value as FindingKind)}
                aria-label="Finding kind"
                className="rounded border px-2 py-1"
                style={{ background: "var(--bg)", borderColor: "var(--border)" }}
              >
                {KINDS.map((k) => (
                  <option key={k.value} value={k.value}>
                    {k.label}
                  </option>
                ))}
              </select>
              <textarea
                value={formBody}
                onChange={(e) => setFormBody(e.target.value)}
                rows={3}
                placeholder="Describe the review context (not an approval)"
                className="rounded border px-2 py-1"
                style={{ background: "var(--bg)", borderColor: "var(--border)" }}
                aria-label="Finding body"
              />
              <button
                type="submit"
                disabled={busy || !formBody.trim()}
                className="rounded px-2 py-1 font-semibold disabled:opacity-50"
                style={{ background: "var(--accent)", color: "#fff" }}
              >
                {busy ? "Recording…" : "Record finding"}
              </button>
              {status ? <p style={{ color: "var(--muted)" }}>{status}</p> : null}
              {error ? <p style={{ color: "#b45309" }}>{error}</p> : null}
            </form>
          </div>
        ) : (
          <p style={{ color: "var(--muted)" }}>Select a checkpoint to inspect.</p>
        )}
      </div>
    </div>
  );
}
