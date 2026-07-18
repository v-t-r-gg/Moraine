import { describe, expect, it } from "vitest";
import {
  remotePeerCount,
  shouldScheduleAutosave,
  statusForPeerTransition,
} from "./hostSave";

describe("hostSave", () => {
  it("autosaves only when host, dirty, solo", () => {
    expect(
      shouldScheduleAutosave({
        isHost: true,
        peerCount: 0,
        dirty: true,
        saving: false,
      }),
    ).toBe(true);
    expect(
      shouldScheduleAutosave({
        isHost: true,
        peerCount: 2,
        dirty: true,
        saving: false,
      }),
    ).toBe(false);
    expect(
      shouldScheduleAutosave({
        isHost: false,
        peerCount: 0,
        dirty: true,
        saving: false,
      }),
    ).toBe(false);
  });

  it("peer status messages", () => {
    expect(statusForPeerTransition(0, 1, true)).toMatch(/paused/);
    expect(statusForPeerTransition(1, 0, true)).toMatch(/shortly/);
    expect(statusForPeerTransition(1, 0, false)).toMatch(/autosave on/);
    expect(statusForPeerTransition(1, 1, true)).toBeNull();
  });

  it("remote peer count excludes self", () => {
    expect(remotePeerCount(1)).toBe(0);
    expect(remotePeerCount(3)).toBe(2);
  });
});
