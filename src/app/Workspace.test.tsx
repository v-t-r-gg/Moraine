// @ts-nocheck — node fs used for structural source checks in vitest only.
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
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
  return { ...actual, isTauri: false };
});

vi.mock("@/features/ledger/ProtocolLedgerPanel", () => ({
  ProtocolLedgerPanel: () => <div data-testid="protocol-ledger">ledger</div>,
}));
vi.mock("@/features/findings/CheckpointFindingsPanel", () => ({
  CheckpointFindingsPanel: () => <div>findings</div>,
}));

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
        detail: "Original claim:\noriginal\n\nAmendment (fix):\nPrior: original\nNew: amended\n\nCurrent statement:\namended\n",
      },
      {
        id: "t2",
        timestamp: "2026-01-01T00:01:00Z",
        kind: "amendment",
        summary: "Amendment: fix",
      },
    ],
    isProtocolRun: true,
    risks: ["maybe flaky"],
    openQuestions: [],
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
    // C3: ledger workspace is the default product shell (not welcome-md / showWorkspace toggle).
    expect(app).toContain("product-shell");
    expect(app).toContain("<Workspace");
    expect(app).not.toContain("WELCOME_MD");
    expect(app).toContain("Ledger workspace");
    expect(app).not.toMatch(/moraine-welcome\.md/);
  });

  it("shows service offline banner when status is offline", async () => {
    render(<Workspace />);
    await waitFor(() => {
      expect(screen.getByText(/Service offline/i)).toBeInTheDocument();
    });
  });

  it("selects a run and shows timeline original→amended→current", async () => {
    const user = userEvent.setup();
    render(<Workspace />);
    await waitFor(() => expect(screen.getByText("Ship discovery")).toBeInTheDocument());
    await user.click(screen.getByText("Ship discovery"));
    await waitFor(() => expect(screen.getByTestId("protocol-ledger")).toBeInTheDocument());
    await waitFor(() => expect(screen.getByText("Timeline")).toBeInTheDocument());
    expect(screen.getByText(/Checkpoint: original → amended/)).toBeInTheDocument();
    // Expand details for original/current claim chain
    const details = screen.getByText("Details");
    await user.click(details);
    expect(screen.getByText(/Original claim/)).toBeInTheDocument();
    expect(screen.getByText(/Current statement/)).toBeInTheDocument();
  });

  it("passes category and search filters into discoveryRuns", async () => {
    const user = userEvent.setup();
    render(<Workspace />);
    await waitFor(() => expect(screen.getByText("Demo")).toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "active" }));
    await waitFor(() => {
      expect(runsMock).toHaveBeenCalledWith(
        expect.objectContaining({ category: "active" }),
      );
    });
    const search = screen.getByLabelText("Search runs");
    await user.clear(search);
    await user.type(search, "Ship");
    await waitFor(() => {
      const calls = runsMock.mock.calls.map((c) => c[0]);
      expect(calls.some((a) => a?.query === "Ship")).toBe(true);
    });
  });

  it("shows malformed integrity on run card", async () => {
    runsMock.mockResolvedValue([malformedRun]);
    render(<Workspace />);
    await waitFor(() => {
      expect(screen.getByText("malformed_sidecar")).toBeInTheDocument();
    });
  });

  it("shows unavailable project warning", async () => {
    projectsMock.mockResolvedValue([
      {
        ...demoProject,
        available: false,
        warning: "project path unavailable",
      },
    ]);
    render(<Workspace />);
    await waitFor(() => {
      expect(screen.getByText("unavailable")).toBeInTheDocument();
      expect(screen.getByText(/project path unavailable/)).toBeInTheDocument();
    });
  });

  it("empty projects state guides the user", async () => {
    projectsMock.mockResolvedValue([]);
    render(<Workspace />);
    await waitFor(() => {
      expect(screen.getByText(/No projects yet/i)).toBeInTheDocument();
    });
  });

  it("empty filter matches state", async () => {
    runsMock.mockResolvedValue([]);
    render(<Workspace />);
    await waitFor(() => {
      expect(screen.getByText(/No runs match filters/i)).toBeInTheDocument();
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
    await user.click(screen.getByRole("button", { name: "ready" }));
    expect(onCategory).toHaveBeenCalledWith("ready");
    await user.click(screen.getByLabelText("Open findings"));
    expect(onOpenFindingsOnly).toHaveBeenCalledWith(true);
    expect(screen.getByLabelText("Capture coverage filter")).toBeInTheDocument();
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
    expect(screen.getByText(/Service offline/i)).toBeInTheDocument();
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
