import {
  REVIEW_SECTIONS,
  type ReviewSection,
} from "./labels";

export interface RunReviewNavigationProps {
  section: ReviewSection;
  onSection: (s: ReviewSection) => void;
  counts?: Partial<Record<ReviewSection, string>>;
}

export function RunReviewNavigation({
  section,
  onSection,
  counts,
}: RunReviewNavigationProps) {
  return (
    <nav
      className="flex flex-wrap gap-1 border-b px-3 py-2"
      style={{ borderColor: "var(--border)" }}
      aria-label="Review sections"
      data-testid="review-nav"
      role="tablist"
    >
      {REVIEW_SECTIONS.map((s) => {
        const selected = section === s.id;
        const count = counts?.[s.id];
        return (
          <button
            key={s.id}
            type="button"
            role="tab"
            aria-selected={selected}
            id={`review-tab-${s.id}`}
            className="rounded px-2 py-1 text-xs font-medium focus:outline focus:outline-2"
            style={{
              background: selected ? "var(--accent-soft)" : "var(--bg)",
              color: selected ? "var(--accent)" : "var(--muted)",
              border: "1px solid var(--border)",
            }}
            onClick={() => onSection(s.id)}
            data-testid={`review-tab-${s.id}`}
          >
            {s.label}
            {count ? <span className="ml-1 opacity-80">{count}</span> : null}
          </button>
        );
      })}
    </nav>
  );
}
