export interface ReviewNoticeProps {
  title: string;
  body: string;
  tone?: "info" | "warn" | "error";
  actionLabel?: string;
  onAction?: () => void;
  testId?: string;
}

export function ReviewNotice({
  title,
  body,
  tone = "info",
  actionLabel,
  onAction,
  testId = "review-notice",
}: ReviewNoticeProps) {
  const color =
    tone === "error" ? "#b45309" : tone === "warn" ? "#b45309" : "var(--muted)";
  return (
    <div
      className="rounded border px-3 py-2 text-xs"
      style={{ borderColor: "var(--border)", color }}
      data-testid={testId}
      role="status"
    >
      <div className="font-semibold" style={{ color: "var(--text)" }}>
        {title}
      </div>
      <p className="mt-1" style={{ color }}>
        {body}
      </p>
      {actionLabel && onAction ? (
        <button type="button" className="mt-2 underline" onClick={onAction}>
          {actionLabel}
        </button>
      ) : null}
    </div>
  );
}
