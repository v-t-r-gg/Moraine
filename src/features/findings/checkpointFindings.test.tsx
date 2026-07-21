// @ts-nocheck — structural + unit tests for findings UI and API binding.
import { readFileSync } from "fs";
import { dirname, join } from "path";
import { fileURLToPath } from "url";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");

vi.mock("@/shared/api", async () => {
  const actual = await vi.importActual<typeof import("@/shared/api")>("@/shared/api");
  return {
    ...actual,
    getRunCheckpoints: vi.fn(),
    getFinding: vi.fn(),
    createFinding: vi.fn(),
    changeFindingState: vi.fn(),
  };
});

import {
  changeFindingState,
  createFinding,
  getFinding,
  getRunCheckpoints,
} from "@/shared/api";
import { CheckpointFindingsPanel } from "./CheckpointFindingsPanel";

describe("checkpoint findings desktop surface", () => {
  it("ships create-finding form and chronological thread", () => {
    const panel = readFileSync(
      join(root, "src/features/findings/CheckpointFindingsPanel.tsx"),
      "utf8",
    );
    expect(panel).toContain("Add finding");
    expect(panel).toContain("createFinding");
    expect(panel).toContain("Finding thread");
    expect(panel).toContain("Human finding");
    expect(panel).toContain("Agent response");
    expect(panel).toContain("Mark addressed");
    expect(panel).toContain("no verdict");
    expect(panel).not.toMatch(/\bapprove\b/i);
    expect(panel).not.toMatch(/\breject\b/i);
  });

  it("wires framework-neutral host API modules", () => {
    const api = readFileSync(join(root, "src/shared/api/findings.ts"), "utf8");
    expect(api).toContain("createFinding");
    expect(api).toContain("listFindings");
    expect(api).toContain("getFinding");
    expect(api).toContain("changeFindingState");
    const app = readFileSync(join(root, "src/app/App.tsx"), "utf8");
    expect(app).toContain("CheckpointFindingsPanel");
  });
});

describe("CheckpointFindingsPanel behavior", () => {
  beforeEach(() => {
    vi.mocked(getRunCheckpoints).mockReset();
    vi.mocked(getFinding).mockReset();
    vi.mocked(createFinding).mockReset();
    vi.mocked(changeFindingState).mockReset();
  });

  it("shows empty state when no checkpoints", async () => {
    vi.mocked(getRunCheckpoints).mockResolvedValue({
      runId: "r1",
      contentHash: "h",
      checkpoints: [],
      findings: [],
    });
    render(<CheckpointFindingsPanel path="/tmp/run.md" />);
    await waitFor(() => {
      expect(screen.getByText(/No structured checkpoints/i)).toBeInTheDocument();
    });
  });

  it("creates a finding via real API client", async () => {
    const user = userEvent.setup();
    vi.mocked(getRunCheckpoints).mockResolvedValue({
      runId: "r1",
      contentHash: "h",
      checkpoints: [
        {
          opId: "cp1",
          summary: "Did work",
          createdAt: "2026-01-01T00:00:00Z",
          openFindingCount: 0,
          findingCount: 0,
        },
      ],
      findings: [],
    });
    vi.mocked(createFinding).mockResolvedValue({
      findingId: "f1",
      state: "open",
      kind: "clarification",
      finding: {
        findingId: "f1",
        runId: "r1",
        kind: "clarification",
        state: "open",
        body: "Why?",
        createdAt: "2026-01-01T00:00:00Z",
        updatedAt: "2026-01-01T00:00:00Z",
        target: {
          kind: "checkpoint",
          checkpointOpId: "cp1",
          snapshotHash: "h",
          checkpointSummary: "Did work",
        },
        targetSnapshot: { opId: "cp1", summary: "Did work", createdAt: "2026-01-01T00:00:00Z" },
        thread: [
          {
            itemKind: "finding",
            id: "f1",
            body: "Why?",
            createdAt: "2026-01-01T00:00:00Z",
            authorKind: "human",
            findingKind: "clarification",
          },
        ],
        responses: [],
        ledgerEvents: [],
      },
    });
    vi.mocked(getFinding).mockResolvedValue({
      findingId: "f1",
      runId: "r1",
      kind: "clarification",
      state: "open",
      body: "Why?",
      createdAt: "2026-01-01T00:00:00Z",
      updatedAt: "2026-01-01T00:00:00Z",
      target: {
        kind: "checkpoint",
        checkpointOpId: "cp1",
        snapshotHash: "h",
        checkpointSummary: "Did work",
      },
      targetSnapshot: { opId: "cp1", summary: "Did work", createdAt: "2026-01-01T00:00:00Z" },
      thread: [
        {
          itemKind: "finding",
          id: "f1",
          body: "Why?",
          createdAt: "2026-01-01T00:00:00Z",
          authorKind: "human",
          findingKind: "clarification",
        },
        {
          itemKind: "response",
          id: "r1",
          body: "Because tests.",
          createdAt: "2026-01-01T01:00:00Z",
          authorKind: "agent",
        },
      ],
      responses: [],
      ledgerEvents: [],
    });

    render(<CheckpointFindingsPanel path="/tmp/run.md" />);
    await waitFor(() => expect(screen.getByText("Add finding")).toBeInTheDocument());
    await user.type(screen.getByPlaceholderText(/Descriptive review context/i), "Why?");
    await user.click(screen.getByRole("button", { name: /Create finding/i }));
    await waitFor(() => {
      expect(createFinding).toHaveBeenCalledWith(
        "/tmp/run.md",
        "cp1",
        "clarification",
        "Why?",
      );
    });
  });
});
