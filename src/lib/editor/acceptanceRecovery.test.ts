import { describe, expect, it } from "vitest";
import {
  acceptanceRecoveryMode,
  parseStructuredError,
  shortHash,
} from "./comments";

describe("acceptance recovery mode", () => {
  const base = "aaa".repeat(20) + "bbbb";
  const other = "ccc".repeat(20) + "dddd";

  it("safe incomplete acceptance allows cancel", () => {
    expect(acceptanceRecoveryMode("accepting", base, base)).toBe("cancel_safe");
  });

  it("changed document requires finalize, not cancel", () => {
    expect(acceptanceRecoveryMode("accepting", base, other)).toBe("finalize_required");
  });

  it("missing hashes are unknown (refresh only)", () => {
    expect(acceptanceRecoveryMode("accepting", base, null)).toBe("unknown");
    expect(acceptanceRecoveryMode("accepting", null, base)).toBe("unknown");
  });

  it("non-accepting is unknown", () => {
    expect(acceptanceRecoveryMode("pending", base, base)).toBe("unknown");
    expect(acceptanceRecoveryMode("accepted", base, other)).toBe("unknown");
  });

  it("uses disk hash equality not buffer semantics", () => {
    // mode is pure hash compare of the two disk hashes provided
    expect(acceptanceRecoveryMode("accepting", base, base)).toBe("cancel_safe");
    expect(acceptanceRecoveryMode("accepting", base, base + "x")).toBe("finalize_required");
  });

  it("parses structured acceptance_document_changed", () => {
    const raw = JSON.stringify({
      kind: "acceptance_document_changed",
      annotationId: "x",
      baseContentHash: base,
      currentContentHash: other,
      message: "changed",
    });
    const e = parseStructuredError(`error: ${raw}`);
    expect(e?.kind).toBe("acceptance_document_changed");
  });

  it("shortHash truncates", () => {
    expect(shortHash("abcdefghijklmnop", 8)).toBe("abcdefgh…");
  });
});
