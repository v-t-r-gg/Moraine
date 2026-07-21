import { useCallback, useEffect, useMemo, useState, type FormEvent } from "react";
import {
  getRunCheckpoints,
  humanObservationAdd,
  listAppendOps,
  runAmend,
  type AppendOnlyOpDto,
  type RunCheckpointsDetailDto,
} from "@/shared/api";

export interface ProtocolLedgerPanelProps {
  path: string | null;
  refreshToken?: number;
  onMutated?: () => void;
}

function when(iso: string): string {
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}

/** Pure helper: latest claim text after amend/supersede/redact chain. */
export function currentClaim(
  cpOpId: string,
  original: string,
  ops: AppendOnlyOpDto[],
): string {
  let cur = original;
  for (const op of ops) {
    if (op.targetId !== cpOpId || op.targetKind !== "checkpoint") continue;
    if (op.relationship === "amended" || op.relationship === "superseded") {
      if (op.newContent) cur = op.newContent;
    } else if (op.relationship === "redacted") {
      cur = "[REDACTED]";
    }
  }
  return cur;
}

/**
 * Structured read-only ledger for protocol runs: timeline of checkpoints,
 * observations, amendments. Canonical content is not free-form editable.
 */
export function ProtocolLedgerPanel({
  path,
  refreshToken = 0,
  onMutated,
}: ProtocolLedgerPanelProps) {
  const [detail, setDetail] = useState<RunCheckpointsDetailDto | null>(null);
  const [ops, setOps] = useState<AppendOnlyOpDto[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [obsBody, setObsBody] = useState("");
  const [obsReason, setObsReason] = useState("review observation");
  const [obsTarget, setObsTarget] = useState<string>("");
  const [amendTarget, setAmendTarget] = useState<string>("");
  const [amendReason, setAmendReason] = useState("");
  const [amendContent, setAmendContent] = useState("");

  const load = useCallback(async () => {
    if (!path) {
      setDetail(null);
      setOps([]);
      return;
    }
    setError(null);
    try {
      const [d, o] = await Promise.all([getRunCheckpoints(path), listAppendOps(path)]);
      setDetail(d);
      setOps(o);
      setObsTarget((prev) => prev || d.checkpoints[0]?.opId || "");
      setAmendTarget((prev) => prev || d.checkpoints[0]?.opId || "");
    } catch (e) {
      setDetail(null);
      setOps([]);
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [path]);

  useEffect(() => {
    void load();
  }, [load, refreshToken]);

  const observations = useMemo(
    () => ops.filter((o) => o.opKind === "human_observation_add"),
    [ops],
  );

  async function submitObservation(e: FormEvent) {
    e.preventDefault();
    if (!path) return;
    const body = obsBody.trim();
    if (!body) {
      setError("Observation body is required.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await humanObservationAdd(
        path,
        body,
        obsReason.trim() || "review observation",
        obsTarget || null,
        obsTarget ? "checkpoint" : null,
      );
      setObsBody("");
      setStatus("Observation recorded (append-only).");
      await load();
      onMutated?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  async function submitAmend(e: FormEvent) {
    e.preventDefault();
    if (!path || !amendTarget) return;
    setBusy(true);
    setError(null);
    try {
      await runAmend(
        path,
        amendTarget,
        "checkpoint",
        amendReason.trim() || "amendment",
        amendContent.trim(),
        "human",
      );
      setAmendReason("");
      setAmendContent("");
      setStatus("Amendment recorded; original claim preserved.");
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
      <section className="border-b px-3 py-2 text-xs" style={{ background: "var(--panel)" }}>
        <p style={{ color: "var(--muted)" }}>Open a protocol run to view the ledger timeline.</p>
      </section>
    );
  }

  return (
    <section
      className="border-b px-3 py-2 text-xs"
      style={{ background: "var(--panel)", borderColor: "var(--border)" }}
    >
      <div className="mb-1 flex items-center justify-between gap-2">
        <span className="font-semibold">Protocol ledger</span>
        <span
          className="rounded px-1.5 py-0.5 font-medium"
          style={{ background: "var(--accent-soft)", color: "var(--muted)" }}
        >
          append-only · read-only claims
        </span>
      </div>

      {error ? <p style={{ color: "#b45309" }}>{error}</p> : null}
      {status ? <p style={{ color: "var(--muted)" }}>{status}</p> : null}

      {!detail ? (
        <p style={{ color: "var(--muted)" }}>Loading ledger…</p>
      ) : (
        <div className="grid gap-3">
          <div>
            <div className="mb-1 font-medium" style={{ color: "var(--muted)" }}>
              Checkpoint timeline
            </div>
            {detail.checkpoints.length === 0 ? (
              <p style={{ color: "var(--muted)" }}>No checkpoints yet.</p>
            ) : (
              <ul className="grid gap-2">
                {detail.checkpoints.map((cp) => {
                  const related = ops.filter(
                    (o) => o.targetId === cp.opId && o.targetKind === "checkpoint",
                  );
                  const current = currentClaim(cp.opId, cp.summary, ops);
                  const hasAmend = related.some(
                    (o) =>
                      o.relationship === "amended" ||
                      o.relationship === "superseded" ||
                      o.relationship === "redacted",
                  );
                  return (
                    <li
                      key={cp.opId}
                      className="rounded border p-2"
                      style={{ borderColor: "var(--border)", background: "var(--bg)" }}
                    >
                      <div className="font-medium" style={{ color: "var(--text)" }}>
                        {when(cp.createdAt)}
                      </div>
                      {hasAmend ? (
                        <div className="mt-1 grid gap-1">
                          <div
                            data-testid="original-claim"
                            style={{ color: "var(--text)", whiteSpace: "pre-wrap" }}
                          >
                            Original claim: {cp.summary}
                          </div>
                          {related
                            .filter((o) => o.relationship !== "observation")
                            .map((o) => (
                              <div
                                key={o.opId}
                                className="rounded px-2 py-1"
                                style={{ border: "1px solid var(--border)" }}
                              >
                                <div style={{ color: "var(--muted)" }}>
                                  {o.relationship} · {o.actorCategory} · {when(o.createdAt)}
                                </div>
                                <div style={{ color: "var(--text)" }}>{o.reason}</div>
                                {o.newContent ? (
                                  <div style={{ whiteSpace: "pre-wrap" }}>{o.newContent}</div>
                                ) : null}
                                {o.relationship === "redacted" && o.previousContent ? (
                                  <div style={{ color: "var(--muted)", fontSize: "10px" }}>
                                    Prior content recoverable in ledger (not erased).
                                  </div>
                                ) : null}
                              </div>
                            ))}
                          <div
                            data-testid="current-statement"
                            style={{ color: "var(--text)", whiteSpace: "pre-wrap" }}
                          >
                            Current statement: {current}
                          </div>
                        </div>
                      ) : (
                        <div className="mt-0.5" style={{ color: "var(--text)", whiteSpace: "pre-wrap" }}>
                          {cp.summary}
                        </div>
                      )}
                      <div className="mt-0.5 font-mono text-[10px]" style={{ color: "var(--muted)" }}>
                        {cp.opId.slice(0, 8)}…
                      </div>
                    </li>
                  );
                })}
              </ul>
            )}
          </div>

          <div>
            <div className="mb-1 font-medium" style={{ color: "var(--muted)" }}>
              Observations
            </div>
            {observations.length === 0 ? (
              <p style={{ color: "var(--muted)" }}>None yet.</p>
            ) : (
              <ul className="grid gap-1">
                {observations.map((o) => (
                  <li
                    key={o.opId}
                    className="rounded border px-2 py-1"
                    style={{ borderColor: "var(--border)" }}
                  >
                    <div style={{ color: "var(--muted)" }}>{when(o.createdAt)}</div>
                    <div style={{ whiteSpace: "pre-wrap", color: "var(--text)" }}>
                      {o.newContent}
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </div>

          <form
            className="grid gap-1 rounded border p-2"
            style={{ borderColor: "var(--border)" }}
            onSubmit={(ev) => void submitObservation(ev)}
          >
            <div className="font-medium">Add observation</div>
            <p style={{ color: "var(--muted)" }}>
              Append-only human note. Does not edit checkpoints or free-form Human notes as the
              durable write path.
            </p>
            {detail.checkpoints.length > 0 ? (
              <label className="grid gap-0.5">
                <span style={{ color: "var(--muted)" }}>Optional checkpoint target</span>
                <select
                  className="rounded border px-2 py-1"
                  style={{
                    background: "var(--bg)",
                    borderColor: "var(--border)",
                    color: "var(--text)",
                  }}
                  value={obsTarget}
                  onChange={(e) => setObsTarget(e.target.value)}
                >
                  <option value="">(none)</option>
                  {detail.checkpoints.map((cp) => (
                    <option key={cp.opId} value={cp.opId}>
                      {cp.summary.slice(0, 60)}
                    </option>
                  ))}
                </select>
              </label>
            ) : null}
            <label className="grid gap-0.5">
              <span style={{ color: "var(--muted)" }}>Body</span>
              <textarea
                className="min-h-[3rem] w-full rounded border px-2 py-1"
                style={{
                  background: "var(--bg)",
                  borderColor: "var(--border)",
                  color: "var(--text)",
                }}
                value={obsBody}
                onChange={(e) => setObsBody(e.target.value)}
                placeholder="Human observation (append-only)"
                maxLength={4000}
              />
            </label>
            <label className="grid gap-0.5">
              <span style={{ color: "var(--muted)" }}>Reason</span>
              <input
                className="rounded border px-2 py-1"
                style={{
                  background: "var(--bg)",
                  borderColor: "var(--border)",
                  color: "var(--text)",
                }}
                value={obsReason}
                onChange={(e) => setObsReason(e.target.value)}
              />
            </label>
            <button type="submit" className="btn self-start" disabled={busy}>
              {busy ? "Saving…" : "Add observation"}
            </button>
          </form>

          {detail.checkpoints.length > 0 ? (
            <form
              className="grid gap-1 rounded border p-2"
              style={{ borderColor: "var(--border)" }}
              onSubmit={(ev) => void submitAmend(ev)}
            >
              <div className="font-medium">Amend checkpoint claim</div>
              <p style={{ color: "var(--muted)" }}>
                Records an amendment; the original claim stays immutable in the ledger.
              </p>
              <label className="grid gap-0.5">
                <span style={{ color: "var(--muted)" }}>Checkpoint</span>
                <select
                  className="rounded border px-2 py-1"
                  style={{
                    background: "var(--bg)",
                    borderColor: "var(--border)",
                    color: "var(--text)",
                  }}
                  value={amendTarget}
                  onChange={(e) => setAmendTarget(e.target.value)}
                >
                  {detail.checkpoints.map((cp) => (
                    <option key={cp.opId} value={cp.opId}>
                      {cp.summary.slice(0, 60)}
                    </option>
                  ))}
                </select>
              </label>
              <label className="grid gap-0.5">
                <span style={{ color: "var(--muted)" }}>Reason</span>
                <input
                  className="rounded border px-2 py-1"
                  style={{
                    background: "var(--bg)",
                    borderColor: "var(--border)",
                    color: "var(--text)",
                  }}
                  value={amendReason}
                  onChange={(e) => setAmendReason(e.target.value)}
                  placeholder="Why the original claim is incomplete"
                />
              </label>
              <label className="grid gap-0.5">
                <span style={{ color: "var(--muted)" }}>Current statement</span>
                <textarea
                  className="min-h-[3rem] w-full rounded border px-2 py-1"
                  style={{
                    background: "var(--bg)",
                    borderColor: "var(--border)",
                    color: "var(--text)",
                  }}
                  value={amendContent}
                  onChange={(e) => setAmendContent(e.target.value)}
                  placeholder="Corrected claim text"
                />
              </label>
              <button type="submit" className="btn self-start" disabled={busy}>
                Record amendment
              </button>
            </form>
          ) : null}
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
