import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { EvidenceReviewPanel } from "./EvidenceReviewPanel";
import { HistoryReviewPanel } from "./HistoryReviewPanel";
import { RunOverviewPanel } from "./RunOverviewPanel";
import {
  agentLabel,
  coverageLabel,
  lifecycleLabel,
  observationLabel,
} from "./labels";

describe("review labels", () => {
  it("never calls full complete knowledge", () => {
    expect(coverageLabel("full")).toBe("Mechanical + semantic observed");
    expect(coverageLabel("full")).not.toMatch(/complete/i);
    expect(lifecycleLabel("ready_for_review")).toBe("Ready for review");
    expect(agentLabel("claude-code")).toBe("Claude Code");
    expect(observationLabel("not_supported")).toMatch(/Not supported/);
  });
});

describe("EvidenceReviewPanel", () => {
  it("distinguishes observed vs agent-reported", async () => {
    const user = userEvent.setup();
    render(
      <EvidenceReviewPanel
        timeline={[
          {
            id: "1",
            timestamp: "2026-01-01T00:00:00Z",
            kind: "evidence",
            summary: "shell finished",
            provenance: "result_observed",
          },
          {
            id: "2",
            timestamp: "2026-01-01T00:01:00Z",
            kind: "evidence",
            summary: "agent note",
            provenance: "agent_reported",
          },
        ]}
      />,
    );
    expect(screen.getAllByTestId("evidence-item")).toHaveLength(2);
    await user.click(screen.getByRole("button", { name: "Agent-reported" }));
    expect(screen.getAllByTestId("evidence-item")).toHaveLength(1);
    expect(screen.getByText(/claim \(not independently verified\)/i)).toBeInTheDocument();
  });
});

describe("HistoryReviewPanel", () => {
  it("expands technical detail", async () => {
    const user = userEvent.setup();
    render(
      <HistoryReviewPanel
        timeline={[
          {
            id: "h1",
            timestamp: "2026-01-01T00:00:00Z",
            kind: "checkpoint",
            actorCategory: "agent",
            summary: "did work",
            targetId: "cp1",
          },
        ]}
      />,
    );
    await user.click(screen.getByTestId("history-expand-h1"));
    expect(screen.getByTestId("history-detail-h1")).toHaveTextContent("cp1");
  });
});

describe("RunOverviewPanel fidelity error", () => {
  it("renders captureFidelityError notice", () => {
    render(
      <RunOverviewPanel
        run={{
          runId: "r1",
          projectId: "p1",
          objective: "obj",
          lifecycle: "active",
          provisional: false,
          captureCoverage: "unknown",
          recordPath: "x",
          absolutePath: "/x",
          checkpointCount: 0,
          evidenceCount: 0,
          openFindingCount: 0,
          riskCount: 0,
          openQuestionCount: 0,
          appendOnlyOpCount: 0,
          integrity: "current",
          recoveryRequired: false,
        }}
        detail={{
          summary: {
            runId: "r1",
            projectId: "p1",
            objective: "obj",
            lifecycle: "active",
            provisional: false,
            captureCoverage: "unknown",
            recordPath: "x",
            absolutePath: "/x",
            checkpointCount: 0,
            evidenceCount: 0,
            openFindingCount: 0,
            riskCount: 0,
            openQuestionCount: 0,
            appendOnlyOpCount: 0,
            integrity: "current",
            recoveryRequired: false,
          },
          timeline: [],
          isProtocolRun: true,
          risks: [],
          openQuestions: [],
          captureFidelityError: "unsupported_schema_version",
        }}
        onGoFindings={() => {}}
        onGoCheckpoints={() => {}}
        onGoHistory={() => {}}
      />,
    );
    expect(screen.getByTestId("overview-fidelity-error")).toBeInTheDocument();
  });
});
