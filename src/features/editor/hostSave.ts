/** Host disk autosave policy. */

export const AUTOSAVE_MS = 1200;

export function canAutosave(isHost: boolean, hasRemotePeers: boolean, dirty: boolean, saving: boolean): boolean {
  return isHost && dirty && !hasRemotePeers && !saving;
}

export function remotePeerCount(awarenessSize: number): number {
  return Math.max(0, awarenessSize - 1);
}

export function peerNames(
  states: Map<number, Record<string, unknown>>,
  localClientId: number,
): string[] {
  const names: string[] = [];
  states.forEach((state, id) => {
    if (id === localClientId) return;
    const user = state.user as { name?: string } | undefined;
    names.push(user?.name ?? `Peer ${id}`);
  });
  return names;
}
