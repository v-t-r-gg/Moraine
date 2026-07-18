import { describe, expect, it } from "vitest";
import {
  canRecordDecision,
  decisionGateMessage,
  isRevisionConflictError,
} from "./reviewGate";

describe("reviewGate", () => {
  it("allows decisions on a clean document with hash", () => {
    const g = canRecordDecision({
      dirty: false,
      externalConflict: false,
      saving: false,
      hasReview: true,
      hasPersistedHash: true,
    });
    expect(g.allowed).toBe(true);
    expect(g.reason).toBe("ok");
  });

  it("disables after an edit (dirty)", () => {
    const g = canRecordDecision({
      dirty: true,
      externalConflict: false,
      saving: false,
      hasReview: true,
      hasPersistedHash: true,
    });
    expect(g.allowed).toBe(false);
    expect(g.reason).toBe("unsaved");
    expect(decisionGateMessage(g.reason)).toMatch(/Save the current revision/);
  });

  it("allows after successful save (clean again)", () => {
    let dirty = true;
    let g = canRecordDecision({
      dirty,
      externalConflict: false,
      saving: false,
      hasReview: true,
      hasPersistedHash: true,
    });
    expect(g.allowed).toBe(false);
    dirty = false; // after save
    g = canRecordDecision({
      dirty,
      externalConflict: false,
      saving: false,
      hasReview: true,
      hasPersistedHash: true,
    });
    expect(g.allowed).toBe(true);
  });

  it("failed save leaves controls disabled (still dirty)", () => {
    const g = canRecordDecision({
      dirty: true,
      externalConflict: false,
      saving: false,
      hasReview: true,
      hasPersistedHash: true,
    });
    expect(g.allowed).toBe(false);
  });

  it("external file change invalidates decisions", () => {
    const g = canRecordDecision({
      dirty: false,
      externalConflict: true,
      saving: false,
      hasReview: true,
      hasPersistedHash: true,
    });
    expect(g.allowed).toBe(false);
    expect(g.reason).toBe("external_conflict");
  });

  it("detects revision conflict errors", () => {
    expect(isRevisionConflictError("revision_conflict: expected a, actual b")).toBe(true);
    expect(isRevisionConflictError(new Error("save failed"))).toBe(false);
  });
});
