import {
  acceptanceRecoveryMode,
  dispositionLabel,
  isResolvedView,
  shortHash,
  type CommentRecord,
} from "@/features/editor/comments";

export interface CommentsPanelProps {
  comments: CommentRecord[];
  orphanedIds: string[];
  showResolved: boolean;
  currentDiskHash: string | null;
  recoveryBusy?: boolean;
  onToggleShowResolved: () => void;
  onResolve: (id: string) => void;
  onReopen: (id: string) => void;
  onAccept: (id: string) => void;
  onReject: (id: string) => void;
  onCancelAccept?: (id: string) => void;
  onFinalizeAccept?: (id: string) => void;
  onRefreshRecovery?: (id: string) => void;
  onFocus: (id: string) => void;
  onClose: () => void;
}

function when(iso: string): string {
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}

export function CommentsPanel(props: CommentsPanelProps) {
  const {
    comments,
    orphanedIds,
    showResolved,
    currentDiskHash,
    recoveryBusy = false,
    onToggleShowResolved,
    onResolve,
    onReopen,
    onAccept,
    onReject,
    onCancelAccept,
    onFinalizeAccept,
    onRefreshRecovery,
    onFocus,
    onClose,
  } = props;

  const orphanSet = new Set(orphanedIds);
  const visible = showResolved ? comments : comments.filter((c) => !isResolvedView(c));

  return (
    <aside
      className="flex h-full w-72 shrink-0 flex-col border-l"
      style={{ background: "var(--panel)", borderColor: "var(--border)" }}
    >
      <div
        className="flex items-center justify-between border-b px-3 py-2"
        style={{ borderColor: "var(--border)" }}
      >
        <h2 className="text-sm font-semibold">Review</h2>
        <button type="button" className="icon-btn" onClick={onClose} title="Close">
          ✕
        </button>
      </div>

      <label
        className="flex items-center gap-2 px-3 py-2 text-[11px]"
        style={{ color: "var(--muted)" }}
      >
        <input type="checkbox" checked={showResolved} onChange={onToggleShowResolved} />
        Show resolved
      </label>

      <div className="moraine-scroll flex-1 overflow-auto p-2">
        {visible.length === 0 ? (
          <p className="px-2 text-xs" style={{ color: "var(--muted)" }}>
            Select text in the editor, then use Comment or Suggest. Accept applies a suggestion;
            Reject discards it.
          </p>
        ) : (
          <ul className="space-y-2">
            {visible.map((c) => {
              const isSug = c.kind === "suggestion";
              const orphan = orphanSet.has(c.id);
              const terminal = isResolvedView(c);
              const accepting = isSug && c.disposition === "accepting";
              const mode = acceptanceRecoveryMode(
                c.disposition,
                c.acceptanceBaseHash,
                currentDiskHash,
              );
              return (
                <li
                  key={c.id}
                  className="rounded-lg border p-2 text-xs"
                  style={{
                    borderColor: accepting ? "#f59e0b" : orphan ? "#dc2626" : "var(--border)",
                    opacity: terminal ? 0.65 : 1,
                  }}
                >
                  <button type="button" className="w-full text-left" onClick={() => onFocus(c.id)}>
                    <div
                      className="mb-1 text-[10px] font-semibold uppercase tracking-wide"
                      style={{
                        color: isSug
                          ? c.disposition === "accepted"
                            ? "#16a34a"
                            : c.disposition === "rejected"
                              ? "#dc2626"
                              : accepting
                                ? "#b45309"
                                : "#16a34a"
                          : "var(--accent)",
                      }}
                    >
                      {isSug ? "Suggestion" : "Comment"} · {dispositionLabel(c)}
                      {orphan ? (
                        <span style={{ color: "#dc2626" }}> · quote not found</span>
                      ) : null}
                    </div>
                    <div className="font-medium" style={{ color: "var(--text)" }}>
                      “{c.quote.slice(0, 80)}
                      {c.quote.length > 80 ? "…" : ""}”
                    </div>
                    {isSug ? (
                      <div className="mt-1 whitespace-pre-wrap" style={{ color: "#16a34a" }}>
                        =&gt; {c.body || "(delete)"}
                      </div>
                    ) : (
                      <div className="mt-1 whitespace-pre-wrap">{c.body}</div>
                    )}
                    <div className="mt-1" style={{ color: "var(--muted)" }}>
                      {c.author} · {when(c.createdAt)}
                    </div>
                  </button>
                  <div className="mt-1.5 flex flex-wrap gap-2">
                    {accepting ? (
                      <>
                        <div className="w-full text-[10px]" style={{ color: "#b45309" }}>
                          Incomplete acceptance.
                          {c.acceptanceBaseHash && currentDiskHash
                            ? ` base ${shortHash(c.acceptanceBaseHash)} · disk ${shortHash(currentDiskHash)}`
                            : ""}
                        </div>
                        {mode === "cancel_safe" && onCancelAccept ? (
                          <button
                            type="button"
                            className="link"
                            disabled={recoveryBusy}
                            onClick={() => onCancelAccept(c.id)}
                          >
                            Cancel acceptance
                          </button>
                        ) : mode === "finalize_required" ? (
                          <>
                            <p className="w-full text-[10px]" style={{ color: "var(--muted)" }}>
                              The document changed after acceptance began. Finalize or restore the
                              original revision first.
                            </p>
                            {onFinalizeAccept ? (
                              <button
                                type="button"
                                className="link"
                                disabled={recoveryBusy}
                                onClick={() => onFinalizeAccept(c.id)}
                              >
                                Finalize acceptance
                              </button>
                            ) : null}
                            {onRefreshRecovery ? (
                              <button
                                type="button"
                                className="link"
                                disabled={recoveryBusy}
                                onClick={() => onRefreshRecovery(c.id)}
                              >
                                Refresh status
                              </button>
                            ) : null}
                          </>
                        ) : onRefreshRecovery ? (
                          <button
                            type="button"
                            className="link"
                            disabled={recoveryBusy}
                            onClick={() => onRefreshRecovery(c.id)}
                          >
                            Refresh status
                          </button>
                        ) : null}
                      </>
                    ) : terminal ? (
                      <button type="button" className="link" onClick={() => onReopen(c.id)}>
                        Reopen
                      </button>
                    ) : isSug ? (
                      <>
                        <button type="button" className="link" onClick={() => onAccept(c.id)}>
                          Accept
                        </button>
                        <button type="button" className="link" onClick={() => onReject(c.id)}>
                          Reject
                        </button>
                      </>
                    ) : (
                      <button type="button" className="link" onClick={() => onResolve(c.id)}>
                        Resolve
                      </button>
                    )}
                  </div>
                </li>
              );
            })}
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
        .link {
          border: none;
          background: none;
          padding: 0;
          color: var(--accent);
          font-size: 11px;
          font-weight: 600;
          cursor: pointer;
        }
      `}</style>
    </aside>
  );
}
