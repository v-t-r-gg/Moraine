import { useCallback, useEffect, useState, type FormEvent } from "react";
import {
  changeFindingState,
  createFinding,
  getFinding,
  getRunCheckpoints,
  type FindingDetailDto,
  type FindingKind,
  type FindingListItemDto,
  type RunCheckpointsDetailDto,
} from "@/shared/api";

export interface CheckpointFindingsPanelProps {
  path: string | null;
  refreshToken?: number;
}

const KINDS: { value: FindingKind; label: string }[] = [
  { value: "clarification", label: "Clarification" },
  { value: "inconsistency", label: "Inconsistency" },
  { value: "missing_evidence", label: "Missing evidence" },
  { value: "risk_concern", label: "Risk concern" },
  { value: "factual_correction", label: "Factual correction" },
  { value: "other", label: "Other" },
];

function when(iso: string): string {
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}

export function CheckpointFindingsPanel({
  path,
  refreshToken = 0,
}: CheckpointFindingsPanelProps) {
  const [detail, setDetail] = useState<RunCheckpointsDetailDto | null>(null);
  const [selectedOpId, setSelectedOpId] = useState<string | null>(null);
  const [selectedFindingId, setSelectedFindingId] = useState<string | null>(null);
  const [thread, setThread] = useState<FindingDetailDto | null>(null);
  const [formKind, setFormKind] = useState<FindingKind>("clarification");
  const [formBody, setFormBody] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  const loadThread = useCallback(
    async (findingId: string) => {
      if (!path) return;
      try {
        const t = await getFinding(path, findingId);
        setThread(t);
        setSelectedFindingId(findingId);
      } catch (e) {
        setThread(null);
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [path],
  );

  const load = useCallback(async () => {
    if (!path) {
      setDetail(null);
      setSelectedOpId(null);
      setSelectedFindingId(null);
      setThread(null);
      return;
    }
    setError(null);
    try {
      const d = await getRunCheckpoints(path);
      setDetail(d);
      setSelectedOpId((prev) => {
        if (prev && d.checkpoints.some((c) => c.opId === prev)) return prev;
        return d.checkpoints[0]?.opId ?? null;
      });
      setSelectedFindingId((fid) => {
        if (fid) void loadThread(fid);
        return fid;
      });
    } catch (e) {
      setDetail(null);
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [path, loadThread]);

  useEffect(() => {
    void load();
  }, [load, refreshToken]);

  function findingsForCheckpoint(opId: string): FindingListItemDto[] {
    if (!detail) return [];
    return detail.findings.filter((f) => f.target.checkpointOpId === opId);
  }

  async function submitFinding(e: FormEvent) {
    e.preventDefault();
    if (!path || !selectedOpId) return;
    const body = formBody.trim();
    if (!body) {
      setError("Finding body is required.");
      return;
    }
    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      const result = await createFinding(path, selectedOpId, formKind, body);
      setFormBody("");
      setStatus("Finding created.");
      await load();
      await loadThread(result.findingId);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  async function markState(state: "open" | "addressed" | "archived") {
    if (!path || !selectedFindingId) return;
    setBusy(true);
    setError(null);
    try {
      await changeFindingState(path, selectedFindingId, state);
      setStatus(`Marked ${state}.`);
      await load();
      await loadThread(selectedFindingId);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  const related = selectedOpId ? findingsForCheckpoint(selectedOpId) : [];

  return (
    <section
      className="border-b px-3 py-2 text-xs"
      style={{ background: "var(--panel)", borderColor: "var(--border)" }}
    >
      <div className="mb-1 flex items-center justify-between gap-2">
        <span className="font-semibold">Checkpoint findings</span>
        <span
          className="rounded px-1.5 py-0.5 font-medium"
          style={{ background: "var(--accent-soft)", color: "var(--muted)" }}
        >
          review context · no verdict
        </span>
      </div>

      {!path ? (
        <p style={{ color: "var(--muted)" }}>
          Open a run record to attach findings to checkpoints.
        </p>
      ) : error && !detail ? (
        <p style={{ color: "#b45309" }}>{error}</p>
      ) : !detail || detail.checkpoints.length === 0 ? (
        <p style={{ color: "var(--muted)" }}>No structured checkpoints on this run yet.</p>
      ) : (
        <div className="grid gap-2">
          <div>
            <label
              className="mb-0.5 block font-medium"
              style={{ color: "var(--muted)" }}
              htmlFor="cp-select"
            >
              Checkpoint
            </label>
            <select
              id="cp-select"
              className="w-full rounded border px-2 py-1"
              style={{
                background: "var(--bg)",
                borderColor: "var(--border)",
                color: "var(--text)",
              }}
              value={selectedOpId ?? ""}
              onChange={(ev) => {
                setSelectedOpId(ev.target.value || null);
                setSelectedFindingId(null);
                setThread(null);
              }}
            >
              {detail.checkpoints.map((cp) => (
                <option key={cp.opId} value={cp.opId}>
                  {cp.summary.slice(0, 80)}
                  {cp.summary.length > 80 ? "…" : ""} ({cp.openFindingCount} open /{" "}
                  {cp.findingCount})
                </option>
              ))}
            </select>
          </div>

          {selectedOpId ? (
            <>
              <div>
                <div className="mb-0.5 font-medium" style={{ color: "var(--muted)" }}>
                  Findings on checkpoint
                </div>
                {related.length === 0 ? (
                  <p style={{ color: "var(--muted)" }}>None yet.</p>
                ) : (
                  <ul className="grid gap-1">
                    {related.map((f) => (
                      <li key={f.findingId}>
                        <button
                          type="button"
                          className="w-full rounded border px-2 py-1 text-left"
                          style={{
                            background:
                              selectedFindingId === f.findingId
                                ? "var(--accent-soft)"
                                : "var(--bg)",
                            borderColor: "var(--border)",
                            color: "var(--text)",
                          }}
                          onClick={() => void loadThread(f.findingId)}
                        >
                          <span className="font-medium">{f.kind}</span>
                          {" · "}
                          <span style={{ color: "var(--muted)" }}>{f.state}</span>
                          {" · "}
                          {f.responseCount} response{f.responseCount === 1 ? "" : "s"}
                          <div className="truncate" style={{ color: "var(--muted)" }}>
                            {f.body}
                          </div>
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
              </div>

              <form
                className="grid gap-1 rounded border p-2"
                style={{ borderColor: "var(--border)" }}
                onSubmit={(ev) => void submitFinding(ev)}
              >
                <div className="font-medium">Add finding</div>
                <label className="grid gap-0.5">
                  <span style={{ color: "var(--muted)" }}>Kind</span>
                  <select
                    className="rounded border px-2 py-1"
                    style={{
                      background: "var(--bg)",
                      borderColor: "var(--border)",
                      color: "var(--text)",
                    }}
                    value={formKind}
                    onChange={(ev) => setFormKind(ev.target.value as FindingKind)}
                  >
                    {KINDS.map((k) => (
                      <option key={k.value} value={k.value}>
                        {k.label}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="grid gap-0.5">
                  <span style={{ color: "var(--muted)" }}>Body</span>
                  <textarea
                    className="min-h-[3.5rem] w-full rounded border px-2 py-1"
                    style={{
                      background: "var(--bg)",
                      borderColor: "var(--border)",
                      color: "var(--text)",
                    }}
                    value={formBody}
                    onChange={(ev) => setFormBody(ev.target.value)}
                    placeholder="Descriptive review context only"
                    maxLength={4000}
                  />
                </label>
                <button type="submit" className="btn self-start" disabled={busy}>
                  {busy ? "Saving…" : "Create finding"}
                </button>
              </form>

              {thread ? (
                <div className="rounded border p-2" style={{ borderColor: "var(--border)" }}>
                  <div className="mb-1 flex flex-wrap items-center justify-between gap-1">
                    <span className="font-medium">Finding thread</span>
                    <span style={{ color: "var(--muted)" }}>
                      {thread.kind} · {thread.state}
                    </span>
                  </div>
                  <div className="mb-1 grid gap-1">
                    {thread.thread.map((item) => (
                      <div
                        key={item.id}
                        className="rounded px-2 py-1"
                        style={{
                          background: "var(--bg)",
                          border: "1px solid var(--border)",
                        }}
                      >
                        <div
                          className="mb-0.5 flex justify-between gap-2"
                          style={{ color: "var(--muted)" }}
                        >
                          <span>
                            {item.itemKind === "finding" ? "Human finding" : "Agent response"}
                            {item.findingKind ? ` · ${item.findingKind}` : ""}
                          </span>
                          <span>{when(item.createdAt)}</span>
                        </div>
                        <div style={{ color: "var(--text)", whiteSpace: "pre-wrap" }}>
                          {item.body}
                        </div>
                      </div>
                    ))}
                  </div>
                  <div className="flex flex-wrap gap-1">
                    {thread.state !== "addressed" ? (
                      <button
                        type="button"
                        className="btn"
                        disabled={busy}
                        onClick={() => void markState("addressed")}
                      >
                        Mark addressed
                      </button>
                    ) : null}
                    {thread.state !== "archived" ? (
                      <button
                        type="button"
                        className="btn"
                        disabled={busy}
                        onClick={() => void markState("archived")}
                      >
                        Archive
                      </button>
                    ) : null}
                    {thread.state !== "open" ? (
                      <button
                        type="button"
                        className="btn"
                        disabled={busy}
                        onClick={() => void markState("open")}
                      >
                        Reopen
                      </button>
                    ) : null}
                  </div>
                </div>
              ) : null}
            </>
          ) : null}

          {status ? <p style={{ color: "var(--muted)" }}>{status}</p> : null}
          {error ? <p style={{ color: "#b45309" }}>{error}</p> : null}
        </div>
      )}
      <style>{`
        .btn {
          border-radius: 0.4rem;
          border: 1px solid var(--border);
          background: var(--bg);
          color: var(--text);
          padding: 0.3rem 0.6rem;
          font-size: 0.75rem;
          cursor: pointer;
        }
        .btn:disabled { opacity: 0.55; cursor: not-allowed; }
      `}</style>
    </section>
  );
}
