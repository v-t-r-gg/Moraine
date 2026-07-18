/**
 * Yjs session: BroadcastChannel locally; optional WS relay (moraine-server).
 * Wire format: JSON text { type: update|awareness|sync-request, ... }.
 */

import * as Y from "yjs";
import {
  Awareness,
  applyAwarenessUpdate,
  encodeAwarenessUpdate,
} from "y-protocols/awareness";

const CHANNEL_PREFIX = "moraine-yjs:";
export const DEFAULT_SYNC_URL = "ws://127.0.0.1:3099";

export interface YjsSession {
  doc: Y.Doc;
  awareness: Awareness;
  roomId: string;
  destroy: () => void;
}

export interface YjsSessionOptions {
  userName?: string;
  syncUrl?: string | null;
}

export interface CollabBootstrap {
  roomId: string | null;
  syncUrl: string | null;
}

/** Same algorithm as moraine_core::room_id_for_str (UTF-16 units, Java-style hash). */
export function roomIdForPath(path: string): string {
  let h = 0;
  for (let i = 0; i < path.length; i++) {
    h = (Math.imul(31, h) + path.charCodeAt(i)) | 0;
  }
  return `doc_${(h >>> 0).toString(16)}`;
}

export function collabFromLocation(
  search: string = typeof window !== "undefined" ? window.location.search : "",
): CollabBootstrap {
  const q = new URLSearchParams(search.startsWith("?") ? search : `?${search}`);

  let roomId = q.get("room");
  if (!roomId) {
    const syncParam = q.get("sync");
    if (syncParam?.startsWith("room=")) {
      roomId = syncParam.slice("room=".length) || null;
    }
  }

  let syncUrl = syncUrlFromQuery(q);
  if (roomId && syncUrl === null && q.get("sync") !== "0" && q.get("sync") !== "off") {
    syncUrl = DEFAULT_SYNC_URL;
  }

  return { roomId, syncUrl };
}

function syncUrlFromQuery(q: URLSearchParams): string | null {
  const fromQuery = q.get("sync");
  if (fromQuery === "0" || fromQuery === "off") return null;
  if (fromQuery === "1" || fromQuery === "on") return DEFAULT_SYNC_URL;
  if (fromQuery?.startsWith("ws")) return fromQuery;
  if (fromQuery?.startsWith("room=")) return DEFAULT_SYNC_URL;

  const env = import.meta.env?.VITE_MORAINE_SYNC_URL as string | undefined;
  if (env === "0" || env === "off") return null;
  if (env) return env;
  return null;
}

/** @deprecated use collabFromLocation().syncUrl */
export function defaultSyncUrl(): string | null {
  return collabFromLocation().syncUrl;
}

export function createYjsSession(
  roomId: string,
  options: YjsSessionOptions | string = {},
): YjsSession {
  const opts: YjsSessionOptions =
    typeof options === "string" ? { userName: options } : options;

  const doc = new Y.Doc();
  const awareness = new Awareness(doc);

  const colors = ["#0ea5e9", "#8b5cf6", "#10b981", "#f59e0b", "#ef4444", "#ec4899"];
  const color = colors[Math.floor(Math.random() * colors.length)]!;
  const name = opts.userName ?? `User ${Math.floor(Math.random() * 900 + 100)}`;
  awareness.setLocalStateField("user", { name, color });

  const channel =
    typeof BroadcastChannel !== "undefined"
      ? new BroadcastChannel(CHANNEL_PREFIX + roomId)
      : null;

  let socket: WebSocket | null = null;
  let closed = false;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  const broadcastLocal = (msg: object) => {
    const json = JSON.stringify(msg);
    channel?.postMessage(msg);
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(json);
    }
  };

  const onDocUpdate = (update: Uint8Array, origin: unknown) => {
    if (origin === "remote" || closed) return;
    broadcastLocal({ type: "update", update: Array.from(update) });
  };

  const onAwareness = (
    {
      added,
      updated,
      removed,
    }: { added: number[]; updated: number[]; removed: number[] },
    origin: unknown,
  ) => {
    if (origin === "remote" || closed) return;
    const changed = added.concat(updated, removed);
    const update = encodeAwarenessUpdate(awareness, changed);
    broadcastLocal({ type: "awareness", update: Array.from(update) });
  };

  const handlePayload = (data: unknown) => {
    if (!data || typeof data !== "object") return;
    const msg = data as { type?: string; update?: number[] };
    if (msg.type === "update" && Array.isArray(msg.update)) {
      Y.applyUpdate(doc, Uint8Array.from(msg.update), "remote");
    } else if (msg.type === "awareness" && Array.isArray(msg.update)) {
      applyAwarenessUpdate(awareness, Uint8Array.from(msg.update), "remote");
    } else if (msg.type === "sync-request") {
      const state = Y.encodeStateAsUpdate(doc);
      broadcastLocal({ type: "update", update: Array.from(state) });
      const ids = Array.from(awareness.getStates().keys());
      if (ids.length > 0) {
        const aw = encodeAwarenessUpdate(awareness, ids);
        broadcastLocal({ type: "awareness", update: Array.from(aw) });
      }
    }
  };

  doc.on("update", onDocUpdate);
  awareness.on("update", onAwareness);

  if (channel) {
    channel.onmessage = (ev: MessageEvent) => handlePayload(ev.data);
    channel.postMessage({ type: "sync-request" });
  }

  const syncUrl = opts.syncUrl === undefined ? collabFromLocation().syncUrl : opts.syncUrl;
  if (syncUrl) {
    const connect = () => {
      if (closed) return;
      const url = `${syncUrl.replace(/\/$/, "")}/ws/${encodeURIComponent(roomId)}`;
      try {
        socket = new WebSocket(url);
      } catch {
        scheduleReconnect();
        return;
      }
      socket.onopen = () => {
        socket?.send(JSON.stringify({ type: "sync-request" }));
      };
      socket.onmessage = (ev) => {
        try {
          handlePayload(JSON.parse(String(ev.data)));
        } catch {
          /* bad frame */
        }
      };
      socket.onclose = () => scheduleReconnect();
      socket.onerror = () => {
        socket?.close();
      };
    };

    const scheduleReconnect = () => {
      if (closed || reconnectTimer) return;
      reconnectTimer = setTimeout(() => {
        reconnectTimer = null;
        connect();
      }, 1500);
    };

    connect();
  }

  return {
    doc,
    awareness,
    roomId,
    destroy: () => {
      closed = true;
      if (reconnectTimer) clearTimeout(reconnectTimer);
      doc.off("update", onDocUpdate);
      awareness.off("update", onAwareness);
      awareness.destroy();
      doc.destroy();
      channel?.close();
      socket?.close();
      socket = null;
    },
  };
}
