import type {
  AppInfo,
  DocumentSnapshot,
  HistoryEntry,
  HistoryEntryMeta,
} from "./types";

const isTauri =
  typeof window !== "undefined" &&
  // @ts-expect-error Tauri injects this
  (window.__TAURI_INTERNALS__ != null || window.__TAURI__ != null);

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri) {
    return browserStub<T>(cmd, args);
  }
  const { invoke: tauriInvoke } = await import("@tauri-apps/api/core");
  return tauriInvoke<T>(cmd, args);
}

export async function openDocument(path: string): Promise<DocumentSnapshot> {
  return invoke("open_document", { path });
}

export async function getDocument(id: string): Promise<DocumentSnapshot> {
  return invoke("get_document", { id });
}

export async function setDocumentContent(id: string, content: string): Promise<void> {
  return invoke("set_document_content", { id, content });
}

export async function saveDocument(
  id: string,
  content?: string,
  recordHistory = true,
  expectedContentHash?: string | null,
): Promise<DocumentSnapshot> {
  return invoke("save_document", {
    id,
    content: content ?? null,
    recordHistory,
    expectedContentHash: expectedContentHash ?? null,
  });
}

export async function reloadDocument(id: string): Promise<DocumentSnapshot> {
  return invoke("reload_document", { id });
}

export async function historyList(path: string): Promise<HistoryEntryMeta[]> {
  return invoke("history_list", { path });
}

export async function historyGet(path: string, entryId: string): Promise<HistoryEntry> {
  return invoke("history_get", { path, entryId });
}

export async function historyRestoreContent(path: string, entryId: string): Promise<string> {
  return invoke("history_restore_content", { path, entryId });
}

export async function historyPush(
  path: string,
  content: string,
  label?: string,
): Promise<HistoryEntry> {
  return invoke("history_push", { path, content, label: label ?? null });
}

export async function appInfo(): Promise<AppInfo> {
  return invoke("app_info");
}

export async function takeStartupPath(): Promise<string | null> {
  return invoke("take_startup_path");
}

export async function writeFile(path: string, content: string): Promise<void> {
  return invoke("write_file", { path, content });
}

export async function readFile(path: string): Promise<string> {
  return invoke("read_file", { path });
}

export interface CommentDto {
  id: string;
  body: string;
  author: string;
  quote: string;
  createdAt: string;
  resolved: boolean;
  kind?: string;
}

export async function loadComments(path: string): Promise<CommentDto[]> {
  return invoke("load_comments", { path });
}

export async function saveComments(path: string, comments: CommentDto[]): Promise<void> {
  return invoke("save_comments", { path, comments });
}

export interface DecisionDto {
  id: string;
  decision: string;
  reviewerLabel: string;
  reason: string | null;
  createdAt: string;
  contentHash: string;
}

export interface RunReviewDto {
  runId: string;
  contentHash: string;
  reviewState: string;
  decisionCurrent: boolean;
  decisionCount: number;
  latest: DecisionDto | null;
  sidecar: string;
  initialized: boolean;
}

export async function getRunReview(path: string): Promise<RunReviewDto> {
  return invoke("get_run_review", { path });
}

export async function recordRunDecision(
  path: string,
  decision: string,
  reviewerLabel: string,
  reason: string | null | undefined,
  expectedContentHash: string,
): Promise<RunReviewDto> {
  return invoke("record_run_decision", {
    path,
    decision,
    reviewerLabel,
    reason: reason ?? null,
    expectedContentHash,
  });
}

export async function pickMarkdownFile(): Promise<string | null> {
  if (!isTauri) {
    return null;
  }
  const { open } = await import("@tauri-apps/plugin-dialog");
  const selected = await open({
    multiple: false,
    filters: [{ name: "Markdown", extensions: ["md", "markdown", "mdx", "mdown"] }],
  });
  if (selected == null) return null;
  return typeof selected === "string" ? selected : selected[0] ?? null;
}

export async function pickSavePath(defaultPath?: string): Promise<string | null> {
  if (!isTauri) return null;
  const { save } = await import("@tauri-apps/plugin-dialog");
  return save({
    defaultPath,
    filters: [{ name: "Markdown", extensions: ["md"] }],
  });
}

export function onFileChanged(
  handler: (event: import("./types").FileChangedEvent) => void,
): () => void {
  if (!isTauri) return () => {};
  let unlisten: (() => void) | undefined;
  import("@tauri-apps/api/event").then(({ listen }) => {
    listen<import("./types").FileChangedEvent>("file-changed", (e) => {
      handler(e.payload);
    }).then((fn) => {
      unlisten = fn;
    });
  });
  return () => unlisten?.();
}

export { isTauri };

const browserDocs = new Map<string, DocumentSnapshot>();

function browserStub<T>(cmd: string, args?: Record<string, unknown>): T {
  switch (cmd) {
    case "app_info":
      return {
        name: "Moraine",
        version: "0.1.0-browser",
        dataDir: "(browser)",
        historyDir: "(browser)",
        configDir: "(browser)",
      } as T;
    case "take_startup_path":
      return null as T;
    case "open_document": {
      const path = String(args?.path ?? "untitled.md");
      const existing = [...browserDocs.values()].find((d) => d.meta.path === path);
      if (existing) return existing as T;
      const id = crypto.randomUUID();
      const title = path.split(/[/\\]/).pop() ?? "untitled.md";
      const snap: DocumentSnapshot = {
        meta: {
          id,
          path,
          title,
          dirty: false,
          lastSavedAt: new Date().toISOString(),
          lastModifiedOnDisk: null,
          byteLen: 0,
        },
        content:
          "# Agent run record (browser stub)\n\n" +
          "Browser-only mode has no real host disk. Use the **Tauri** desktop app for file I/O and sidecar persistence.\n\n" +
          "- Open a real run-record path in desktop via `MORAINE_OPEN` or Open\n" +
          "- **Comment** / **Suggest** for human review\n" +
          "- Live share needs `moraine-server` and `?room=`\n",
      };
      snap.meta.byteLen = snap.content.length;
      browserDocs.set(id, snap);
      return snap as T;
    }
    case "save_document": {
      const id = String(args?.id);
      const doc = browserDocs.get(id);
      if (!doc) throw new Error("document not open");
      if (typeof args?.content === "string") {
        doc.content = args.content;
        doc.meta.byteLen = doc.content.length;
      }
      doc.meta.dirty = false;
      doc.meta.lastSavedAt = new Date().toISOString();
      return doc as T;
    }
    case "set_document_content": {
      const id = String(args?.id);
      const doc = browserDocs.get(id);
      if (doc && typeof args?.content === "string") {
        doc.content = args.content;
        doc.meta.dirty = true;
        doc.meta.byteLen = doc.content.length;
      }
      return undefined as T;
    }
    case "history_list":
      return [] as T;
    case "history_push":
      return {
        id: crypto.randomUUID(),
        createdAt: new Date().toISOString(),
        label: args?.label ?? null,
        contentHash: 0,
        source: "manual",
        byteLen: String(args?.content ?? "").length,
        content: String(args?.content ?? ""),
      } as T;
    case "reload_document": {
      const id = String(args?.id);
      const doc = browserDocs.get(id);
      if (!doc) throw new Error("document not open");
      return doc as T;
    }
    case "write_file":
    case "read_file":
      return undefined as T;
    case "load_comments":
      return [] as T;
    case "save_comments":
      return undefined as T;
    case "comments_sidecar_path_cmd":
      return `${args?.path ?? ""}.moraine.json` as T;
    case "get_run_review":
      return {
        runId: "00000000-0000-4000-8000-000000000000",
        contentHash: "0".repeat(64),
        reviewState: "unreviewed",
        decisionCurrent: true,
        decisionCount: 0,
        latest: null,
        sidecar: "(browser)",
        initialized: true,
      } as T;
    case "record_run_decision":
      return {
        runId: "00000000-0000-4000-8000-000000000000",
        contentHash: String(args?.expectedContentHash ?? "0".repeat(64)),
        reviewState: String(args?.decision ?? "approved"),
        decisionCurrent: true,
        decisionCount: 1,
        latest: {
          id: crypto.randomUUID(),
          decision: String(args?.decision ?? "approved"),
          reviewerLabel: String(args?.reviewerLabel ?? "Reviewer"),
          reason: (args?.reason as string) ?? null,
          createdAt: new Date().toISOString(),
          contentHash: String(args?.expectedContentHash ?? "0".repeat(64)),
        },
        sidecar: "(browser)",
        initialized: true,
      } as T;
    case "ensure_run_id":
      return "00000000-0000-4000-8000-000000000000" as T;
    default:
      console.warn("[moraine browser stub] unhandled command:", cmd, args);
      return undefined as T;
  }
}
