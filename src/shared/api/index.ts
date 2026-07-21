import type {
  AppInfo,
  DocumentSnapshot,
  HistoryEntry,
  HistoryEntryMeta,
} from "@/shared/types";

/** Live check (not module-load snapshot) so tests and Strict Mode remounts see the real host. */
export function isTauriRuntime(): boolean {
  if (typeof window === "undefined") return false;
  // @ts-expect-error Tauri injects these
  return window.__TAURI_INTERNALS__ != null || window.__TAURI__ != null;
}

/** @deprecated Prefer isTauriRuntime(); kept for call-site compatibility. */
export const isTauri = typeof window !== "undefined" && isTauriRuntime();

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauriRuntime()) {
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
  revision?: number;
  disposition?: string | null;
  acceptanceOpId?: string | null;
  acceptanceBaseHash?: string | null;
  acceptanceStartedAt?: string | null;
  appliedContentHash?: string | null;
  acceptanceCompletedAt?: string | null;
}

export interface AnnotationOpDto {
  annotation: CommentDto;
  comments: CommentDto[];
  runId: string;
}

export interface BeginAcceptDto {
  annotation: CommentDto;
  comments: CommentDto[];
  runId: string;
  acceptanceOpId: string;
  baseContentHash: string;
}

export interface ReconcileDto {
  comments: CommentDto[];
  created: number;
  updated: number;
  conflicts: {
    annotationId: string;
    expectedRevision: number;
    actualRevision: number;
    message: string;
  }[];
  runId: string;
}

export async function loadComments(path: string): Promise<CommentDto[]> {
  return invoke("load_comments", { path });
}

export async function createAnnotation(
  path: string,
  id: string,
  body: string,
  author: string,
  quote: string,
  kind: string,
): Promise<AnnotationOpDto> {
  return invoke("create_annotation_cmd", { path, id, body, author, quote, kind });
}

export async function updateAnnotation(
  path: string,
  id: string,
  expectedRevision: number,
  body?: string | null,
  author?: string | null,
): Promise<AnnotationOpDto> {
  return invoke("update_annotation_cmd", {
    path,
    id,
    expectedRevision,
    body: body ?? null,
    author: author ?? null,
  });
}

export async function resolveAnnotation(
  path: string,
  id: string,
  expectedRevision: number,
): Promise<AnnotationOpDto> {
  return invoke("resolve_annotation_cmd", { path, id, expectedRevision });
}

export async function reopenAnnotation(
  path: string,
  id: string,
  expectedRevision: number,
): Promise<AnnotationOpDto> {
  return invoke("reopen_annotation_cmd", { path, id, expectedRevision });
}

export async function beginAcceptSuggestion(
  path: string,
  id: string,
  expectedRevision: number,
  expectedContentHash: string,
): Promise<BeginAcceptDto> {
  return invoke("begin_accept_suggestion_cmd", {
    path,
    id,
    expectedRevision,
    expectedContentHash,
  });
}

export async function completeAcceptSuggestion(
  path: string,
  id: string,
  expectedRevision: number,
  acceptanceOpId: string,
  expectedSavedHash: string,
): Promise<AnnotationOpDto> {
  return invoke("complete_accept_suggestion_cmd", {
    path,
    id,
    expectedRevision,
    acceptanceOpId,
    expectedSavedHash,
  });
}

export async function cancelAcceptSuggestion(
  path: string,
  id: string,
  expectedRevision: number,
  acceptanceOpId: string,
): Promise<AnnotationOpDto> {
  return invoke("cancel_accept_suggestion_cmd", {
    path,
    id,
    expectedRevision,
    acceptanceOpId,
  });
}

export interface AcceptanceRecoveryDto {
  annotationId: string;
  disposition: string;
  revision: number;
  acceptanceOpId: string | null;
  baseContentHash: string | null;
  currentContentHash: string;
  cancelSafe: boolean;
}

export async function getAcceptanceRecoveryStatus(
  path: string,
  id: string,
): Promise<AcceptanceRecoveryDto> {
  return invoke("acceptance_recovery_status_cmd", { path, id });
}

export async function rejectSuggestion(
  path: string,
  id: string,
  expectedRevision: number,
): Promise<AnnotationOpDto> {
  return invoke("reject_suggestion_cmd", { path, id, expectedRevision });
}

/** Host Save: merge live-session annotations without full-list replace or deletes. */
export async function reconcileSessionAnnotations(
  path: string,
  comments: CommentDto[],
): Promise<ReconcileDto> {
  return invoke("reconcile_session_annotations_cmd", { path, comments });
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

/** Finding kinds (descriptive review context — not verdicts). */
export type FindingKind =
  | "clarification"
  | "inconsistency"
  | "missing_evidence"
  | "risk_concern"
  | "factual_correction"
  | "other";

export type FindingState = "open" | "addressed" | "archived";

export interface FindingTargetDto {
  kind: string;
  checkpointOpId: string;
  snapshotHash: string;
  checkpointSummary: string;
}

export interface FindingListItemDto {
  findingId: string;
  runId: string;
  kind: FindingKind | string;
  state: FindingState | string;
  body: string;
  createdAt: string;
  updatedAt: string;
  responseCount: number;
  target: FindingTargetDto;
}

export interface FindingThreadItemDto {
  itemKind: "finding" | "response" | string;
  id: string;
  body: string;
  createdAt: string;
  authorKind?: string | null;
  findingKind?: string | null;
}

export interface FindingDetailDto {
  findingId: string;
  runId: string;
  kind: string;
  state: string;
  body: string;
  createdAt: string;
  updatedAt: string;
  target: FindingTargetDto;
  targetSnapshot: {
    opId: string;
    summary: string;
    createdAt: string;
    [key: string]: unknown;
  };
  thread: FindingThreadItemDto[];
  responses: Array<{
    id: string;
    findingId: string;
    body: string;
    createdAt: string;
    idempotencyKey: string;
    authorKind: string;
  }>;
  ledgerEvents: Array<{
    eventId: string;
    event: string;
    findingId: string;
    createdAt: string;
    [key: string]: unknown;
  }>;
}

export interface CheckpointSummaryDto {
  opId: string;
  summary: string;
  createdAt: string;
  openFindingCount: number;
  findingCount: number;
}

export interface RunCheckpointsDetailDto {
  runId: string;
  contentHash: string;
  checkpoints: CheckpointSummaryDto[];
  findings: FindingListItemDto[];
}

export async function getRunCheckpoints(path: string): Promise<RunCheckpointsDetailDto> {
  return invoke("get_run_checkpoints_cmd", { path });
}

export async function createFinding(
  path: string,
  checkpointOpId: string,
  kind: FindingKind | string,
  body: string,
): Promise<{ findingId: string; finding: FindingDetailDto; state: string; kind: string }> {
  return invoke("create_finding_cmd", {
    path,
    checkpointOpId,
    kind,
    body,
  });
}

export async function listFindings(
  path: string,
  openOnly = false,
): Promise<FindingListItemDto[]> {
  return invoke("list_findings_cmd", { path, openOnly });
}

export async function getFinding(path: string, findingId: string): Promise<FindingDetailDto> {
  return invoke("get_finding_cmd", { path, findingId });
}

export async function changeFindingState(
  path: string,
  findingId: string,
  state: FindingState | string,
): Promise<{ findingId: string; state: string; finding: FindingDetailDto }> {
  return invoke("change_finding_state_cmd", { path, findingId, state });
}

export async function pickMarkdownFile(): Promise<string | null> {
  if (!isTauriRuntime()) {
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
  if (!isTauriRuntime()) return null;
  const { save } = await import("@tauri-apps/plugin-dialog");
  return save({
    defaultPath,
    filters: [{ name: "Markdown", extensions: ["md"] }],
  });
}

export type FileChangedHandler = (
  event: import("@/shared/types").FileChangedEvent,
) => void;

export type TauriListen = <T>(
  event: string,
  handler: (event: { payload: T }) => void,
) => Promise<() => void>;

/**
 * Core subscribe/unsubscribe for file-changed events.
 * Accepts an injectable `listen` so tests drive the real cancel/late-resolve
 * logic without depending on module-load Tauri detection.
 *
 * Cleanup is safe under React Strict Mode: if unlisten runs before `listen`
 * resolves, the late registration is cancelled and any resolved unlisten is
 * invoked immediately so duplicate handlers cannot accumulate.
 */
export function subscribeFileChanged(
  listen: TauriListen,
  handler: FileChangedHandler,
): () => void {
  let cancelled = false;
  let unlisten: (() => void) | undefined;
  void listen<import("@/shared/types").FileChangedEvent>("file-changed", (e) => {
    if (!cancelled) handler(e.payload);
  })
    .then((fn) => {
      if (cancelled) {
        fn();
        return;
      }
      unlisten = fn;
    })
    .catch(() => {
      /* host unavailable */
    });
  return () => {
    cancelled = true;
    unlisten?.();
    unlisten = undefined;
  };
}

/**
 * Subscribe to host file-changed events (Tauri).
 * No-op when not running under the desktop host.
 */
export function onFileChanged(handler: FileChangedHandler): () => void {
  if (!isTauriRuntime()) return () => {};
  return subscribeFileChanged(async (event, h) => {
    const { listen } = await import("@tauri-apps/api/event");
    return listen(event, h);
  }, handler);
}

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
      const content =
        "# Agent run record (browser stub)\n\n" +
        "Browser-only mode has no real host disk. Use the **Tauri** desktop app for file I/O and sidecar persistence.\n\n" +
        "- Open a real run-record path in desktop via `MORAINE_OPEN` or Open\n" +
        "- **Comment** / **Suggest** for human review\n" +
        "- Live share needs `moraine-server` and `?room=`\n";
      const snap: DocumentSnapshot = {
        meta: {
          id,
          path,
          title,
          dirty: false,
          lastSavedAt: new Date().toISOString(),
          lastModifiedOnDisk: null,
          byteLen: content.length,
        },
        content,
        contentHash: "0".repeat(64),
      };
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
      doc.contentHash = "0".repeat(64);
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
    case "create_annotation_cmd":
    case "update_annotation_cmd":
    case "resolve_annotation_cmd":
    case "reopen_annotation_cmd":
    case "reject_suggestion_cmd":
    case "complete_accept_suggestion_cmd":
    case "cancel_accept_suggestion_cmd": {
      const id = String(args?.id ?? crypto.randomUUID());
      const kind = String(args?.kind ?? "comment");
      const expected = Number(args?.expectedRevision ?? 0);
      const isSug = kind === "suggestion" || cmd.includes("accept") || cmd.includes("reject");
      let disposition: string | null = isSug ? "pending" : null;
      let resolved = false;
      if (cmd === "reject_suggestion_cmd") {
        disposition = "rejected";
        resolved = true;
      }
      if (cmd === "complete_accept_suggestion_cmd") {
        disposition = "accepted";
        resolved = true;
      }
      if (cmd === "resolve_annotation_cmd") resolved = true;
      const ann = {
        id,
        body: String(args?.body ?? ""),
        author: String(args?.author ?? "You"),
        quote: String(args?.quote ?? ""),
        createdAt: new Date().toISOString(),
        resolved,
        kind: isSug ? "suggestion" : "comment",
        revision: cmd === "create_annotation_cmd" ? 1 : expected + 1 || 1,
        disposition,
      };
      return {
        annotation: ann,
        comments: [ann],
        runId: "00000000-0000-4000-8000-000000000000",
      } as T;
    }
    case "begin_accept_suggestion_cmd": {
      const id = String(args?.id ?? crypto.randomUUID());
      const expected = Number(args?.expectedRevision ?? 0);
      const ann = {
        id,
        body: "",
        author: "You",
        quote: "",
        createdAt: new Date().toISOString(),
        resolved: false,
        kind: "suggestion",
        revision: expected + 1 || 1,
        disposition: "accepting",
        acceptanceOpId: crypto.randomUUID(),
      };
      return {
        annotation: ann,
        comments: [ann],
        runId: "00000000-0000-4000-8000-000000000000",
        acceptanceOpId: ann.acceptanceOpId,
        baseContentHash: String(args?.expectedContentHash ?? "0".repeat(64)),
      } as T;
    }
    case "acceptance_recovery_status_cmd":
      return {
        annotationId: String(args?.id ?? ""),
        disposition: "accepting",
        revision: 2,
        acceptanceOpId: crypto.randomUUID(),
        baseContentHash: "0".repeat(64),
        currentContentHash: "0".repeat(64),
        cancelSafe: true,
      } as T;
    case "reconcile_session_annotations_cmd":
      return {
        comments: (args?.comments as CommentDto[]) ?? [],
        created: 0,
        updated: 0,
        conflicts: [],
        runId: "00000000-0000-4000-8000-000000000000",
      } as T;
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
    case "ensure_run_id":
      return "00000000-0000-4000-8000-000000000000" as T;
    case "get_run_checkpoints_cmd":
      return {
        runId: "00000000-0000-4000-8000-000000000000",
        contentHash: "0".repeat(64),
        checkpoints: [],
        findings: [],
      } as T;
    case "list_findings_cmd":
      return [] as T;
    case "create_finding_cmd":
    case "get_finding_cmd":
    case "change_finding_state_cmd":
      throw new Error("findings require the Tauri desktop host");
    default:
      console.warn("[moraine browser stub] unhandled command:", cmd, args);
      return undefined as T;
  }
}
