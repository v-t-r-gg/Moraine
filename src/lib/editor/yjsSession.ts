/**
 * Local multi-tab Yjs sync via BroadcastChannel.
 * Later phases replace/augment with WebSocket (Axum) and P2P.
 */

import * as Y from "yjs";
import {
  Awareness,
  applyAwarenessUpdate,
  encodeAwarenessUpdate,
} from "y-protocols/awareness";

const CHANNEL_PREFIX = "moraine-yjs:";

export interface YjsSession {
  doc: Y.Doc;
  awareness: Awareness;
  roomId: string;
  destroy: () => void;
}

export function roomIdForPath(path: string): string {
  let h = 0;
  for (let i = 0; i < path.length; i++) {
    h = (Math.imul(31, h) + path.charCodeAt(i)) | 0;
  }
  return `doc_${(h >>> 0).toString(16)}`;
}

export function createYjsSession(roomId: string, userName?: string): YjsSession {
  const doc = new Y.Doc();
  const awareness = new Awareness(doc);

  const colors = ["#0ea5e9", "#8b5cf6", "#10b981", "#f59e0b", "#ef4444", "#ec4899"];
  const color = colors[Math.floor(Math.random() * colors.length)]!;
  const name = userName ?? `User ${Math.floor(Math.random() * 900 + 100)}`;

  awareness.setLocalStateField("user", { name, color });

  const channel =
    typeof BroadcastChannel !== "undefined"
      ? new BroadcastChannel(CHANNEL_PREFIX + roomId)
      : null;

  const onDocUpdate = (update: Uint8Array, origin: unknown) => {
    if (origin === "remote" || !channel) return;
    channel.postMessage({
      type: "update",
      update: Array.from(update),
    });
  };

  const onAwareness = (
    {
      added,
      updated,
      removed,
    }: { added: number[]; updated: number[]; removed: number[] },
    origin: unknown,
  ) => {
    if (origin === "remote" || !channel) return;
    const changed = added.concat(updated, removed);
    const update = encodeAwarenessUpdate(awareness, changed);
    channel.postMessage({ type: "awareness", update: Array.from(update) });
  };

  doc.on("update", onDocUpdate);
  awareness.on("update", onAwareness);

  if (channel) {
    channel.onmessage = (ev: MessageEvent) => {
      const data = ev.data;
      if (!data || typeof data !== "object") return;
      if (data.type === "update" && Array.isArray(data.update)) {
        Y.applyUpdate(doc, Uint8Array.from(data.update), "remote");
      } else if (data.type === "awareness" && Array.isArray(data.update)) {
        applyAwarenessUpdate(awareness, Uint8Array.from(data.update), "remote");
      } else if (data.type === "sync-request") {
        const state = Y.encodeStateAsUpdate(doc);
        channel.postMessage({
          type: "update",
          update: Array.from(state),
        });
        const ids = Array.from(awareness.getStates().keys());
        if (ids.length > 0) {
          const aw = encodeAwarenessUpdate(awareness, ids);
          channel.postMessage({ type: "awareness", update: Array.from(aw) });
        }
      }
    };

    channel.postMessage({ type: "sync-request" });
  }

  return {
    doc,
    awareness,
    roomId,
    destroy: () => {
      doc.off("update", onDocUpdate);
      awareness.off("update", onAwareness);
      awareness.destroy();
      doc.destroy();
      channel?.close();
    },
  };
}
