import type { ViewMode } from "@/shared/types";

export interface ToolbarProps {
  title: string;
  path: string | null;
  dirty: boolean;
  saving: boolean;
  viewMode: ViewMode;
  historyOpen: boolean;
  commentsOpen: boolean;
  remotePeers: number;
  isTauri: boolean;
  onOpen: () => void;
  onSave: () => void;
  onComment: () => void;
  onSuggest: () => void;
  onToggleComments: () => void;
  onToggleHistory: () => void;
  onViewMode: (mode: ViewMode) => void;
}

export function Toolbar(props: ToolbarProps) {
  const {
    title,
    path,
    dirty,
    saving,
    viewMode,
    historyOpen,
    commentsOpen,
    remotePeers,
    isTauri,
    onOpen,
    onSave,
    onComment,
    onSuggest,
    onToggleComments,
    onToggleHistory,
    onViewMode,
  } = props;

  return (
    <header
      className="flex h-12 shrink-0 items-center gap-2 border-b px-3"
      style={{ background: "var(--panel)", borderColor: "var(--border)" }}
    >
      <div className="flex min-w-0 flex-1 items-center gap-2">
        <span
          className="flex h-7 w-7 items-center justify-center rounded-md text-sm font-bold text-white"
          style={{ background: "linear-gradient(135deg, #0ea5e9, #6366f1)" }}
          title="Moraine"
        >
          M
        </span>
        <div className="min-w-0">
          <div className="flex items-center gap-1.5 truncate text-sm font-semibold">
            <span className="truncate">{title}</span>
            {dirty ? (
              <span title="Unsaved changes" style={{ color: "var(--accent)" }}>
                ●
              </span>
            ) : null}
            {remotePeers > 0 ? (
              <span
                className="rounded px-1.5 py-0.5 text-[10px] font-medium"
                style={{ background: "var(--accent-soft)", color: "var(--accent)" }}
                title="Remote peers in the review session; host autosave paused"
              >
                live
              </span>
            ) : null}
          </div>
          {path ? (
            <div className="truncate text-[11px]" style={{ color: "var(--muted)" }} title={path}>
              {path}
            </div>
          ) : null}
        </div>
      </div>

      <div className="flex items-center gap-1">
        <div
          className="mr-1 flex rounded-lg border p-0.5 text-xs"
          style={{ borderColor: "var(--border)" }}
          role="group"
          aria-label="View mode"
        >
          {(
            [
              ["edit", "Edit"],
              ["split", "Split"],
              ["preview", "Preview"],
            ] as const
          ).map(([mode, label]) => (
            <button
              key={mode}
              type="button"
              className={`rounded-md px-2.5 py-1 transition ${viewMode === mode ? "font-semibold" : ""}`}
              style={
                viewMode === mode
                  ? { background: "var(--accent-soft)", color: "var(--accent)" }
                  : { color: "var(--muted)" }
              }
              onClick={() => onViewMode(mode)}
            >
              {label}
            </button>
          ))}
        </div>

        <button
          type="button"
          className="btn"
          onClick={onOpen}
          title={isTauri ? "Open Markdown file" : "Open (Tauri only)"}
        >
          Open
        </button>
        <button
          type="button"
          className="btn btn-primary"
          onClick={onSave}
          disabled={saving || (!dirty && isTauri)}
          title={
            remotePeers > 0
              ? "Save to disk (autosave paused while peers present)"
              : "Save"
          }
        >
          {saving ? "Saving…" : "Save"}
        </button>
        <button type="button" className="btn" onClick={onComment} title="Select text, then add a comment">
          Comment
        </button>
        <button
          type="button"
          className="btn"
          onClick={onSuggest}
          title="Select text, then propose a replacement"
        >
          Suggest
        </button>
        <button
          type="button"
          className={`btn ${commentsOpen ? "ring-1" : ""}`}
          onClick={onToggleComments}
          title="Open comments and suggestions"
        >
          Review
        </button>
        <button
          type="button"
          className={`btn ${historyOpen ? "ring-1" : ""}`}
          onClick={onToggleHistory}
          title="Edit history"
        >
          History
        </button>
      </div>
      <style>{`
        .btn {
          border-radius: 0.5rem;
          border: 1px solid var(--border);
          background: var(--panel);
          color: var(--text);
          padding: 0.35rem 0.75rem;
          font-size: 0.8rem;
          font-weight: 500;
          cursor: pointer;
        }
        .btn:hover:not(:disabled) { border-color: var(--accent); }
        .btn:disabled { opacity: 0.5; cursor: not-allowed; }
        .btn-primary {
          background: var(--accent);
          border-color: transparent;
          color: #fff;
        }
      `}</style>
    </header>
  );
}
