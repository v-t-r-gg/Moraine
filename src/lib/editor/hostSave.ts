/** Host disk write policy (desktop). Pure helpers for tests and UI. */

export const AUTOSAVE_MS = 1200;

export function shouldScheduleAutosave(opts: {
  isHost: boolean;
  peerCount: number;
  dirty: boolean;
  saving: boolean;
}): boolean {
  return opts.isHost && opts.dirty && !opts.saving && opts.peerCount === 0;
}

export function statusForPeerTransition(
  prevPeers: number,
  nextPeers: number,
  dirty: boolean,
): string | null {
  if (nextPeers > 0 && prevPeers === 0) {
    return "Autosave paused: remote collaborators present";
  }
  if (nextPeers === 0 && prevPeers > 0) {
    return dirty ? "Peers left; will autosave shortly" : "Peers left; autosave on";
  }
  return null;
}

export function remotePeerCount(awarenessSize: number): number {
  return Math.max(0, awarenessSize - 1);
}
