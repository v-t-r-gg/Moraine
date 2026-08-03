/** Compact capture fidelity panel — shared report facts only. */

export interface CaptureDimensionDto {
  dimension: string;
  capability: string;
  observation: string;
  exactCount?: number | null;
  countIsComplete: boolean;
  explanation: string;
}

export interface CaptureFidelityDto {
  schemaVersion: number;
  runId: string;
  integration?: string | null;
  legacyCoverage: string;
  provisional: boolean;
  sessionBound: boolean;
  dimensions: CaptureDimensionDto[];
  gaps: { dimension: string; reason: string }[];
}

const DIMENSION_LABELS: Record<string, string> = {
  session_lifecycle: "Session lifecycle",
  prompt_activity: "Prompt activity",
  tool_activity: "Tool activity",
  semantic_start: "Semantic start",
  checkpoints: "Checkpoints",
  mechanical_evidence: "Mechanical evidence",
  agent_reported_evidence: "Agent-reported evidence",
  review_findings: "Review findings",
};

function legacyLabel(coverage: string): string {
  switch (coverage) {
    case "full":
      return "Mechanical + semantic observed";
    case "mechanical_only":
      return "Mechanical observed";
    case "semantic_only":
      return "Semantic observed";
    case "partial":
      return "Partial observation";
    default:
      return "Coverage unknown";
  }
}

function observationLabel(d: CaptureDimensionDto): string {
  switch (d.observation) {
    case "observed":
      if (d.exactCount != null && d.exactCount > 0) {
        if (d.countIsComplete) return `Observed (${d.exactCount})`;
        return "At least one observed";
      }
      return "Observed";
    case "not_observed":
      return "Not observed";
    case "not_supported":
      return "Not supported by this adapter";
    default:
      return "Unknown";
  }
}

function integrationLabel(id?: string | null): string {
  if (!id) return "Unknown";
  if (id === "claude-code") return "Claude Code";
  if (id === "codex") return "Codex";
  return id;
}

export function CaptureFidelityPanel({ report }: { report: CaptureFidelityDto }) {
  return (
    <div
      className="border-t px-3 py-2"
      style={{ borderColor: "var(--border)" }}
      data-testid="capture-fidelity"
    >
      <div className="mb-1 font-medium">Capture fidelity</div>
      <div style={{ color: "var(--muted)" }} data-testid="capture-fidelity-summary">
        Integration: {integrationLabel(report.integration)} ·{" "}
        {legacyLabel(report.legacyCoverage)}
        {report.provisional ? " · provisional" : ""}
      </div>
      <ul className="mt-2 grid gap-1" data-testid="capture-fidelity-dimensions">
        {report.dimensions.map((d) => (
          <li key={d.dimension} className="flex flex-wrap gap-x-2">
            <span className="min-w-[10rem] font-medium">
              {DIMENSION_LABELS[d.dimension] ?? d.dimension}
            </span>
            <span style={{ color: "var(--muted)" }}>{observationLabel(d)}</span>
            {(d.observation === "not_observed" ||
              d.observation === "not_supported" ||
              d.observation === "unknown") &&
            d.explanation ? (
              <span className="basis-full text-[10px]" style={{ color: "var(--muted)" }}>
                {d.explanation}
              </span>
            ) : null}
          </li>
        ))}
      </ul>
    </div>
  );
}
