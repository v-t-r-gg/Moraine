/** Human-readable labels for review workspace (no raw enum dumps in primary UI). */

export function lifecycleLabel(lifecycle: string): string {
  switch (lifecycle) {
    case "active":
      return "Active";
    case "ready_for_review":
      return "Ready for review";
    case "closed":
      return "Closed";
    default:
      return lifecycle || "Unknown";
  }
}

export function coverageLabel(coverage: string): string {
  switch (coverage) {
    case "full":
      return "Mechanical + semantic observed";
    case "mechanical_only":
      return "Mechanical observed";
    case "semantic_only":
      return "Semantic observed";
    case "partial":
      return "Partial observation";
    case "unknown":
    default:
      return "Coverage unknown";
  }
}

export function agentLabel(integration?: string | null): string {
  if (!integration) return "Unknown agent";
  if (integration === "claude-code") return "Claude Code";
  if (integration === "codex") return "Codex";
  return integration;
}

export function integrityLabel(integrity: string): string {
  switch (integrity) {
    case "current":
      return "Record current";
    case "malformed_sidecar":
      return "Malformed run record";
    case "unsupported_schema":
      return "Unsupported run schema";
    case "recovery_required":
      return "Recovery required";
    default:
      return integrity || "Unknown integrity";
  }
}

export function observationLabel(obs: string): string {
  switch (obs) {
    case "observed":
      return "Observed";
    case "not_observed":
      return "Not observed";
    case "not_supported":
      return "Not supported by this adapter";
    case "unknown":
    default:
      return "Unknown";
  }
}

export function findingKindLabel(kind: string): string {
  switch (kind) {
    case "clarification":
      return "Clarification";
    case "inconsistency":
      return "Inconsistency";
    case "missing_evidence":
      return "Missing evidence";
    case "risk_concern":
      return "Risk concern";
    case "factual_correction":
      return "Factual correction";
    case "other":
      return "Other";
    default:
      return kind;
  }
}

export function findingStateLabel(state: string): string {
  switch (state) {
    case "open":
      return "Open finding";
    case "addressed":
      return "Addressed finding";
    case "archived":
      return "Archived finding";
    default:
      return state;
  }
}

export function provenanceLabel(p?: string | null): string {
  switch (p) {
    case "invocation_observed":
      return "Invocation observed";
    case "result_observed":
      return "Result observed";
    case "moraine_captured":
      return "Moraine captured";
    case "agent_reported":
      return "Agent reported";
    case "external_reference":
      return "External reference";
    default:
      return p || "Unknown provenance";
  }
}

export function formatWhen(iso?: string | null): string {
  if (!iso) return "—";
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}

export type ReviewSection = "overview" | "checkpoints" | "evidence" | "findings" | "history";

export const REVIEW_SECTIONS: { id: ReviewSection; label: string }[] = [
  { id: "overview", label: "Overview" },
  { id: "checkpoints", label: "Checkpoints" },
  { id: "evidence", label: "Evidence" },
  { id: "findings", label: "Findings" },
  { id: "history", label: "History" },
];
