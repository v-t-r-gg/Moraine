export interface StatusBarProps {
  wordCount: number;
  charCount: number;
  collabPeers: number;
  peerNames: string;
  roomId: string | null;
  autosavePaused: boolean;
  pendingComments: number;
  pendingSuggestions: number;
  orphanedMarks: number;
  message: string | null;
}

export function StatusBar({
  wordCount,
  charCount,
  collabPeers,
  peerNames,
  roomId,
  autosavePaused,
  pendingComments,
  pendingSuggestions,
  orphanedMarks,
  message,
}: StatusBarProps) {
  return (
    <footer
      className="flex h-7 shrink-0 items-center gap-3 border-t px-3 text-[11px]"
      style={{
        background: "var(--panel)",
        borderColor: "var(--border)",
        color: "var(--muted)",
      }}
    >
      <span>{wordCount} words</span>
      <span>{charCount} chars</span>
      {roomId ? <span title="Yjs room">room:{roomId}</span> : null}
      {collabPeers > 0 ? (
        <span title={peerNames || undefined}>
          You + {collabPeers} other{collabPeers === 1 ? "" : "s"}
          {peerNames ? ` (${peerNames})` : ""}
        </span>
      ) : null}
      {pendingSuggestions > 0 ? (
        <span style={{ color: "#16a34a", fontWeight: 600 }}>
          {pendingSuggestions} suggestion{pendingSuggestions === 1 ? "" : "s"} pending
        </span>
      ) : null}
      {pendingComments > 0 ? (
        <span style={{ color: "#d97706" }}>
          {pendingComments} comment{pendingComments === 1 ? "" : "s"}
        </span>
      ) : null}
      {orphanedMarks > 0 ? (
        <span title="Quoted text no longer found in the document" style={{ color: "#dc2626" }}>
          {orphanedMarks} mark{orphanedMarks === 1 ? "" : "s"} missing
        </span>
      ) : null}
      {autosavePaused ? (
        <span style={{ color: "#f59e0b", fontWeight: 600 }}>Live collab: autosave paused</span>
      ) : null}
      <span className="ml-auto truncate">{message ?? "Ready"}</span>
    </footer>
  );
}
