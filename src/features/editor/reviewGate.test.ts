import { describe, expect, it } from "vitest";
import { isRevisionConflictError } from "./reviewGate";

describe("isRevisionConflictError", () => {
  it("detects revision conflict errors", () => {
    expect(isRevisionConflictError("revision_conflict: expected a, actual b")).toBe(true);
    expect(isRevisionConflictError(new Error("Revision conflict"))).toBe(true);
    expect(isRevisionConflictError(new Error("save failed"))).toBe(false);
  });
});
