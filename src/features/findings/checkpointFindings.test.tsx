/**
 * Behavioral RTL for CheckpointFindingsPanel against the real component +
 * mocked framework-neutral shared/api (production paths, not fixtures).
 */
import { describe, expect, it, vi, beforeEach } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach } from "vitest";

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
  type FindingDetailDto,
  type FindingListItemDto,
  type RunCheckpointsDetailDto,
} from "@/shared/api";
import { CheckpointFindingsPanel } from "./CheckpointFindingsPanel";

afterEach(() => {
  cleanup();
});

const CP = {
  opId: "cp1",
  summary: "Implemented widget",
  createdAt: "2026-01-01T00:00:00Z",
  openFindingCount: 1,
  findingCount: 1,
};

function openFinding(overrides: Partial<FindingListItemDto> = {}): FindingListItemDto {
  return {
    findingId: "f1",
    runId: "r1",
    kind: "clarification",
    state: "open",
    body: "Please clarify validation.",
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    responseCount: 0,
    target: {
      kind: "checkpoint",
      checkpointOpId: "cp1",
      snapshotHash: "abc",
      checkpointSummary: "Implemented widget",
    },
    ...overrides,
  };
}

function detailWithThread(state: string, withResponse: boolean): FindingDetailDto {
  const thread: FindingDetailDto["thread"] = [
    {
      itemKind: "finding",
      id: "f1",
      body: "Please clarify validation.",
      createdAt: "2026-01-01T00:00:00Z",
      authorKind: "human",
      findingKind: "clarification",
    },
  ];
  if (withResponse) {
    thread.push({
      itemKind: "response",
      id: "resp1",
      body: "Ran cargo test -p widget; exit 0.",
      createdAt: "2026-01-01T01:00:00Z",
      authorKind: "agent",
    });
  }
  return {
    findingId: "f1",
    runId: "r1",
    kind: "clarification",
    state,
    body: "Please clarify validation.",
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T01:00:00Z",
    target: {
      kind: "checkpoint",
      checkpointOpId: "cp1",
      snapshotHash: "abc",
      checkpointSummary: "Implemented widget",
    },
    targetSnapshot: {
      opId: "cp1",
      summary: "Implemented widget",
      createdAt: "2026-01-01T00:00:00Z",
    },
    thread,
    responses: withResponse
      ? [
          {
            id: "resp1",
            findingId: "f1",
            body: "Ran cargo test -p widget; exit 0.",
            createdAt: "2026-01-01T01:00:00Z",
            idempotencyKey: "k1",
            authorKind: "agent",
          },
        ]
      : [],
    ledgerEvents: [],
  };
}

function baseDetail(findings: FindingListItemDto[] = []): RunCheckpointsDetailDto {
  return {
    runId: "r1",
    contentHash: "h",
    checkpoints: [
      {
        ...CP,
        openFindingCount: findings.filter((f) => f.state === "open").length,
        findingCount: findings.length,
      },
    ],
    findings,
  };
}

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

  it("lists loaded open findings for a checkpoint", async () => {
    vi.mocked(getRunCheckpoints).mockResolvedValue(
      baseDetail([openFinding({ responseCount: 1 })]),
    );
    const { container } = render(<CheckpointFindingsPanel path="/tmp/run.md" />);
    await waitFor(() => {
      expect(
        within(container).getByText("Please clarify validation."),
      ).toBeInTheDocument();
    });
    expect(within(container).getByText(/1 response/i)).toBeInTheDocument();
  });

  it("displays chronological human finding and agent response in the thread", async () => {
    const user = userEvent.setup();
    vi.mocked(getRunCheckpoints).mockResolvedValue(baseDetail([openFinding({ responseCount: 1 })]));
    vi.mocked(getFinding).mockResolvedValue(detailWithThread("open", true));

    const { container } = render(<CheckpointFindingsPanel path="/tmp/run.md" />);
    await waitFor(() =>
      expect(within(container).getByText("Please clarify validation.")).toBeInTheDocument(),
    );
    await user.click(
      within(container).getByRole("button", { name: /Please clarify validation/i }),
    );

    await waitFor(() => {
      expect(within(container).getByText("Finding thread")).toBeInTheDocument();
    });
    expect(within(container).getByText(/Human finding/i)).toBeInTheDocument();
    expect(within(container).getByText(/Agent response/i)).toBeInTheDocument();
    expect(within(container).getByText(/Ran cargo test -p widget/i)).toBeInTheDocument();
  });

  it("creates a finding via the real shared/api client", async () => {
    const user = userEvent.setup();
    vi.mocked(getRunCheckpoints).mockResolvedValue(baseDetail([]));
    vi.mocked(createFinding).mockResolvedValue({
      findingId: "f-new",
      state: "open",
      kind: "missing_evidence",
      finding: detailWithThread("open", false),
    });
    vi.mocked(getFinding).mockResolvedValue(detailWithThread("open", false));

    const { container } = render(<CheckpointFindingsPanel path="/tmp/run.md" />);
    await waitFor(() =>
      expect(within(container).getByRole("button", { name: /Create finding/i })).toBeInTheDocument(),
    );

    await user.selectOptions(within(container).getByLabelText(/^Kind$/i), "missing_evidence");
    await user.type(
      within(container).getByPlaceholderText(/Descriptive review context/i),
      "Need proof",
    );
    await user.click(within(container).getByRole("button", { name: /Create finding/i }));

    await waitFor(() => {
      expect(createFinding).toHaveBeenCalledWith(
        "/tmp/run.md",
        "cp1",
        "missing_evidence",
        "Need proof",
      );
    });
  });

  it("marks addressed, archives, and reopens via changeFindingState", async () => {
    const user = userEvent.setup();
    let currentState = "open";
    vi.mocked(getRunCheckpoints).mockImplementation(async () =>
      baseDetail([openFinding({ state: currentState })]),
    );
    vi.mocked(getFinding).mockImplementation(async () =>
      detailWithThread(currentState, false),
    );
    vi.mocked(changeFindingState).mockImplementation(async (_p, _id, state) => {
      currentState = state;
      return {
        findingId: "f1",
        state,
        finding: detailWithThread(state, false),
      };
    });

    const { container } = render(<CheckpointFindingsPanel path="/tmp/run.md" />);
    await waitFor(() =>
      expect(within(container).getByText("Please clarify validation.")).toBeInTheDocument(),
    );
    await user.click(
      within(container).getByRole("button", { name: /Please clarify validation/i }),
    );
    await waitFor(() =>
      expect(within(container).getByRole("button", { name: /Mark addressed/i })).toBeInTheDocument(),
    );

    await user.click(within(container).getByRole("button", { name: /Mark addressed/i }));
    await waitFor(() => {
      expect(changeFindingState).toHaveBeenCalledWith("/tmp/run.md", "f1", "addressed");
      expect(within(container).getByRole("button", { name: /Archive/i })).toBeInTheDocument();
      expect(within(container).queryByRole("button", { name: /Mark addressed/i })).toBeNull();
    });

    await user.click(within(container).getByRole("button", { name: /Archive/i }));
    await waitFor(() => {
      expect(changeFindingState).toHaveBeenCalledWith("/tmp/run.md", "f1", "archived");
      expect(within(container).getByRole("button", { name: /Reopen/i })).toBeInTheDocument();
    });

    await user.click(within(container).getByRole("button", { name: /Reopen/i }));
    await waitFor(() => {
      expect(changeFindingState).toHaveBeenCalledWith("/tmp/run.md", "f1", "open");
      expect(within(container).getByRole("button", { name: /Mark addressed/i })).toBeInTheDocument();
    });
  });

  it("surfaces backend conflict errors on create", async () => {
    const user = userEvent.setup();
    vi.mocked(getRunCheckpoints).mockResolvedValue(baseDetail([]));
    vi.mocked(createFinding).mockRejectedValue(new Error("invalid_finding: body required"));

    const { container } = render(<CheckpointFindingsPanel path="/tmp/run.md" />);
    await waitFor(() =>
      expect(within(container).getByRole("button", { name: /Create finding/i })).toBeInTheDocument(),
    );
    await user.type(
      within(container).getByPlaceholderText(/Descriptive review context/i),
      "x",
    );
    await user.click(within(container).getByRole("button", { name: /Create finding/i }));

    await waitFor(() => {
      expect(within(container).getByText(/invalid_finding/i)).toBeInTheDocument();
    });
  });

  it("surfaces load failure for missing/stale checkpoint path", async () => {
    vi.mocked(getRunCheckpoints).mockRejectedValue(new Error("run missing agent state"));
    const { container } = render(<CheckpointFindingsPanel path="/tmp/missing.md" />);
    await waitFor(() => {
      expect(within(container).getByText(/run missing agent state/i)).toBeInTheDocument();
    });
  });

  it("reloads after mutation when refreshToken bumps", async () => {
    vi.mocked(getRunCheckpoints)
      .mockResolvedValueOnce(baseDetail([]))
      .mockResolvedValueOnce(baseDetail([openFinding()]));

    const { container, rerender } = render(
      <CheckpointFindingsPanel path="/tmp/run.md" refreshToken={0} />,
    );
    await waitFor(() => expect(within(container).getByText(/None yet/i)).toBeInTheDocument());

    rerender(<CheckpointFindingsPanel path="/tmp/run.md" refreshToken={1} />);
    await waitFor(() => {
      expect(getRunCheckpoints).toHaveBeenCalledTimes(2);
      expect(within(container).getByText("Please clarify validation.")).toBeInTheDocument();
    });
  });

  it("does not render approval or rejection controls", async () => {
    vi.mocked(getRunCheckpoints).mockResolvedValue(baseDetail([openFinding()]));
    vi.mocked(getFinding).mockResolvedValue(detailWithThread("open", true));
    const user = userEvent.setup();
    const { container } = render(<CheckpointFindingsPanel path="/tmp/run.md" />);
    await waitFor(() =>
      expect(within(container).getByText("Please clarify validation.")).toBeInTheDocument(),
    );
    await user.click(
      within(container).getByRole("button", { name: /Please clarify validation/i }),
    );
    await waitFor(() =>
      expect(within(container).getByText("Finding thread")).toBeInTheDocument(),
    );
    expect(within(container).queryByRole("button", { name: /approve/i })).toBeNull();
    expect(within(container).queryByRole("button", { name: /^reject$/i })).toBeNull();
    expect(within(container).getByText(/no verdict/i)).toBeInTheDocument();
  });
});
