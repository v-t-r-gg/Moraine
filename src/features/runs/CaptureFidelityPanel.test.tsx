import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { CaptureFidelityPanel, type CaptureFidelityDto } from "./CaptureFidelityPanel";

function base(over: Partial<CaptureFidelityDto> = {}): CaptureFidelityDto {
  return {
    schemaVersion: 1,
    runId: "00000000-0000-4000-8000-000000000001",
    integration: "claude-code",
    legacyCoverage: "full",
    provisional: false,
    sessionBound: true,
    dimensions: [
      {
        dimension: "session_lifecycle",
        capability: "supported",
        observation: "observed",
        exactCount: 1,
        countIsComplete: true,
        explanation: "ok",
      },
      {
        dimension: "prompt_activity",
        capability: "supported",
        observation: "observed",
        exactCount: 1,
        countIsComplete: true,
        explanation: "ok",
      },
      {
        dimension: "tool_activity",
        capability: "not_supported",
        observation: "not_supported",
        exactCount: 0,
        countIsComplete: true,
        explanation: "Tool activity is not supported by this adapter.",
      },
      {
        dimension: "semantic_start",
        capability: "supported",
        observation: "observed",
        countIsComplete: true,
        explanation: "ok",
      },
      {
        dimension: "checkpoints",
        capability: "supported",
        observation: "observed",
        exactCount: 1,
        countIsComplete: true,
        explanation: "ok",
      },
    ],
    gaps: [],
    ...over,
  };
}

describe("CaptureFidelityPanel", () => {
  it("shows Claude tool activity as not supported", () => {
    render(<CaptureFidelityPanel report={base()} />);
    expect(screen.getByTestId("capture-fidelity-summary")).toHaveTextContent(
      /Claude Code/,
    );
    expect(screen.getByTestId("capture-fidelity-summary")).toHaveTextContent(
      /Mechanical \+ semantic observed/,
    );
    expect(screen.getByTestId("capture-fidelity-dimensions")).toHaveTextContent(
      /Not supported by this adapter/,
    );
  });

  it("shows Codex tools as observed", () => {
    render(
      <CaptureFidelityPanel
        report={base({
          integration: "codex",
          dimensions: [
            {
              dimension: "tool_activity",
              capability: "supported",
              observation: "observed",
              exactCount: 2,
              countIsComplete: true,
              explanation: "tools",
            },
          ],
        })}
      />,
    );
    expect(screen.getByTestId("capture-fidelity-summary")).toHaveTextContent(/Codex/);
    expect(screen.getByTestId("capture-fidelity-dimensions")).toHaveTextContent(
      /Observed \(2\)/,
    );
  });

  it("shows incomplete historical counts as at least one", () => {
    render(
      <CaptureFidelityPanel
        report={base({
          dimensions: [
            {
              dimension: "session_lifecycle",
              capability: "supported",
              observation: "observed",
              exactCount: 1,
              countIsComplete: false,
              explanation: "migrated",
            },
          ],
        })}
      />,
    );
    expect(screen.getByTestId("capture-fidelity-dimensions")).toHaveTextContent(
      /At least one observed/,
    );
  });

  it("labels mechanical-only provisional runs", () => {
    render(
      <CaptureFidelityPanel
        report={base({
          legacyCoverage: "mechanical_only",
          provisional: true,
          integration: "codex",
        })}
      />,
    );
    expect(screen.getByTestId("capture-fidelity-summary")).toHaveTextContent(
      /Mechanical observed/,
    );
    expect(screen.getByTestId("capture-fidelity-summary")).toHaveTextContent(
      /provisional/,
    );
  });

  it("labels semantic-only unbound runs", () => {
    render(
      <CaptureFidelityPanel
        report={base({
          legacyCoverage: "semantic_only",
          provisional: false,
          sessionBound: false,
          integration: null,
          dimensions: [
            {
              dimension: "session_lifecycle",
              capability: "unknown",
              observation: "unknown",
              exactCount: 0,
              countIsComplete: false,
              explanation: "Session envelope unavailable.",
            },
            {
              dimension: "semantic_start",
              capability: "supported",
              observation: "observed",
              countIsComplete: true,
              explanation: "confirmed",
            },
          ],
        })}
      />,
    );
    expect(screen.getByTestId("capture-fidelity-summary")).toHaveTextContent(
      /Semantic observed/,
    );
  });

  it("labels unknown integration coverage", () => {
    render(
      <CaptureFidelityPanel
        report={base({
          integration: "some-future-agent",
          legacyCoverage: "unknown",
        })}
      />,
    );
    expect(screen.getByTestId("capture-fidelity-summary")).toHaveTextContent(
      /some-future-agent/,
    );
    expect(screen.getByTestId("capture-fidelity-summary")).toHaveTextContent(
      /Coverage unknown/,
    );
  });

  it("shows not observed explanations without scoring language", () => {
    render(
      <CaptureFidelityPanel
        report={base({
          dimensions: [
            {
              dimension: "mechanical_evidence",
              capability: "supported",
              observation: "not_observed",
              exactCount: 0,
              countIsComplete: true,
              explanation:
                "No mechanical evidence with Moraine-observed provenance was recorded.",
            },
          ],
        })}
      />,
    );
    const panel = screen.getByTestId("capture-fidelity");
    expect(panel).toHaveTextContent(/Not observed/);
    expect(panel).toHaveTextContent(/No mechanical evidence/);
    expect(panel.textContent).not.toMatch(/percent|score|grade|approve|ready/i);
  });
});
