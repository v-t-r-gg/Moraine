import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Toolbar } from "@/features/shell/Toolbar";
import { StatusBar } from "@/features/shell/StatusBar";
import { Preview } from "@/features/shell/Preview";
import { HistoryPanel } from "@/features/shell/HistoryPanel";
import { CommentsPanel } from "@/features/annotations/CommentsPanel";
import { RunReviewPanel } from "@/features/runs/RunReviewPanel";
import { CheckpointFindingsPanel } from "@/features/findings/CheckpointFindingsPanel";
import { ProtocolLedgerPanel } from "@/features/ledger/ProtocolLedgerPanel";
import { Editor, type EditorHandle } from "@/features/editor/Editor";
import { isProtocolRunMarkdown } from "@/features/editor/managedRegion";
import {
  beginAcceptSuggestion,
  cancelAcceptSuggestion,
  completeAcceptSuggestion,
  createAnnotation,
  getAcceptanceRecoveryStatus,
  getRunReview,
  historyList,
  historyRestoreContent,
  isTauri,
  isTauriRuntime,
  loadComments,
  onFileChanged,
  openDocument,
  pickMarkdownFile,
  reconcileSessionAnnotations,
  rejectSuggestion as rejectSuggestionApi,
  reloadDocument,
  reopenAnnotation,
  resolveAnnotation,
  saveDocument,
  takeStartupPath,
  writeFile,
  type CommentDto,
  type RunReviewDto,
} from "@/shared/api";
import {
  createYjsSession,
  resolveSessionConfig,
  roomIdForPath,
  type SessionConfig,
  type YjsSession,
} from "@/features/editor/yjsSession";
import {
  applyDurableRecord,
  commentsMap,
  countPending,
  isAnnotationConflictError,
  listComments,
  mergeDiskIntoMap,
  newCommentId,
  upsertComment,
  type CommentRecord,
} from "@/features/editor/comments";
import {
  AUTOSAVE_MS,
  canAutosave,
  peerNames,
  remotePeerCount,
} from "@/features/editor/hostSave";
import { isRevisionConflictError } from "@/features/editor/reviewGate";
import { classifyDiskEvent, type ViewportState } from "@/features/editor/diskWatch";
import type { DocumentSnapshot, HistoryEntryMeta, ViewMode } from "@/shared/types";

function countWords(text: string): number {
  const t = text.trim();
  if (!t) return 0;
  return t.split(/\s+/).length;
}

function dtoToRecord(c: CommentDto): CommentRecord {
  return {
    id: c.id,
    body: c.body,
    author: c.author,
    quote: c.quote,
    createdAt: c.createdAt,
    resolved: c.resolved,
    kind: c.kind === "suggestion" ? "suggestion" : "comment",
    revision: c.revision && c.revision > 0 ? c.revision : 1,
    disposition: (c.disposition as CommentRecord["disposition"]) ?? null,
    acceptanceOpId: c.acceptanceOpId ?? null,
    acceptanceBaseHash: c.acceptanceBaseHash ?? null,
    acceptanceStartedAt: c.acceptanceStartedAt ?? null,
    appliedContentHash: c.appliedContentHash ?? null,
    acceptanceCompletedAt: c.acceptanceCompletedAt ?? null,
  };
}

function recordToDto(c: CommentRecord): CommentDto {
  return {
    id: c.id,
    body: c.body,
    author: c.author,
    quote: c.quote,
    createdAt: c.createdAt,
    resolved: c.resolved,
    kind: c.kind,
    revision: c.revision ?? 1,
    disposition: c.disposition ?? null,
    acceptanceOpId: c.acceptanceOpId ?? null,
    acceptanceBaseHash: c.acceptanceBaseHash ?? null,
    acceptanceStartedAt: c.acceptanceStartedAt ?? null,
    appliedContentHash: c.appliedContentHash ?? null,
    acceptanceCompletedAt: c.acceptanceCompletedAt ?? null,
  };
}

function hashFromSnap(snap: DocumentSnapshot): string | null {
  if (snap.contentHash && snap.contentHash.length === 64) return snap.contentHash;
  return null;
}

export interface LegacyDocumentAppProps {
  /** Optional path to open on mount (absolute). */
  initialPath?: string | null;
  productStatus?: string | null;
  onBackToWorkspace?: () => void;
}

/**
 * Historical/compatibility free-form document editor.
 * Live collab (Yjs relay) is frozen for C3 beta — local editor session only.
 * Not the primary installed-product path (see App ledger workspace).
 */
export function LegacyDocumentApp({
  initialPath = null,
  productStatus = null,
  onBackToWorkspace,
}: LegacyDocumentAppProps) {
  const [doc, setDoc] = useState<DocumentSnapshot | null>(null);
  const [markdown, setMarkdown] = useState("");
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [viewMode, setViewMode] = useState<ViewMode>("edit");
  const [historyOpen, setHistoryOpen] = useState(false);
  const [commentsOpen, setCommentsOpen] = useState(false);
  const [showResolvedComments, setShowResolvedComments] = useState(false);
  const [historyEntries, setHistoryEntries] = useState<HistoryEntryMeta[]>([]);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [commentList, setCommentList] = useState<CommentRecord[]>([]);
  const [orphanedMarkIds, setOrphanedMarkIds] = useState<string[]>([]);
  const [status, setStatus] = useState<string | null>(
    productStatus ? `${productStatus} · legacy document` : "Legacy document mode",
  );
  const [session, setSession] = useState<YjsSession | null>(null);
  const [peerCount, setPeerCount] = useState(0);
  const [peerLabel, setPeerLabel] = useState("");
  // C3: freeze live collab — never attach remote sync URL.
  const [sessionCfg, setSessionCfg] = useState<SessionConfig>({ roomId: null, syncUrl: null });
  const [localAuthor, setLocalAuthor] = useState("You");
  const [runReview, setRunReview] = useState<RunReviewDto | null>(null);
  const [findingsRefreshToken, setFindingsRefreshToken] = useState(0);
  const [recoveryBusy, setRecoveryBusy] = useState(false);
  const [baseContentHash, setBaseContentHash] = useState<string | null>(null);
  const [externalConflict, setExternalConflict] = useState(false);
  const [conflictLocalMarkdown, setConflictLocalMarkdown] = useState<string | null>(null);
  const [lastHandledExternalHash, setLastHandledExternalHash] = useState<string | null>(null);

  const editorRef = useRef<EditorHandle | null>(null);
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const unsubCommentsRef = useRef<(() => void) | null>(null);
  const sessionRef = useRef<YjsSession | null>(null);
  const docRef = useRef(doc);
  const markdownRef = useRef(markdown);
  const dirtyRef = useRef(dirty);
  const savingRef = useRef(saving);
  const baseHashRef = useRef(baseContentHash);
  const lastHandledRef = useRef(lastHandledExternalHash);
  const prevPeersRef = useRef(0);
  const ignoreProgrammaticRef = useRef(false);
  const programmaticGenRef = useRef(0);
  const pendingRehydrateRef = useRef(false);
  const watchInFlightRef = useRef<Promise<void> | null>(null);
  const watchGenRef = useRef(0);
  const commentListRef = useRef(commentList);

  docRef.current = doc;
  markdownRef.current = markdown;
  dirtyRef.current = dirty;
  savingRef.current = saving;
  baseHashRef.current = baseContentHash;
  lastHandledRef.current = lastHandledExternalHash;
  sessionRef.current = session;
  commentListRef.current = commentList;

  const title = doc?.meta.title ?? "Moraine";
  const path = doc?.meta.path ?? null;
  const wordCount = useMemo(() => countWords(markdown), [markdown]);
  const charCount = markdown.length;
  const hasRemotePeers = peerCount > 0;
  const pending = useMemo(() => countPending(commentList), [commentList]);
  /** Protocol runs use append-only ledger UX; free-form edit is Legacy mode only. */
  const isProtocolRun = useMemo(() => isProtocolRunMarkdown(markdown), [markdown]);
  const legacyDocumentMode = Boolean(doc) && !isProtocolRun;
  // Document-only route (ledger workspace lives in App.tsx).


  const clearSaveTimer = useCallback(() => {
    if (saveTimerRef.current) {
      clearTimeout(saveTimerRef.current);
      saveTimerRef.current = null;
    }
  }, []);

  const beginProgrammaticUpdate = useCallback(() => {
    programmaticGenRef.current += 1;
    ignoreProgrammaticRef.current = true;
  }, []);

  const endProgrammaticUpdateSoon = useCallback(() => {
    const gen = programmaticGenRef.current;
    queueMicrotask(() => {
      requestAnimationFrame(() => {
        if (gen === programmaticGenRef.current) {
          ignoreProgrammaticRef.current = false;
        }
      });
    });
  }, []);

  const refreshRunReview = useCallback(async (filePath: string) => {
    if (!isTauri) {
      setRunReview(null);
      return;
    }
    try {
      const review = await getRunReview(filePath);
      setRunReview(review);
      if (review.contentHash) setBaseContentHash(review.contentHash);
      setFindingsRefreshToken((t) => t + 1);
    } catch (e) {
      setStatus(`error: could not load run review (${e})`);
    }
  }, []);

  const tryRehydrateMarks = useCallback(() => {
    if (!pendingRehydrateRef.current || !editorRef.current?.rehydrateMarks) return;
    pendingRehydrateRef.current = false;
    const open = commentListRef.current.filter((c) => !c.resolved);
    const { applied, orphaned } = editorRef.current.rehydrateMarks(open);
    setOrphanedMarkIds(orphaned);
    if (open.length === 0) return;
    const parts: string[] = [];
    const pend = countPending(open);
    if (pend.suggestions) {
      parts.push(
        `${pend.suggestions} suggestion${pend.suggestions === 1 ? "" : "s"} pending`,
      );
    }
    if (applied.length) parts.push(`${applied.length} mark(s) restored`);
    if (orphaned.length) parts.push(`${orphaned.length} quote(s) not found in text`);
    if (parts.length) setStatus(parts.join("; "));
  }, []);

  const seedCommentsFromDisk = useCallback(
    async (filePath: string, cmap: ReturnType<typeof commentsMap>) => {
      if (!isTauri) return;
      try {
        const disk = await loadComments(filePath);
        const records = disk.map(dtoToRecord);
        mergeDiskIntoMap(cmap, records);
        setCommentList(listComments(cmap, true));
        pendingRehydrateRef.current = true;
        tryRehydrateMarks();
      } catch {
        /* no sidecar */
      }
    },
    [tryRehydrateMarks],
  );

  const applyDocument = useCallback(
    (snap: DocumentSnapshot, resetSession: boolean) => {
      beginProgrammaticUpdate();
      setDoc(snap);
      setMarkdown(snap.content);
      setDirty(snap.meta.dirty);
      setExternalConflict(false);
      setConflictLocalMarkdown(null);
      const h = hashFromSnap(snap);
      if (h) setBaseContentHash(h);
      setLastHandledExternalHash(null);

      if (resetSession) {
        sessionRef.current?.destroy();
        unsubCommentsRef.current?.();
        unsubCommentsRef.current = null;
        clearSaveTimer();

        const room = sessionCfg.roomId ?? roomIdForPath(snap.meta.path);
        const s = createYjsSession(room, { syncUrl: sessionCfg.syncUrl });
        setSession(s);
        sessionRef.current = s;
        setPeerCount(0);
        prevPeersRef.current = 0;
        setPeerLabel("");
        setLocalAuthor(
          (s.awareness.getLocalState()?.user as { name?: string } | undefined)?.name ?? "You",
        );

        const cmap = commentsMap(s.doc);
        const refresh = () => setCommentList(listComments(cmap, true));
        refresh();
        cmap.observe(refresh);
        unsubCommentsRef.current = () => cmap.unobserve(refresh);

        s.awareness.on("change", () => {
          const size = s.awareness.getStates().size;
          const next = remotePeerCount(size);
          if (next !== prevPeersRef.current) {
            if (next > 0 && prevPeersRef.current === 0) clearSaveTimer();
            prevPeersRef.current = next;
          }
          setPeerCount(next);
          setPeerLabel(
            peerNames(
              s.awareness.getStates() as Map<number, Record<string, unknown>>,
              s.doc.clientID,
            ).join(", "),
          );
        });

        void seedCommentsFromDisk(snap.meta.path, cmap);
        void refreshRunReview(snap.meta.path);
      }

      endProgrammaticUpdateSoon();
    },
    [
      beginProgrammaticUpdate,
      clearSaveTimer,
      endProgrammaticUpdateSoon,
      refreshRunReview,
      seedCommentsFromDisk,
      sessionCfg.roomId,
      sessionCfg.syncUrl,
    ],
  );

  const applyDocumentInPlace = useCallback(
    (snap: DocumentSnapshot) => {
      const samePath = docRef.current?.meta.path === snap.meta.path;
      if (!samePath || !sessionRef.current) {
        applyDocument(snap, true);
        return;
      }
      const vp: ViewportState | undefined = editorRef.current?.getViewportState?.();
      beginProgrammaticUpdate();
      setDoc(snap);
      setMarkdown(snap.content);
      setDirty(false);
      setExternalConflict(false);
      setConflictLocalMarkdown(null);
      const h = hashFromSnap(snap);
      if (h) setBaseContentHash(h);
      editorRef.current?.setMarkdown?.(snap.content);
      void refreshRunReview(snap.meta.path);
      pendingRehydrateRef.current = true;
      queueMicrotask(() => {
        tryRehydrateMarks();
        if (vp) editorRef.current?.restoreViewportState?.(vp);
        endProgrammaticUpdateSoon();
      });
    },
    [applyDocument, beginProgrammaticUpdate, endProgrammaticUpdateSoon, refreshRunReview, tryRehydrateMarks],
  );

  const loadPath = useCallback(
    async (filePath: string) => {
      try {
        applyDocument(await openDocument(filePath), true);
        setStatus(`Opened ${filePath.split(/[/\\]/).pop() ?? filePath}`);
      } catch (e) {
        setStatus(`Open failed: ${e}`);
      }
    },
    [applyDocument],
  );

  // Mount: freeze collab; open initial path only (no welcome.md / workspace).
  useEffect(() => {
    let cancelled = false;
    const cfg = resolveSessionConfig();
    setSessionCfg({ roomId: cfg.roomId, syncUrl: null });
    void (async () => {
      if (cancelled) return;
      if (initialPath) {
        await loadPath(initialPath);
        return;
      }
      if (isTauriRuntime()) {
        const startup = await takeStartupPath();
        if (cancelled) return;
        if (startup) {
          await loadPath(startup);
          return;
        }
      }
      if (!cancelled) {
        setStatus(
          "Legacy document mode · open a Markdown file (protocol runs belong in Workspace)",
        );
      }
    })();

    const unlisten = onFileChanged((ev) => {
      if (cancelled) return;
      const run = async () => {
        const gen = ++watchGenRef.current;
        const current = docRef.current;
        if (!current || !ev.documentId || ev.documentId !== current.meta.id) return;

        const kind = classifyDiskEvent({
          event: ev,
          openDocumentId: current.meta.id,
          knownPersistedHash: baseHashRef.current,
          lastHandledExternalHash: lastHandledRef.current,
          dirty: dirtyRef.current,
          saving: savingRef.current,
        });

        if (
          kind === "ignore_same_hash" ||
          kind === "ignore_duplicate" ||
          kind === "ignore_sidecar" ||
          kind === "ignore_while_saving"
        ) {
          return;
        }

        const diskHash = ev.diskContentHash ?? null;
        if (diskHash) setLastHandledExternalHash(diskHash);

        if (kind === "external_dirty") {
          setExternalConflict((prev) => {
            if (!prev) {
              setConflictLocalMarkdown(
                editorRef.current?.getMarkdownContent?.() ?? markdownRef.current,
              );
              setStatus(
                "File changed on disk while you have local edits. Reload from disk or copy your text before Save.",
              );
            }
            return true;
          });
          return;
        }

        if (gen !== watchGenRef.current) return;
        try {
          const snap = await reloadDocument(current.meta.id);
          if (gen !== watchGenRef.current || cancelled) return;
          applyDocumentInPlace(snap);
          setStatus("Updated from disk");
        } catch (e) {
          if (!cancelled) setStatus(`Reload failed: ${e}`);
        }
      };

      const chain = watchInFlightRef.current
        ? watchInFlightRef.current.then(run)
        : run();
      watchInFlightRef.current = chain.finally(() => {
        watchInFlightRef.current = null;
      });
    });

    return () => {
      cancelled = true;
      unlisten();
      clearSaveTimer();
      unsubCommentsRef.current?.();
      sessionRef.current?.destroy();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- mount-only init
  }, []);

  const handleSave = useCallback(
    async (fromAutosave = false) => {
      const current = docRef.current;
      if (!current) return;
      if (fromAutosave && (remotePeerCount(peerCount + 1) > 0 || savingRef.current)) {
        // hasRemotePeers uses peerCount; use ref for peers via peerCount state
      }
      if (fromAutosave && (peerCount > 0 || savingRef.current)) return;

      // Protocol runs: no free-form dirty/save for canonical claims.
      if (isProtocolRunMarkdown(markdownRef.current)) {
        if (!fromAutosave) {
          setStatus(
            "Protocol run: claims are append-only. Use Add observation / Amend — not free-form Save.",
          );
        }
        setDirty(false);
        return;
      }

      if (!isTauri) {
        setDirty(false);
        setStatus(fromAutosave ? "Autosaved (browser)" : "Saved (browser; comments session-only)");
        return;
      }

      if (externalConflict && baseHashRef.current) {
        setConflictLocalMarkdown(
          editorRef.current?.getMarkdownContent?.() ?? markdownRef.current,
        );
        setStatus(
          "Cannot Save: file changed on disk. Reload from disk (local text kept for recovery) or resolve the conflict.",
        );
        return;
      }

      const md = editorRef.current?.getMarkdownContent?.() ?? markdownRef.current;
      setSaving(true);
      try {
        const snap = await saveDocument(current.meta.id, md, true, baseHashRef.current);
        setDoc(snap);
        const savedHash = hashFromSnap(snap);
        if (savedHash) {
          setBaseContentHash(savedHash);
          setLastHandledExternalHash(null);
        }
        const now = editorRef.current?.getMarkdownContent?.() ?? markdownRef.current;
        if (now === md) {
          setMarkdown(snap.content);
          setDirty(false);
          setStatus(
            fromAutosave
              ? "Autosaved"
              : peerCount > 0
                ? "Saved (host; autosave paused for peers)"
                : "Saved",
          );
        } else {
          setMarkdown(now);
          setDirty(true);
          setStatus("Saved; newer edits still pending");
        }

        // Reconcile annotations
        const s = sessionRef.current;
        if (s) {
          const list = listComments(commentsMap(s.doc), true).map(recordToDto);
          try {
            const res = await reconcileSessionAnnotations(snap.meta.path, list);
            const map = commentsMap(s.doc);
            for (const c of res.comments) {
              applyDurableRecord(map, dtoToRecord(c));
            }
            if (res.conflicts.length) {
              setStatus(
                `Annotation conflict(s): ${res.conflicts.length}. Refreshed durable state; review latest revisions.`,
              );
            }
          } catch (e) {
            setStatus(`error: could not reconcile annotations (${e})`);
          }
        }

        try {
          setRunReview(await getRunReview(snap.meta.path));
        } catch {
          /* non-fatal */
        }
        setExternalConflict(false);
        if (!fromAutosave) {
          setStatus((prev) => `${prev ?? "Saved"}; ledger: ${snap.meta.path}.moraine.json`);
        }
        if (historyOpen) {
          try {
            setHistoryEntries(await historyList(snap.meta.path));
          } catch {
            /* ignore */
          }
        }
      } catch (e) {
        if (isRevisionConflictError(e)) {
          setExternalConflict(true);
          setConflictLocalMarkdown(md);
          setStatus(
            "Revision conflict on Save: disk content changed. Reload from disk; local text is retained for recovery.",
          );
        } else {
          setStatus(`error: save failed (${e})`);
        }
      } finally {
        setSaving(false);
      }
    },
    [externalConflict, historyOpen, peerCount],
  );

  const scheduleSave = useCallback(() => {
    clearSaveTimer();
    if (!canAutosave(isTauri, peerCount > 0, true, savingRef.current)) return;
    saveTimerRef.current = setTimeout(() => {
      void handleSave(true);
    }, AUTOSAVE_MS);
  }, [clearSaveTimer, handleSave, peerCount]);

  const onEditorUpdate = useCallback(
    (md: string) => {
      if (ignoreProgrammaticRef.current) return;
      if (md === markdownRef.current) return;
      // Protocol runs: claims are not free-form editable; ignore buffer drift for dirty/save.
      if (isProtocolRunMarkdown(md) || isProtocolRunMarkdown(markdownRef.current)) {
        setMarkdown(md);
        return;
      }
      setMarkdown(md);
      setDirty(true);
      setRunReview((rr) => {
        if (rr?.decisionCurrent && rr.latest) {
          return { ...rr, decisionCurrent: false, reviewState: "stale" };
        }
        return rr;
      });
      scheduleSave();
    },
    [scheduleSave],
  );

  const onEditorReady = useCallback(() => {
    tryRehydrateMarks();
  }, [tryRehydrateMarks]);

  async function handleOpen() {
    if (!isTauri) {
      setStatus("File dialogs require the Tauri desktop app");
      return;
    }
    const picked = await pickMarkdownFile();
    if (picked) await loadPath(picked);
  }

  async function reloadFromDiskKeepingLocalCopy() {
    if (!docRef.current || !isTauri) return;
    setConflictLocalMarkdown(
      editorRef.current?.getMarkdownContent?.() ?? markdownRef.current,
    );
    try {
      const snap = await reloadDocument(docRef.current.meta.id);
      applyDocumentInPlace(snap);
      setStatus(
        conflictLocalMarkdown
          ? "Reloaded from disk. Your previous local text is kept in memory for copy (conflict buffer)."
          : "Updated from disk",
      );
    } catch (e) {
      setStatus(`Reload failed: ${e}`);
    }
  }

  function previewQuote(q: string, max = 48): string {
    const t = q.replace(/\s+/g, " ").trim();
    return t.length <= max ? t : `${t.slice(0, max)}…`;
  }

  async function addAnnotation(kind: "comment" | "suggestion") {
    if (!sessionRef.current || !editorRef.current) return;
    const quote = editorRef.current.getSelectionQuote?.();
    if (!quote) {
      setStatus(
        kind === "suggestion"
          ? "Select text first, then Suggest"
          : "Select text first, then Comment",
      );
      return;
    }
    if (kind === "suggestion" && editorRef.current.selectionTouchesManagedRegion?.()) {
      setStatus(
        "Suggestions cannot rewrite Moraine-managed regions (above Human notes). Add a comment, or edit free-form text only under Human notes.",
      );
      return;
    }
    const body =
      kind === "suggestion"
        ? window.prompt(
            `Suggest replacement for “${previewQuote(quote)}”\n(leave empty to delete that text)`,
            quote,
          )
        : window.prompt(`Comment on “${previewQuote(quote)}”`, "");
    if (body == null) {
      setStatus("Cancelled");
      return;
    }
    if (kind === "comment" && !body.trim()) {
      setStatus("Comment text is empty");
      return;
    }
    const id = newCommentId();
    if (!editorRef.current.applyCommentMark?.(id, kind)) {
      setStatus("Could not attach highlight");
      return;
    }
    const provisional: CommentRecord = {
      id,
      body: body.trim(),
      author: localAuthor,
      quote,
      createdAt: new Date().toISOString(),
      resolved: false,
      kind,
      revision: 1,
    };
    upsertComment(commentsMap(sessionRef.current.doc), provisional);
    setCommentsOpen(true);
    setHistoryOpen(false);

    if (isTauri && docRef.current) {
      try {
        const op = await createAnnotation(
          docRef.current.meta.path,
          id,
          provisional.body,
          provisional.author,
          provisional.quote,
          kind,
        );
        applyDurableRecord(commentsMap(sessionRef.current.doc), dtoToRecord(op.annotation));
      } catch (e) {
        commentsMap(sessionRef.current.doc).delete(id);
        editorRef.current?.clearCommentMark?.(id);
        setStatus(`error: could not create annotation (${e})`);
        return;
      }
    }
    const open = countPending(listComments(commentsMap(sessionRef.current.doc), true));
    setStatus(
      kind === "suggestion"
        ? `Suggestion added; ${open.suggestions} pending`
        : isTauri
          ? `Comment added; ${open.comments} open`
          : "Comment added (browser: session only)",
    );
  }

  async function applyOpResult(op: { annotation: CommentDto }) {
    if (!sessionRef.current) return;
    applyDurableRecord(commentsMap(sessionRef.current.doc), dtoToRecord(op.annotation));
  }

  async function refreshAnnotationsFromDisk() {
    if (!isTauri || !docRef.current || !sessionRef.current) return;
    try {
      const disk = await loadComments(docRef.current.meta.path);
      const map = commentsMap(sessionRef.current.doc);
      for (const c of disk) applyDurableRecord(map, dtoToRecord(c));
    } catch {
      /* ignore */
    }
  }

  async function resolveComment(id: string) {
    if (!sessionRef.current) return;
    const rec = commentsMap(sessionRef.current.doc).get(id);
    if (!rec) return;
    const prev = { ...rec };
    editorRef.current?.clearCommentMark?.(id);
    if (isTauri && docRef.current) {
      try {
        const op = await resolveAnnotation(docRef.current.meta.path, id, rec.revision ?? 1);
        await applyOpResult(op);
        setStatus("Comment resolved");
      } catch (e) {
        upsertComment(commentsMap(sessionRef.current.doc), prev);
        if (isAnnotationConflictError(e)) {
          await refreshAnnotationsFromDisk();
          setStatus("Annotation conflict: refreshed from disk. Resolve again if still needed.");
        } else {
          setStatus(`error: could not resolve (${e})`);
        }
      }
    } else {
      upsertComment(commentsMap(sessionRef.current.doc), { ...rec, resolved: true });
      setStatus("Comment resolved");
    }
  }

  async function reopenComment(id: string) {
    if (!sessionRef.current) return;
    const rec = commentsMap(sessionRef.current.doc).get(id);
    if (!rec) return;
    const prev = { ...rec };
    if (isTauri && docRef.current) {
      try {
        const op = await reopenAnnotation(docRef.current.meta.path, id, rec.revision ?? 1);
        await applyOpResult(op);
        setStatus("Thread reopened");
      } catch (e) {
        upsertComment(commentsMap(sessionRef.current.doc), prev);
        if (isAnnotationConflictError(e)) {
          await refreshAnnotationsFromDisk();
          setStatus("Annotation conflict: refreshed from disk.");
        } else {
          setStatus(`error: could not reopen (${e})`);
        }
      }
    } else {
      upsertComment(commentsMap(sessionRef.current.doc), { ...rec, resolved: false });
      setStatus("Thread reopened");
    }
  }

  async function rejectSuggestion(id: string) {
    if (!sessionRef.current) return;
    const rec = commentsMap(sessionRef.current.doc).get(id);
    if (!rec || rec.kind !== "suggestion") return;
    const prev = { ...rec };
    editorRef.current?.clearCommentMark?.(id);
    if (isTauri && docRef.current) {
      try {
        const op = await rejectSuggestionApi(docRef.current.meta.path, id, rec.revision ?? 1);
        await applyOpResult(op);
      } catch (e) {
        upsertComment(commentsMap(sessionRef.current.doc), prev);
        if (isAnnotationConflictError(e)) {
          await refreshAnnotationsFromDisk();
          setStatus("Annotation conflict: refreshed from disk.");
        } else {
          setStatus(`error: could not reject (${e})`);
        }
        return;
      }
    } else {
      upsertComment(commentsMap(sessionRef.current.doc), { ...rec, resolved: true });
    }
    setOrphanedMarkIds((ids) => ids.filter((x) => x !== id));
    const left = countPending(listComments(commentsMap(sessionRef.current.doc), true)).suggestions;
    setStatus(left > 0 ? `Suggestion rejected; ${left} still pending` : "Suggestion rejected");
  }

  async function acceptSuggestion(id: string) {
    if (!sessionRef.current || !docRef.current) return;
    const rec = commentsMap(sessionRef.current.doc).get(id);
    if (!rec || rec.kind !== "suggestion") return;
    if (rec.disposition === "accepting") {
      setStatus(
        "This suggestion has an incomplete acceptance. Use Cancel acceptance or complete recovery after Save.",
      );
      return;
    }
    if (editorRef.current?.suggestionTargetsManagedRegion?.(id, rec.quote)) {
      setStatus(
        "Cannot accept suggestion: it targets Moraine-managed content (above Human notes). Use comments instead.",
      );
      return;
    }
    if (!isTauri) {
      const ok = editorRef.current?.acceptSuggestion?.(id, rec.body, rec.quote) ?? false;
      if (!ok) {
        setStatus("Accept failed: quoted text not found in document");
        return;
      }
      upsertComment(commentsMap(sessionRef.current.doc), {
        ...rec,
        resolved: true,
        disposition: "accepted",
      });
      setDirty(true);
      setStatus("Suggestion accepted (browser session only)");
      return;
    }

    const expectedHash = baseHashRef.current;
    if (!expectedHash) {
      setStatus("Save the document first so acceptance can bind to a content revision.");
      return;
    }
    let begin;
    try {
      begin = await beginAcceptSuggestion(
        docRef.current.meta.path,
        id,
        rec.revision ?? 1,
        expectedHash,
      );
      await applyOpResult(begin);
    } catch (e) {
      if (isAnnotationConflictError(e)) {
        await refreshAnnotationsFromDisk();
        setStatus("Could not begin accept (conflict or content hash mismatch). Refreshed from disk.");
      } else {
        setStatus(`Could not begin accept (${e})`);
      }
      return;
    }

    const ok = editorRef.current?.acceptSuggestion?.(id, rec.body, rec.quote) ?? false;
    if (!ok) {
      try {
        const cancelled = await cancelAcceptSuggestion(
          docRef.current.meta.path,
          id,
          begin.annotation.revision ?? 2,
          begin.acceptanceOpId,
        );
        await applyOpResult(cancelled);
      } catch {
        /* best effort */
      }
      setOrphanedMarkIds((ids) => [...new Set([...ids, id])]);
      setStatus("Accept cancelled: quoted text not found. Reservation released.");
      return;
    }

    setDirty(true);
    await handleSave(false);
    if (dirtyRef.current || externalConflict) {
      try {
        const cancelled = await cancelAcceptSuggestion(
          docRef.current.meta.path,
          id,
          begin.annotation.revision ?? 2,
          begin.acceptanceOpId,
        );
        await applyOpResult(cancelled);
      } catch {
        setStatus(
          "Save failed after reservation. Suggestion remains incomplete (accepting). Cancel or retry.",
        );
        return;
      }
      setStatus("Save failed; acceptance cancelled. Markdown not finalized as accepted.");
      return;
    }

    const savedHash = baseHashRef.current ?? runReview?.contentHash;
    if (!savedHash) {
      setStatus("Missing saved content hash; suggestion left incomplete. Cancel or complete after Save.");
      return;
    }
    try {
      const cur = commentsMap(sessionRef.current.doc).get(id);
      const expectRev = cur?.revision ?? begin.annotation.revision ?? 1;
      const op = await completeAcceptSuggestion(
        docRef.current.meta.path,
        id,
        expectRev,
        begin.acceptanceOpId,
        savedHash,
      );
      await applyOpResult(op);
      setOrphanedMarkIds((ids) => ids.filter((x) => x !== id));
      const left = countPending(listComments(commentsMap(sessionRef.current.doc), true)).suggestions;
      setStatus(left > 0 ? `Suggestion accepted; ${left} still pending` : "Suggestion accepted");
    } catch (e) {
      await refreshAnnotationsFromDisk();
      setStatus(
        `Incomplete acceptance: finalize failed (${e}). Cancel or retry after checking the document.`,
      );
    }
  }

  async function cancelIncompleteAcceptance(id: string) {
    if (!sessionRef.current || !docRef.current || !isTauri) return;
    const rec = commentsMap(sessionRef.current.doc).get(id);
    if (!rec?.acceptanceOpId) {
      setStatus("No active acceptance to cancel.");
      return;
    }
    setRecoveryBusy(true);
    try {
      const st = await getAcceptanceRecoveryStatus(docRef.current.meta.path, id);
      if (!st.cancelSafe) {
        await refreshAnnotationsFromDisk();
        await refreshRunReview(docRef.current.meta.path);
        setStatus(
          "Cannot cancel: Markdown changed after acceptance began. Finalize against the saved document, or restore the original revision first.",
        );
        return;
      }
      const op = await cancelAcceptSuggestion(
        docRef.current.meta.path,
        id,
        rec.revision ?? st.revision,
        rec.acceptanceOpId,
      );
      await applyOpResult(op);
      setStatus("Acceptance cancelled; suggestion is pending again.");
    } catch (e) {
      await refreshAnnotationsFromDisk();
      if (docRef.current) await refreshRunReview(docRef.current.meta.path);
      setStatus(`Could not cancel acceptance (${e})`);
    } finally {
      setRecoveryBusy(false);
    }
  }

  async function finalizeIncompleteAcceptance(id: string) {
    if (!sessionRef.current || !docRef.current || !isTauri) return;
    setRecoveryBusy(true);
    try {
      const st = await getAcceptanceRecoveryStatus(docRef.current.meta.path, id);
      if (!st.acceptanceOpId) {
        setStatus("No acceptance operation to finalize.");
        return;
      }
      if (st.cancelSafe) {
        setStatus(
          "Document still matches the base revision. Cancel acceptance if the suggestion was not applied, or apply and Save before finalizing.",
        );
        return;
      }
      const op = await completeAcceptSuggestion(
        docRef.current.meta.path,
        id,
        st.revision,
        st.acceptanceOpId,
        st.currentContentHash,
      );
      await applyOpResult(op);
      await refreshRunReview(docRef.current.meta.path);
      setStatus("Acceptance finalized for the current saved document revision.");
    } catch (e) {
      await refreshAnnotationsFromDisk();
      if (docRef.current) await refreshRunReview(docRef.current.meta.path);
      setStatus(`Could not finalize acceptance (${e})`);
    } finally {
      setRecoveryBusy(false);
    }
  }

  async function refreshAcceptanceRecovery(id: string) {
    if (!docRef.current || !isTauri) return;
    setRecoveryBusy(true);
    try {
      await refreshAnnotationsFromDisk();
      await refreshRunReview(docRef.current.meta.path);
      const st = await getAcceptanceRecoveryStatus(docRef.current.meta.path, id);
      setStatus(
        st.cancelSafe
          ? "Recovery status: Markdown still at base hash (Cancel is safe)."
          : "Recovery status: Markdown differs from base (Finalize or restore original).",
      );
    } catch (e) {
      setStatus(`Could not refresh recovery status (${e})`);
    } finally {
      setRecoveryBusy(false);
    }
  }

  async function refreshHistory() {
    if (!docRef.current) return;
    setHistoryLoading(true);
    try {
      setHistoryEntries(await historyList(docRef.current.meta.path));
    } catch (e) {
      setStatus(`History failed: ${e}`);
      setHistoryEntries([]);
    } finally {
      setHistoryLoading(false);
    }
  }

  async function toggleHistory() {
    setHistoryOpen((open) => {
      const next = !open;
      if (next) {
        setCommentsOpen(false);
        void refreshHistory();
      }
      return next;
    });
  }

  async function restoreEntry(id: string) {
    if (!docRef.current) return;
    try {
      const content = await historyRestoreContent(docRef.current.meta.path, id);
      applyDocument(
        {
          ...docRef.current,
          content,
          meta: { ...docRef.current.meta, dirty: true, byteLen: content.length },
        },
        true,
      );
      setStatus("Restored snapshot (unsaved)");
    } catch (e) {
      setStatus(`Restore failed: ${e}`);
    }
  }

  return (
    <div className="flex h-screen flex-col" data-testid="legacy-document-route">
      <Toolbar
        title={title}
        path={path}
        dirty={legacyDocumentMode ? dirty : false}
        saving={saving}
        viewMode={viewMode}
        historyOpen={historyOpen}
        commentsOpen={commentsOpen}
        remotePeers={0}
        isTauri={isTauri}
        onOpen={() => void handleOpen()}
        onSave={() => void handleSave(false)}
        onComment={() => void addAnnotation("comment")}
        onSuggest={() => void addAnnotation("suggestion")}
        onToggleComments={() => {
          setCommentsOpen((o) => {
            const next = !o;
            if (next) setHistoryOpen(false);
            return next;
          });
        }}
        onToggleHistory={() => void toggleHistory()}
        onViewMode={setViewMode}
      />

      <div
        className="flex border-b px-3 py-1 text-[11px] gap-2 items-center"
        style={{ borderColor: "var(--border)", background: "var(--panel)" }}
      >
        {onBackToWorkspace ? (
          <button
            type="button"
            className="rounded px-2 py-0.5 font-medium"
            style={{
              background: "var(--accent-soft)",
              color: "var(--accent)",
              border: "1px solid var(--border)",
            }}
            onClick={onBackToWorkspace}
          >
            ← Ledger workspace
          </button>
        ) : null}
        <span style={{ color: "#b45309" }}>
          Legacy document route · free-form editing · live collab frozen
        </span>
        {doc && isProtocolRun ? (
          <span style={{ color: "var(--muted)" }}>
            Protocol run opened here · prefer Workspace ledger for append-only review
          </span>
        ) : null}
      </div>

      <>
      {isTauri ? (
        <>
          <RunReviewPanel
            review={runReview}
            externalConflict={externalConflict}
            onReload={() => void reloadFromDiskKeepingLocalCopy()}
          />
          {isProtocolRun ? (
            <ProtocolLedgerPanel
              path={path}
              refreshToken={findingsRefreshToken}
              onMutated={() => {
                setFindingsRefreshToken((t) => t + 1);
                if (docRef.current) {
                  void reloadDocument(docRef.current.meta.id).then((snap) => {
                    applyDocumentInPlace(snap);
                  });
                }
              }}
            />
          ) : null}
          <CheckpointFindingsPanel path={path} refreshToken={findingsRefreshToken} />
        </>
      ) : null}

      <div className="flex min-h-0 flex-1">
        <main className="flex min-w-0 flex-1">
          {viewMode === "edit" || viewMode === "split" ? (
            <div
              className={`min-w-0 ${viewMode === "edit" ? "w-full" : "w-1/2"} ${viewMode === "split" ? "border-r" : ""}`}
              style={{ borderColor: "var(--border)" }}
            >
              {session ? (
                <Editor
                  ref={editorRef}
                  session={session}
                  initialMarkdown={markdown}
                  editable={legacyDocumentMode}
                  onUpdate={onEditorUpdate}
                  onReady={onEditorReady}
                />
              ) : (
                <div className="p-6 text-sm" style={{ color: "var(--muted)" }}>
                  Loading editor…
                </div>
              )}
            </div>
          ) : null}

          {viewMode === "preview" || viewMode === "split" ? (
            <div className={`min-w-0 ${viewMode === "preview" ? "w-full" : "w-1/2"}`}>
              <Preview content={markdown} />
            </div>
          ) : null}
        </main>

        {commentsOpen ? (
          <CommentsPanel
            comments={commentList}
            orphanedIds={orphanedMarkIds}
            showResolved={showResolvedComments}
            currentDiskHash={baseContentHash}
            recoveryBusy={recoveryBusy}
            onToggleShowResolved={() => setShowResolvedComments((v) => !v)}
            onResolve={(id) => void resolveComment(id)}
            onReopen={(id) => void reopenComment(id)}
            onAccept={(id) => void acceptSuggestion(id)}
            onReject={(id) => void rejectSuggestion(id)}
            onCancelAccept={(id) => void cancelIncompleteAcceptance(id)}
            onFinalizeAccept={(id) => void finalizeIncompleteAcceptance(id)}
            onRefreshRecovery={(id) => void refreshAcceptanceRecovery(id)}
            onFocus={(id) => editorRef.current?.focusComment?.(id)}
            onClose={() => setCommentsOpen(false)}
          />
        ) : null}

        {historyOpen ? (
          <HistoryPanel
            entries={historyEntries}
            loading={historyLoading}
            onRestore={(id) => void restoreEntry(id)}
            onRefresh={() => void refreshHistory()}
            onClose={() => setHistoryOpen(false)}
          />
        ) : null}
      </div>
      </>

      <StatusBar
        wordCount={wordCount}
        charCount={charCount}
        collabPeers={0}
        peerNames=""
        roomId={null}
        autosavePaused={false}
        pendingComments={pending.comments}
        pendingSuggestions={pending.suggestions}
        orphanedMarks={orphanedMarkIds.length}
        message={status}
      />
    </div>
  );
}
