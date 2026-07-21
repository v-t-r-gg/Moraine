/**
 * Single entry for collab: room resolution + Yjs session (BroadcastChannel / WS).
 */

import * as Y from "yjs";
import {
  Awareness,
  applyAwarenessUpdate,
  encodeAwarenessUpdate,
} from "y-protocols/awareness";

export const DEFAULT_SYNC_URL = "ws://127.0.0.1:3099";

export interface SessionConfig {
  roomId: string | null;
  syncUrl: string | null;
}

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

/** Same hash as moraine_core::room_id_for_str. */
export function roomIdForPath(path: string): string {
  let h = 0;
  for (let i = 0; i < path.length; i++) {
    h = (Math.imul(31, h) + path.charCodeAt(i)) | 0;
  }
  return `doc_${(h >>> 0).toString(16)}`;
}

/** Parse ?room= / ?sync= / VITE_MORAINE_SYNC_URL. */
export function resolveSessionConfig(
  search: string = typeof window !== "undefined" ? window.location.search : "",
): SessionConfig {
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

export function createYjsSession(roomId: string, options: YjsSessionOptions = {}): YjsSession {
  const doc = new Y.Doc();
  const awareness = new Awareness(doc);
  const colors = ["#0ea5e9", "#8b5cf6", "#10b981", "#f59e0b", "#ef4444", "#ec4899"];
  const color = colors[Math.floor(Math.random() * colors.length)]!;
  const name = options.userName ?? `User ${Math.floor(Math.random() * 900 + 100)}`;
  awareness.setLocalStateField("user", { name, color });

  const channel =
    typeof BroadcastChannel !== "undefined"
      ? new BroadcastChannel(`moraine-yjs:${roomId}`)
      : null;

  let socket: WebSocket | null = null;
  let closed = false;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  const send = (msg: object) => {
    const json = JSON.stringify(msg);
    channel?.postMessage(msg);
    if (socket?.readyState === WebSocket.OPEN) socket.send(json);
  };

  const onDocUpdate = (update: Uint8Array, origin: unknown) => {
    if (origin === "remote" || closed) return;
    send({ type: "update", update: Array.from(update) });
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
    const update = encodeAwarenessUpdate(awareness, added.concat(updated, removed));
    send({ type: "awareness", update: Array.from(update) });
  };

  const handlePayload = (data: unknown) => {
    if (!data || typeof data !== "object") return;
    const msg = data as { type?: string; update?: number[] };
    if (msg.type === "update" && Array.isArray(msg.update)) {
      Y.applyUpdate(doc, Uint8Array.from(msg.update), "remote");
    } else if (msg.type === "awareness" && Array.isArray(msg.update)) {
      applyAwarenessUpdate(awareness, Uint8Array.from(msg.update), "remote");
    } else if (msg.type === "sync-request") {
      send({ type: "update", update: Array.from(Y.encodeStateAsUpdate(doc)) });
      const ids = Array.from(awareness.getStates().keys());
      if (ids.length > 0) {
        send({
          type: "awareness",
          update: Array.from(encodeAwarenessUpdate(awareness, ids)),
        });
      }
    }
  };

  doc.on("update", onDocUpdate);
  awareness.on("update", onAwareness);

  if (channel) {
    channel.onmessage = (ev: MessageEvent) => handlePayload(ev.data);
    channel.postMessage({ type: "sync-request" });
  }

  const syncUrl =
    options.syncUrl !== undefined ? options.syncUrl : resolveSessionConfig().syncUrl;

  if (syncUrl) {
    const base = syncUrl.replace(/\/$/, "");
    const connect = () => {
      if (closed) return;
      try {
        socket = new WebSocket(`${base}/ws/${encodeURIComponent(roomId)}`);
      } catch {
        scheduleReconnect();
        return;
      }
      socket.onopen = () => socket?.send(JSON.stringify({ type: "sync-request" }));
      socket.onmessage = (ev) => {
        try {
          handlePayload(JSON.parse(String(ev.data)));
        } catch {
          /* ignore */
        }
      };
      socket.onclose = () => scheduleReconnect();
      socket.onerror = () => socket?.close();
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
