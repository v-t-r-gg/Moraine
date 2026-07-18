/** Room ids and URL bootstrap. Matches moraine_core room hash. */

export const DEFAULT_SYNC_URL = "ws://127.0.0.1:3099";

export interface CollabBootstrap {
  roomId: string | null;
  /** null = offline (BroadcastChannel only) */
  syncUrl: string | null;
}

export function roomIdForPath(path: string): string {
  let h = 0;
  for (let i = 0; i < path.length; i++) {
    h = (Math.imul(31, h) + path.charCodeAt(i)) | 0;
  }
  return `doc_${(h >>> 0).toString(16)}`;
}

/**
 * Query rules (first match wins for sync):
 * - sync=0|off  -> offline
 * - sync=ws...  -> that base
 * - sync=1|on   -> default relay
 * - room=...    -> force room; default relay unless sync=0
 * - VITE_MORAINE_SYNC_URL env
 */
export function collabFromLocation(
  search: string = typeof window !== "undefined" ? window.location.search : "",
): CollabBootstrap {
  const q = new URLSearchParams(search.startsWith("?") ? search : `?${search}`);
  const roomId = q.get("room");
  const sync = q.get("sync");

  if (sync === "0" || sync === "off") {
    return { roomId, syncUrl: null };
  }
  if (sync?.startsWith("ws")) {
    return { roomId, syncUrl: sync };
  }
  if (sync === "1" || sync === "on" || roomId) {
    return { roomId, syncUrl: DEFAULT_SYNC_URL };
  }

  const env = import.meta.env?.VITE_MORAINE_SYNC_URL as string | undefined;
  if (env === "0" || env === "off") return { roomId, syncUrl: null };
  if (env) return { roomId, syncUrl: env };
  return { roomId, syncUrl: null };
}
