// @ts-nocheck — node fs used for structural source checks in vitest only.
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { readFileSync } from "fs";
import { dirname, join } from "path";
import { fileURLToPath } from "url";

const statusMock = vi.fn();
const projectsMock = vi.fn();
const runsMock = vi.fn();
const detailMock = vi.fn();
const rebuildMock = vi.fn();
const rescanMock = vi.fn();
const subscribeMock = vi.fn();

const demoProject = {
  projectId: "p1",
  name: "Demo",
  rootPath: "/tmp/demo",
  available: true,
  runCounts: { active: 1, ready: 0, recent: 1 },
  openFindingCount: 2,
  lastActivityAt: "2026-01-01T00:00:00Z",
};

const healthyRun = {
  runId: "r1",
  projectId: "p1",
  objective: "Ship discovery",
  lifecycle: "active",
  provisional: false,
  captureCoverage: "semantic_only",
  recordPath: ".moraine/runs/x.md",
  absolutePath: "/tmp/demo/.moraine/runs/x.md",
  checkpointCount: 2,
  evidenceCount: 1,
  openFindingCount: 1,
  riskCount: 1,
  openQuestionCount: 0,
  appendOnlyOpCount: 1,
  integrity: "current",
  recoveryRequired: false,
  startedAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T01:00:00Z",
};

const malformedRun = {
  ...healthyRun,
  runId: "r-bad",
  objective: "",
  integrity: "malformed_sidecar",
  error: "sidecar parse error",
  checkpointCount: 0,
  evidenceCount: 0,
  openFindingCount: 0,
  riskCount: 0,
  recoveryRequired: false,
};

vi.mock("@/shared/api/discovery", () => ({
  discoveryStatus: (...a: unknown[]) => statusMock(...a),
  discoveryProjects: (...a: unknown[]) => projectsMock(...a),
  discoveryRuns: (...a: unknown[]) => runsMock(...a),
  discoveryRunDetail: (...a: unknown[]) => detailMock(...a),
  discoveryRebuildIndex: (...a: unknown[]) => rebuildMock(...a),
  discoveryRescanProject: (...a: unknown[]) => rescanMock(...a),
  discoveryAddExistingProject: vi.fn(),
  subscribeDiscoveryRevision: (...a: unknown[]) => subscribeMock(...a),
  discoveryRevision: vi.fn().mockResolvedValue(0),
}));

vi.mock("@/shared/api", async () => {
  const actual = await vi.importActual<typeof import("@/shared/api")>("@/shared/api");
  return {
    ...actual,
    isTauri: false,
    getRunCheckpoints: vi.fn().mockResolvedValue({
      runId: "r1",
      contentHash: "h",
      checkpoints: [
        {
          opId: "cp1",
          summary: "First checkpoint",
          createdAt: "2026-01-01T00:00:00Z",
          openFindingCount: 1,
          findingCount: 1,
        },
      ],
      findings: [],
    }),
    listFindings: vi.fn().mockResolvedValue([]),
    getFinding: vi.fn(),
    createFinding: vi.fn(),
    changeFindingState: vi.fn(),
    listAppendOps: vi.fn().mockResolvedValue([]),
  };
});

import { Workspace } from "./Workspace";
import { LedgerTimeline } from "@/features/ledger/LedgerTimeline";
import { RunList } from "@/features/run-list/RunList";
import { ProjectList } from "@/features/projects/ProjectList";

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");

function defaultMocks() {
  statusMock.mockResolvedValue({
    online: false,
    revision: 0,
    mode: "direct",
    message: "offline",
  });
  projectsMock.mockResolvedValue([demoProject]);
  runsMock.mockResolvedValue([healthyRun]);
  detailMock.mockResolvedValue({
    summary: healthyRun,
    timeline: [
      {
        id: "t1",
        timestamp: "2026-01-01T00:00:00Z",
        kind: "checkpoint",
        summary: "Checkpoint: original → amended",
        detail: "Original claim:\noriginal\n\nCurrent statement:\namended\n",
      },
      {
        id: "t2",
        timestamp: "2026-01-01T00:01:00Z",
        kind: "evidence",
        summary: "tool result",
        provenance: "result_observed",
      },
      {
        id: "t3",
        timestamp: "2026-01-01T00:02:00Z",
        kind: "evidence",
        summary: "agent note",
        provenance: "agent_reported",
      },
    ],
    isProtocolRun: true,
    objective: "Ship discovery",
    risks: ["maybe flaky"],
    openQuestions: ["ordering?"],
    captureFidelity: {
      schemaVersion: 1,
      runId: "r1",
      integration: "codex",
      legacyCoverage: "full",
      provisional: false,
      sessionBound: true,
      dimensions: [
        {
          dimension: "tool_activity",
          capability: "supported",
          observation: "observed",
          exactCount: 1,
          countIsComplete: true,
          explanation: "tools",
        },
      ],
      gaps: [],
    },
  });
  rebuildMock.mockResolvedValue({ ok: true });
  rescanMock.mockResolvedValue({ ok: true });
  subscribeMock.mockImplementation(() => () => {});
}

describe("Workspace discovery shell", () => {
  beforeEach(() => {
    defaultMocks();
  });

  it("renders projects→runs workspace without welcome markdown", async () => {
    render(<Workspace />);
    await waitFor(() => {
      expect(screen.getByTestId("ledger-workspace")).toBeInTheDocument();
    });
    await waitFor(() => {
      expect(screen.getByText("Demo")).toBeInTheDocument();
    });
    expect(screen.getByText("Projects")).toBeInTheDocument();
    expect(screen.getByText("Runs")).toBeInTheDocument();
  });

  it("App defaults to workspace not welcome-md", () => {
    const app = readFileSync(join(root, "src/app/App.tsx"), "utf8");
    expect(app).toContain("product-shell");
    expect(app).toContain("<Workspace");
    expect(app).not.toContain("WELCOME_MD");
    expect(app).toContain("Ledger workspace");
    expect(app).not.toMatch(/moraine-welcome\.md/);
  });

  it("shows discovery offline banner when status is offline", async () => {
    render(<Workspace />);
    await waitFor(() => {
      expect(screen.getByTestId("offline-banner")).toBeInTheDocument();
    });
  });

  it("selects a run and shows review header and overview", async () => {
    const user = userEvent.setup();
    render(<Workspace />);
    await waitFor(() => expect(screen.getByText("Ship discovery")).toBeInTheDocument());
    await user.click(screen.getByText("Ship discovery"));
    await waitFor(() => expect(screen.getByTestId("run-review-header")).toBeInTheDocument());
    expect(screen.getByTestId("header-lifecycle")).toHaveTextContent("Active");
    expect(screen.getByTestId("header-fidelity")).toHaveTextContent(
      /Mechanical \+ semantic observed|Semantic observed/,
    );
    await waitFor(() => expect(screen.getByTestId("review-overview")).toBeInTheDocument());
    expect(screen.getByTestId("overview-risks")).toBeInTheDocument();
  });

  it("run change clears section to overview and supports nav", async () => {
    const user = userEvent.setup();
    const second = {
      ...healthyRun,
      runId: "r2",
      objective: "Second run",
    };
    runsMock.mockResolvedValue([healthyRun, second]);
    detailMock.mockImplementation(async (args: { runId?: string }) => ({
      summary: args?.runId === "r2" ? second : healthyRun,
      timeline: [],
      isProtocolRun: true,
      objective: args?.runId === "r2" ? "Second run" : "Ship discovery",
      risks: [],
      openQuestions: [],
      captureFidelity: {
        schemaVersion: 1,
        runId: args?.runId ?? "r1",
        integration: "claude-code",
        legacyCoverage: "full",
        provisional: false,
        sessionBound: true,
        dimensions: [
          {
            dimension: "tool_activity",
            capability: "not_supported",
            observation: "not_supported",
            exactCount: 0,
            countIsComplete: true,
            explanation: "Tool activity is not supported by this adapter.",
          },
        ],
        gaps: [],
      },
    }));
    render(<Workspace />);
    await waitFor(() => expect(screen.getByText("Ship discovery")).toBeInTheDocument());
    await user.click(screen.getByText("Ship discovery"));
    await waitFor(() => expect(screen.getByTestId("review-nav")).toBeInTheDocument());
    await user.click(screen.getByTestId("review-tab-history"));
    expect(screen.getByTestId("review-history")).toBeInTheDocument();
    await user.click(screen.getByText("Second run"));
    await waitFor(() => expect(screen.getByTestId("review-overview")).toBeInTheDocument());
  });

  it("passes category and search filters into discoveryRuns", async () => {
    const user = userEvent.setup();
    render(<Workspace />);
    await waitFor(() => expect(screen.getByText("Demo")).toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Active" }));
    await waitFor(() => {
      expect(runsMock).toHaveBeenCalledWith(expect.objectContaining({ category: "active" }));
    });
    const search = screen.getByLabelText("Search runs");
    await user.clear(search);
    await user.type(search, "Ship");
    await waitFor(() => {
      const calls = runsMock.mock.calls.map((c) => c[0]);
      expect(calls.some((a) => a?.query === "Ship")).toBe(true);
    });
  });

  it("shows malformed integrity on run card with human label", async () => {
    runsMock.mockResolvedValue([malformedRun]);
    render(<Workspace />);
    await waitFor(() => {
      expect(screen.getByText(/Malformed run record/i)).toBeInTheDocument();
    });
  });

  it("shows unavailable project warning", async () => {
    projectsMock.mockResolvedValue([
      {
        ...demoProject,
        available: false,
        warning: "needs integrity attention",
      },
    ]);
    render(<Workspace />);
    await waitFor(() => {
      expect(screen.getByText(/Project path unavailable/i)).toBeInTheDocument();
      expect(screen.getByText(/needs integrity attention/i)).toBeInTheDocument();
    });
  });

  it("empty projects state guides the user", async () => {
    projectsMock.mockResolvedValue([]);
    render(<Workspace />);
    await waitFor(() => {
      expect(screen.getByText(/No projects yet/i)).toBeInTheDocument();
    });
  });

  it("distinguishes no runs vs filter mismatch", async () => {
    // First unfiltered probe returns empty → project has no runs
    runsMock.mockResolvedValue([]);
    render(<Workspace />);
    await waitFor(() => {
      expect(screen.getByText(/This project has no runs/i)).toBeInTheDocument();
    });
  });

  it("subscribes to discovery revision and cleans up on unmount", async () => {
    const unsub = vi.fn();
    subscribeMock.mockImplementation(() => unsub);
    const { unmount } = render(<Workspace />);
    await waitFor(() => expect(subscribeMock).toHaveBeenCalled());
    unmount();
    expect(unsub).toHaveBeenCalled();
  });

  it("rebuild triggers discoveryRebuildIndex", async () => {
    const user = userEvent.setup();
    render(<Workspace />);
    await waitFor(() => expect(screen.getByText("Demo")).toBeInTheDocument());
    await user.click(screen.getByTitle("Rebuild discovery index"));
    await waitFor(() => expect(rebuildMock).toHaveBeenCalled());
  });

  it("shows captureFidelityError as explicit notice", async () => {
    const user = userEvent.setup();
    detailMock.mockResolvedValue({
      summary: healthyRun,
      timeline: [],
      isProtocolRun: true,
      objective: "Ship discovery",
      risks: [],
      openQuestions: [],
      captureFidelity: null,
      captureFidelityError: "unsupported_schema_version",
    });
    render(<Workspace />);
    await waitFor(() => expect(screen.getByText("Ship discovery")).toBeInTheDocument());
    await user.click(screen.getByText("Ship discovery"));
    await waitFor(() =>
      expect(screen.getByTestId("overview-fidelity-error")).toBeInTheDocument(),
    );
  });

  it("does not use approval vocabulary in review chrome", async () => {
    const user = userEvent.setup();
    render(<Workspace />);
    await waitFor(() => expect(screen.getByText("Ship discovery")).toBeInTheDocument());
    await user.click(screen.getByText("Ship discovery"));
    await waitFor(() => expect(screen.getByTestId("review-nav")).toBeInTheDocument());
    const shell = screen.getByTestId("ledger-workspace").textContent || "";
    expect(shell).not.toMatch(/approve|reject|block merge|safe to merge|verdict/i);
  });
});

describe("LedgerTimeline presentation", () => {
  it("renders original/amendment chain details", async () => {
    const user = userEvent.setup();
    render(
      <LedgerTimeline
        entries={[
          {
            id: "1",
            timestamp: "2026-01-01T00:00:00Z",
            kind: "checkpoint",
            summary: "Checkpoint: a → b",
            detail: "Original claim:\na\n\nCurrent statement:\nb\n",
          },
        ]}
      />,
    );
    expect(screen.getByText("checkpoint")).toBeInTheDocument();
    await user.click(screen.getByText("Details"));
    expect(screen.getByText(/Original claim/)).toBeInTheDocument();
    expect(screen.getByText(/Current statement/)).toBeInTheDocument();
  });

  it("empty timeline state", () => {
    render(<LedgerTimeline entries={[]} />);
    expect(screen.getByText(/No timeline events/i)).toBeInTheDocument();
  });
});

describe("RunList filters (local UI)", () => {
  it("exposes category coverage and filter controls", async () => {
    const user = userEvent.setup();
    const onCategory = vi.fn();
    const onQuery = vi.fn();
    const onOpenFindingsOnly = vi.fn();
    render(
      <RunList
        runs={[healthyRun]}
        selectedId={null}
        category="recent"
        query=""
        openFindingsOnly={false}
        hasRisks={false}
        hasQuestions={false}
        captureCoverage=""
        onCategory={onCategory}
        onQuery={onQuery}
        onOpenFindingsOnly={onOpenFindingsOnly}
        onHasRisks={vi.fn()}
        onHasQuestions={vi.fn()}
        onCaptureCoverage={vi.fn()}
        onSelect={vi.fn()}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Ready for review" }));
    expect(onCategory).toHaveBeenCalledWith("ready");
    await user.click(screen.getByLabelText("Open findings"));
    expect(onOpenFindingsOnly).toHaveBeenCalledWith(true);
    expect(screen.getByLabelText("Capture observation filter")).toBeInTheDocument();
  });

  it("filter mismatch offers reset", () => {
    const onReset = vi.fn();
    render(
      <RunList
        runs={[]}
        selectedId={null}
        category="active"
        query="zzz"
        openFindingsOnly={false}
        hasRisks={false}
        hasQuestions={false}
        captureCoverage=""
        projectHasRuns
        hasProject
        onCategory={vi.fn()}
        onQuery={vi.fn()}
        onOpenFindingsOnly={vi.fn()}
        onHasRisks={vi.fn()}
        onHasQuestions={vi.fn()}
        onCaptureCoverage={vi.fn()}
        onResetFilters={onReset}
        onSelect={vi.fn()}
      />,
    );
    expect(screen.getByText(/No runs match the current filters/i)).toBeInTheDocument();
  });
});

describe("ProjectList empty + offline", () => {
  it("shows offline and empty guidance", () => {
    render(
      <ProjectList
        projects={[]}
        selectedId={null}
        offline
        onSelect={vi.fn()}
        onRescan={vi.fn()}
        onAdd={vi.fn()}
        onRebuild={vi.fn()}
      />,
    );
    expect(screen.getByTestId("projects-offline")).toBeInTheDocument();
    expect(screen.getByText(/No projects yet/i)).toBeInTheDocument();
  });
});

describe("shared/api discovery has no React", () => {
  it("discovery.ts has no react imports and exports revision helpers", () => {
    const src = readFileSync(join(root, "src/shared/api/discovery.ts"), "utf8");
    expect(src).not.toMatch(/from ["']react["']/);
    expect(src).toContain("subscribeDiscoveryRevision");
    expect(src).toContain("discoveryRevision");
  });
});
