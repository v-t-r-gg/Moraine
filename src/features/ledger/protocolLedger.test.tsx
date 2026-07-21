// @ts-nocheck — node fs used for structural source checks in vitest only.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { readFileSync } from "fs";
import { dirname, join } from "path";
import { fileURLToPath } from "url";
import { currentClaim, isClaimRedacted, ProtocolLedgerPanel } from "./ProtocolLedgerPanel";

vi.mock("@/shared/api", async () => {
  const actual = await vi.importActual<typeof import("@/shared/api")>("@/shared/api");
  return {
    ...actual,
    getRunCheckpoints: vi.fn(),
    listAppendOps: vi.fn(),
    humanObservationAdd: vi.fn(),
    runAmend: vi.fn(),
  };
});

import {
  getRunCheckpoints,
  humanObservationAdd,
  listAppendOps,
  runAmend,
} from "@/shared/api";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");

afterEach(() => cleanup());

describe("currentClaim helper", () => {
  it("applies amend then redaction without erasing history inputs", () => {
    const ops = [
      {
        opId: "a1",
        opKind: "run_amend",
        actorCategory: "agent",
        createdAt: "t1",
        reason: "fix",
        targetId: "cp1",
        targetKind: "checkpoint",
        previousSnapshotHash: "h",
        previousContent: "All concurrency tests pass.",
        newContent: "All concurrency tests, including ordering, pass.",
        relationship: "amended" as const,
      },
      {
        opId: "r1",
        opKind: "entry_redact",
        actorCategory: "human",
        createdAt: "t2",
        reason: "sensitive",
        targetId: "cp1",
        targetKind: "checkpoint",
        previousSnapshotHash: "h2",
        previousContent: "All concurrency tests, including ordering, pass.",
        newContent: "[REDACTED]",
        relationship: "redacted" as const,
      },
    ];
    expect(currentClaim("cp1", "All concurrency tests pass.", ops)).toBe("[REDACTED]");
    expect(isClaimRedacted("cp1", ops)).toBe(true);
    expect(ops[0]!.previousContent).toContain("All concurrency tests pass");
  });
});

describe("ProtocolLedgerPanel", () => {
  beforeEach(() => {
    vi.mocked(getRunCheckpoints).mockReset();
    vi.mocked(listAppendOps).mockReset();
    vi.mocked(humanObservationAdd).mockReset();
    vi.mocked(runAmend).mockReset();
  });

  it("is labeled append-only read-only claims surface", async () => {
    vi.mocked(getRunCheckpoints).mockResolvedValue({
      runId: "r",
      contentHash: "h",
      checkpoints: [],
      findings: [],
    });
    vi.mocked(listAppendOps).mockResolvedValue([]);
    render(<ProtocolLedgerPanel path="/tmp/run.md" />);
    await waitFor(() => {
      expect(screen.getByText(/append-only · read-only claims/i)).toBeInTheDocument();
    });
    expect(screen.getByRole("button", { name: /Add observation/i })).toBeInTheDocument();
  });

  it("shows original and current claim after amendment", async () => {
    vi.mocked(getRunCheckpoints).mockImplementation(async () => ({
      runId: "r",
      contentHash: "h",
      checkpoints: [
        {
          opId: "cp1",
          summary: "All concurrency tests pass.",
          createdAt: "2026-01-01T00:00:00Z",
          openFindingCount: 0,
          findingCount: 0,
        },
      ],
      findings: [],
    }));
    vi.mocked(listAppendOps).mockImplementation(async () => [
      {
        opId: "a1",
        opKind: "run_amend",
        actorCategory: "agent",
        createdAt: "2026-01-01T01:00:00Z",
        reason: "Incomplete",
        targetId: "cp1",
        targetKind: "checkpoint",
        previousSnapshotHash: "h",
        previousContent: "All concurrency tests pass.",
        newContent: "All concurrency tests, including ordering, pass.",
        relationship: "amended",
      },
    ]);
    const { container } = render(<ProtocolLedgerPanel path="/tmp/run.md" />);
    await waitFor(() => {
      expect(within(container).getByTestId("original-claim")).toBeInTheDocument();
    });
    expect(within(container).getByTestId("original-claim").textContent).toContain(
      "All concurrency tests pass.",
    );
    expect(within(container).getByTestId("current-statement").textContent).toContain(
      "including ordering",
    );
  });

  it("adds observation via real host API client", async () => {
    const user = userEvent.setup();
    vi.mocked(getRunCheckpoints).mockResolvedValue({
      runId: "r",
      contentHash: "h",
      checkpoints: [
        {
          opId: "cp1",
          summary: "Work done",
          createdAt: "2026-01-01T00:00:00Z",
          openFindingCount: 0,
          findingCount: 0,
        },
      ],
      findings: [],
    });
    vi.mocked(listAppendOps).mockResolvedValue([]);
    vi.mocked(humanObservationAdd).mockResolvedValue({
      runId: "r",
      opId: "o1",
      opKind: "human_observation_add",
      relationship: "observation",
      op: {
        opId: "o1",
        opKind: "human_observation_add",
        actorCategory: "human",
        createdAt: "2026-01-01T02:00:00Z",
        reason: "review observation",
        previousSnapshotHash: "h",
        newContent: "Need more evidence",
        relationship: "observation",
      },
    });

    const { container } = render(<ProtocolLedgerPanel path="/tmp/run.md" />);
    await waitFor(() =>
      expect(
        within(container).getByRole("button", { name: /Add observation/i }),
      ).toBeInTheDocument(),
    );
    await user.type(
      within(container).getByPlaceholderText(/Human observation/i),
      "Need more evidence",
    );
    await user.click(within(container).getByRole("button", { name: /Add observation/i }));
    await waitFor(() => {
      expect(humanObservationAdd).toHaveBeenCalled();
    });
    expect(runAmend).not.toHaveBeenCalled();
  });

  it("ordinary UI withholds redacted claim text (original, amend chain, current)", async () => {
    const secret = "All concurrency tests pass.";
    const amended = "All concurrency tests, including ordering, pass.";
    vi.mocked(getRunCheckpoints).mockImplementation(async () => ({
      runId: "r",
      contentHash: "h",
      checkpoints: [
        {
          opId: "cp1",
          summary: secret,
          createdAt: "2026-01-01T00:00:00Z",
          openFindingCount: 0,
          findingCount: 0,
        },
      ],
      findings: [],
    }));
    vi.mocked(listAppendOps).mockImplementation(async () => [
      {
        opId: "a1",
        opKind: "run_amend",
        actorCategory: "agent",
        createdAt: "2026-01-01T01:00:00Z",
        reason: "Incomplete",
        targetId: "cp1",
        targetKind: "checkpoint",
        previousSnapshotHash: "h",
        previousContent: secret,
        newContent: amended,
        relationship: "amended",
      },
      {
        opId: "r1",
        opKind: "entry_redact",
        actorCategory: "human",
        createdAt: "2026-01-01T02:00:00Z",
        reason: "sensitive",
        targetId: "cp1",
        targetKind: "checkpoint",
        previousSnapshotHash: "h2",
        previousContent: amended,
        newContent: "[REDACTED]",
        relationship: "redacted",
      },
    ]);

    const { container } = render(<ProtocolLedgerPanel path="/tmp/run.md" />);
    await waitFor(() => {
      expect(within(container).getByTestId("original-claim")).toBeInTheDocument();
    });
    const body = container.textContent ?? "";
    expect(within(container).getByTestId("original-claim").textContent).toContain("[REDACTED]");
    expect(within(container).getByTestId("current-statement").textContent).toContain(
      "[REDACTED]",
    );
    expect(within(container).getByTestId("redaction-marker")).toBeInTheDocument();
    expect(body).not.toContain(secret);
    expect(body).not.toContain(amended);
    expect(body).toContain("sensitive"); // redaction reason is ok to show
    expect(body).toMatch(/Prior content retained in structured ledger/i);
  });

  it("ships bindings without general dirty/save for protocol ledger", () => {
    const src = readFileSync(
      join(root, "src/features/ledger/ProtocolLedgerPanel.tsx"),
      "utf8",
    );
    expect(src).toContain("Add observation");
    expect(src).toContain("humanObservationAdd");
    expect(src).toContain("runAmend");
    expect(src).toContain("append-only");
    expect(src).toContain("isClaimRedacted");
    expect(src.toLowerCase()).not.toContain("approve");
  });

  it("legacy document route labels free-form editing; ledger shell stays primary", () => {
    const app = readFileSync(join(root, "src/app/App.tsx"), "utf8");
    const legacy = readFileSync(join(root, "src/app/LegacyDocumentApp.tsx"), "utf8");
    expect(app).toContain("Open legacy document");
    expect(app).toContain("LegacyDocumentApp");
    expect(legacy).toContain("Legacy document route");
    expect(legacy).toContain("ProtocolLedgerPanel");
    expect(legacy).toContain("isProtocolRunMarkdown");
  });
});
