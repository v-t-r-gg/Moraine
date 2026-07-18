/** Pure helpers: when run-level decisions may be recorded. */

export type DecisionGateReason =
  | "ok"
  | "unsaved"
  | "external_conflict"
  | "saving"
  | "no_review"
  | "no_hash";

export function canRecordDecision(opts: {
  dirty: boolean;
  externalConflict: boolean;
  saving: boolean;
  hasReview: boolean;
  hasPersistedHash: boolean;
}): { allowed: boolean; reason: DecisionGateReason } {
  if (!opts.hasReview) return { allowed: false, reason: "no_review" };
  if (opts.saving) return { allowed: false, reason: "saving" };
  if (opts.dirty) return { allowed: false, reason: "unsaved" };
  if (opts.externalConflict) return { allowed: false, reason: "external_conflict" };
  if (!opts.hasPersistedHash) return { allowed: false, reason: "no_hash" };
  return { allowed: true, reason: "ok" };
}

export function decisionGateMessage(reason: DecisionGateReason): string | null {
  switch (reason) {
    case "unsaved":
      return "Save the current revision before recording a review decision.";
    case "external_conflict":
      return "The file changed on disk. Reload or resolve the conflict before recording a decision.";
    case "saving":
      return "Wait for Save to finish before recording a review decision.";
    case "no_hash":
      return "No persisted content hash yet. Save the file first.";
    case "no_review":
      return "Open a file to load run review state.";
    default:
      return null;
  }
}

/** Detect revision conflict messages from Tauri/core. */
export function isRevisionConflictError(err: unknown): boolean {
  const s = String(err);
  return s.includes("revision_conflict") || s.includes("Revision conflict");
}
