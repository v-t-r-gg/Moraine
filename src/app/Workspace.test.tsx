// @ts-nocheck — node fs used for structural source checks in vitest only.
import { describe, expect, it, vi, beforeEach } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach } from "vitest";
import { readFileSync } from "fs";
import { dirname, join } from "path";
import { fileURLToPath } from "url";

vi.mock("@/shared/api/discovery", () => ({
  discoveryStatus: vi.fn().mockResolvedValue({
    online: false,
    revision: 0,
    mode: "direct",
    message: "offline",
  }),
  discoveryProjects: vi.fn().mockResolvedValue([
    {
      projectId: "p1",
      name: "Demo",
      rootPath: "/tmp/demo",
      available: true,
      runCounts: { active: 1, ready: 0, recent: 1 },
      openFindingCount: 0,
    },
  ]),
  discoveryRuns: vi.fn().mockResolvedValue([
    {
      runId: "r1",
      projectId: "p1",
      objective: "Ship discovery",
      lifecycle: "active",
      provisional: false,
      captureCoverage: "semantic_only",
      recordPath: ".moraine/runs/x.md",
      absolutePath: "/tmp/demo/.moraine/runs/x.md",
      checkpointCount: 2,
      evidenceCount: 0,
      openFindingCount: 0,
      riskCount: 0,
      openQuestionCount: 0,
      appendOnlyOpCount: 0,
      integrity: "current",
      recoveryRequired: false,
    },
  ]),
  discoveryRunDetail: vi.fn().mockResolvedValue({
    summary: {
      runId: "r1",
      projectId: "p1",
      objective: "Ship discovery",
      lifecycle: "active",
      provisional: false,
      captureCoverage: "semantic_only",
      recordPath: ".moraine/runs/x.md",
      absolutePath: "/tmp/demo/.moraine/runs/x.md",
      checkpointCount: 2,
      evidenceCount: 0,
      openFindingCount: 0,
      riskCount: 0,
      openQuestionCount: 0,
      appendOnlyOpCount: 0,
      integrity: "current",
      recoveryRequired: false,
    },
    timeline: [
      {
        id: "t1",
        timestamp: "2026-01-01T00:00:00Z",
        kind: "checkpoint",
        summary: "Checkpoint: did work",
      },
    ],
    isProtocolRun: true,
    risks: [],
    openQuestions: [],
  }),
  discoveryRebuildIndex: vi.fn().mockResolvedValue({ ok: true }),
  discoveryRescanProject: vi.fn().mockResolvedValue({ ok: true }),
  discoveryAddExistingProject: vi.fn(),
}));

vi.mock("@/shared/api", async () => {
  const actual = await vi.importActual<typeof import("@/shared/api")>("@/shared/api");
  return { ...actual, isTauri: false };
});

// Heavy panels stubbed for workspace shell test
vi.mock("@/features/ledger/ProtocolLedgerPanel", () => ({
  ProtocolLedgerPanel: () => <div data-testid="protocol-ledger">ledger</div>,
}));
vi.mock("@/features/findings/CheckpointFindingsPanel", () => ({
  CheckpointFindingsPanel: () => <div>findings</div>,
}));

import { Workspace } from "./Workspace";

afterEach(() => cleanup());

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");

describe("Workspace discovery shell", () => {
  beforeEach(() => {
    vi.clearAllMocks();
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
    expect(app).toContain("showWorkspace");
    expect(app).toContain("Ledger workspace");
    expect(app).not.toMatch(/moraine-welcome\.md/);
  });
});
