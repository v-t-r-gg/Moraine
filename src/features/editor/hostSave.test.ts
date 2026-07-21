import { describe, expect, it } from "vitest";
import { canAutosave, remotePeerCount } from "./hostSave";

describe("hostSave", () => {
  it("autosaves only when host, dirty, solo, not saving", () => {
    expect(canAutosave(true, false, true, false)).toBe(true);
    expect(canAutosave(true, true, true, false)).toBe(false);
    expect(canAutosave(false, false, true, false)).toBe(false);
    expect(canAutosave(true, false, false, false)).toBe(false);
    expect(canAutosave(true, false, true, true)).toBe(false);
  });

  it("chaos peer toggles", () => {
    let peers = 0;
    const dirty = true;
    const samples = [0, 1, 2, 0, 1, 0];
    const can: boolean[] = [];
    for (const p of samples) {
      peers = p;
      can.push(canAutosave(true, peers > 0, dirty, false));
    }
    expect(can).toEqual([true, false, false, true, false, true]);
  });

  it("remote peer count excludes self", () => {
    expect(remotePeerCount(1)).toBe(0);
    expect(remotePeerCount(3)).toBe(2);
  });
});
