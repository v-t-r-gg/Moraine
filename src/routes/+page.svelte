<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import Toolbar from "$lib/components/Toolbar.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import Preview from "$lib/components/Preview.svelte";
  import HistoryPanel from "$lib/components/HistoryPanel.svelte";
  import CommentsPanel from "$lib/components/CommentsPanel.svelte";
  import RunReviewPanel from "$lib/components/RunReviewPanel.svelte";
  import Editor from "$lib/components/Editor.svelte";
  import {
    appInfo,
    beginAcceptSuggestion,
    cancelAcceptSuggestion,
    completeAcceptSuggestion,
    createAnnotation,
    getAcceptanceRecoveryStatus,
    getRunReview,
    historyList,
    historyRestoreContent,
    isTauri,
    loadComments,
    onFileChanged,
    openDocument,
    pickMarkdownFile,
    recordRunDecision,
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
  } from "$lib/api";
  import {
    createYjsSession,
    resolveSessionConfig,
    roomIdForPath,
    type SessionConfig,
    type YjsSession,
  } from "$lib/editor/yjsSession";
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
  } from "$lib/editor/comments";
  import {
    AUTOSAVE_MS,
    canAutosave,
    peerNames,
    remotePeerCount,
  } from "$lib/editor/hostSave";
  import { isRevisionConflictError } from "$lib/editor/reviewGate";
  import { classifyDiskEvent, type ViewportState } from "$lib/editor/diskWatch";
  import type { DocumentSnapshot, HistoryEntryMeta, ViewMode } from "$lib/types";

  const WELCOME_MD = `# Agent run record

This is a **run record**: a durable Markdown log of agent work for human review.

## How to use

1. Agents write or update \`.md\` files (CLI or any tool).
2. Optional live room: \`moraine share this-file.md\` (relay must be running).
3. Humans open the file or join URL, then **Comment** / **Suggest**, **Review**, **Save**.
4. Use **Run review** (Approve / Request changes / Reject) for the whole record.
5. Annotations + decisions persist in \`file.md.moraine.json\` on host Save.

See the project README and VISION.md for the full model.
`;

  let doc = $state<DocumentSnapshot | null>(null);
  let markdown = $state("");
  let dirty = $state(false);
  let saving = $state(false);
  let viewMode = $state<ViewMode>("edit");
  let historyOpen = $state(false);
  let commentsOpen = $state(false);
  let showResolvedComments = $state(false);
  let historyEntries = $state<HistoryEntryMeta[]>([]);
  let historyLoading = $state(false);
  let commentList = $state<CommentRecord[]>([]);
  let orphanedMarkIds = $state<string[]>([]);
  let status = $state<string | null>(null);
  let session = $state<YjsSession | null>(null);
  let peerCount = $state(0);
  let peerLabel = $state("");
  let editorRef = $state<Editor | undefined>(undefined);
  let sessionCfg = $state<SessionConfig>({ roomId: null, syncUrl: null });
  let localAuthor = $state("You");
  let runReview = $state<RunReviewDto | null>(null);
  let reviewBusy = $state(false);
  let recoveryBusy = $state(false);
  /** Hash of the last known persisted Markdown revision (from disk load/save). */
  let baseContentHash = $state<string | null>(null);
  /** External disk change detected while this session holds a different base. */
  let externalConflict = $state(false);
  /** Local text preserved when an external conflict blocks overwrite. */
  let conflictLocalMarkdown = $state<string | null>(null);
  /** Last external disk hash we already handled (dedupe reload/conflict). */
  let lastHandledExternalHash = $state<string | null>(null);

  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let unlistenFile: (() => void) | null = null;
  let unsubComments: (() => void) | null = null;
  /** Generation token for programmatic editor updates (reload/open). */
  let programmaticUpdateGen = 0;
  let ignoreProgrammaticUpdates = false;
  let prevPeers = 0;
  let pendingRehydrate = false;
  /** Serialize watcher handlers for one document. */
  let watchInFlight: Promise<void> | null = null;
  let watchGeneration = 0;

  const title = $derived(doc?.meta.title ?? "Moraine");
  const path = $derived(doc?.meta.path ?? null);
  const wordCount = $derived(countWords(markdown));
  const charCount = $derived(markdown.length);
  const hasRemotePeers = $derived(peerCount > 0);
  const pending = $derived(countPending(commentList));

  onMount(async () => {
    sessionCfg = resolveSessionConfig();
    try {
      const info = await appInfo();
      status = [info.name, info.version, !isTauri ? "browser" : null, sessionCfg.roomId]
        .filter(Boolean)
        .join(" · ");
    } catch {
      status = "Moraine";
    }
    await loadInitial();
    unlistenFile = onFileChanged((ev) => {
      void handleDiskWatchEvent(ev);
    });
  });

  async function handleDiskWatchEvent(ev: import("$lib/types").FileChangedEvent) {
    // Serialize handlers so bursts do not stack reloads.
    const run = async () => {
      const gen = ++watchGeneration;
      if (!doc || !ev.documentId || ev.documentId !== doc.meta.id) return;

      const kind = classifyDiskEvent({
        event: ev,
        openDocumentId: doc.meta.id,
        knownPersistedHash: baseContentHash,
        lastHandledExternalHash,
        dirty,
        saving,
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
      if (diskHash) lastHandledExternalHash = diskHash;

      if (kind === "external_dirty") {
        // Stable conflict: set once, no auto-reload, no session reset.
        if (!externalConflict) {
          externalConflict = true;
          conflictLocalMarkdown = editorRef?.getMarkdownContent?.() ?? markdown;
          status =
            "File changed on disk while you have local edits. Reload from disk or copy your text before Save.";
        }
        return;
      }

      // external_clean: reload once in place without conflict language
      if (gen !== watchGeneration) return;
      try {
        const snap = await reloadDocument(doc.meta.id);
        if (gen !== watchGeneration) return;
        applyDocumentInPlace(snap);
        status = "Updated from disk";
      } catch (e) {
        status = `Reload failed: ${e}`;
      }
    };

    if (watchInFlight) {
      await watchInFlight;
    }
    watchInFlight = run().finally(() => {
      watchInFlight = null;
    });
    await watchInFlight;
  }

  onDestroy(() => {
    clearSaveTimer();
    unlistenFile?.();
    unsubComments?.();
    session?.destroy();
  });

  async function loadInitial() {
    if (isTauri) {
      const startup = await takeStartupPath();
      if (startup) {
        await loadPath(startup);
        return;
      }
      const demo = "/tmp/moraine-welcome.md";
      try {
        await writeFile(demo, WELCOME_MD);
      } catch {
        /* create on open */
      }
      await loadPath(demo);
    } else {
      await loadPath("welcome.md");
    }
  }

  async function loadPath(filePath: string) {
    try {
      applyDocument(await openDocument(filePath), true);
      status = `Opened ${doc?.meta.title ?? filePath}`;
      if (historyOpen) await refreshHistory();
    } catch (e) {
      status = `Open failed: ${e}`;
      if (!doc) {
        applyDocument(
          {
            meta: {
              id: crypto.randomUUID(),
              path: "untitled.md",
              title: "untitled.md",
              dirty: true,
              lastSavedAt: null,
              lastModifiedOnDisk: null,
              byteLen: WELCOME_MD.length,
            },
            content: WELCOME_MD,
          },
          true,
        );
      }
    }
  }

  function hashFromSnap(snap: DocumentSnapshot): string | null {
    if (snap.contentHash && snap.contentHash.length === 64) return snap.contentHash;
    return null;
  }

  /** Open a different document (or first open): may reset collab session. */
  function applyDocument(snap: DocumentSnapshot, resetSession: boolean) {
    beginProgrammaticUpdate();
    doc = snap;
    markdown = snap.content;
    dirty = snap.meta.dirty;
    externalConflict = false;
    conflictLocalMarkdown = null;
    const h = hashFromSnap(snap);
    if (h) baseContentHash = h;
    lastHandledExternalHash = null;

    if (resetSession) {
      session?.destroy();
      unsubComments?.();
      unsubComments = null;
      clearSaveTimer();

      const room = sessionCfg.roomId ?? roomIdForPath(snap.meta.path);
      const s = createYjsSession(room, { syncUrl: sessionCfg.syncUrl });
      session = s;
      peerCount = 0;
      prevPeers = 0;
      peerLabel = "";
      localAuthor =
        (s.awareness.getLocalState()?.user as { name?: string } | undefined)?.name ?? "You";

      const cmap = commentsMap(s.doc);
      const refresh = () => {
        commentList = listComments(cmap, true);
      };
      refresh();
      cmap.observe(refresh);
      unsubComments = () => cmap.unobserve(refresh);

      s.awareness.on("change", () => {
        const size = s.awareness.getStates().size;
        const next = remotePeerCount(size);
        if (next !== prevPeers) {
          if (next > 0 && prevPeers === 0) clearSaveTimer();
          if (next === 0 && prevPeers > 0 && dirty) scheduleSave();
          prevPeers = next;
        }
        peerCount = next;
        peerLabel = peerNames(s.awareness.getStates() as Map<number, Record<string, unknown>>, s.doc.clientID).join(", ");
      });

      void seedCommentsFromDisk(snap.meta.path, cmap);
      void refreshRunReview(snap.meta.path);
    }

    endProgrammaticUpdateSoon();
  }

  /**
   * Same-file revision reload: keep Yjs session, avoid reseeding, restore viewport.
   */
  function applyDocumentInPlace(snap: DocumentSnapshot) {
    const samePath = doc?.meta.path === snap.meta.path;
    if (!samePath || !session) {
      applyDocument(snap, true);
      return;
    }
    const vp: ViewportState | undefined = editorRef?.getViewportState?.();
    beginProgrammaticUpdate();
    doc = snap;
    markdown = snap.content;
    dirty = false;
    externalConflict = false;
    conflictLocalMarkdown = null;
    const h = hashFromSnap(snap);
    if (h) baseContentHash = h;
    editorRef?.setMarkdown?.(snap.content);
    void refreshRunReview(snap.meta.path);
    // Rehydrate marks for open annotations without destroying session.
    pendingRehydrate = true;
    queueMicrotask(() => {
      tryRehydrateMarks();
      if (vp) editorRef?.restoreViewportState?.(vp);
      endProgrammaticUpdateSoon();
    });
  }

  function beginProgrammaticUpdate() {
    programmaticUpdateGen += 1;
    ignoreProgrammaticUpdates = true;
  }

  function endProgrammaticUpdateSoon() {
    const gen = programmaticUpdateGen;
    // Editor may emit updates async; hold suppression across a couple of frames.
    queueMicrotask(() => {
      requestAnimationFrame(() => {
        if (gen === programmaticUpdateGen) {
          ignoreProgrammaticUpdates = false;
        }
      });
    });
  }

  async function refreshRunReview(filePath: string) {
    if (!isTauri) {
      runReview = null;
      return;
    }
    try {
      runReview = await getRunReview(filePath);
      // Do not overwrite a fresher base from Save with a racey stale read.
      if (runReview.contentHash) {
        baseContentHash = runReview.contentHash;
      }
    } catch (e) {
      status = `error: could not load run review (${e})`;
    }
  }

  async function onRunDecide(decision: string, reviewer: string, reason: string) {
    if (!doc || !isTauri) return;
    if (dirty || externalConflict || saving) {
      status = dirty
        ? "Save the current revision before recording a review decision."
        : "Resolve the external file conflict before recording a decision.";
      return;
    }
    const expected = baseContentHash ?? runReview?.contentHash;
    if (!expected) {
      status = "No persisted content hash. Save the file first.";
      return;
    }
    reviewBusy = true;
    try {
      runReview = await recordRunDecision(
        doc.meta.path,
        decision,
        reviewer,
        reason || null,
        expected,
      );
      baseContentHash = runReview.contentHash;
      status = `Run decision recorded: ${decision}`;
    } catch (e) {
      if (isRevisionConflictError(e)) {
        externalConflict = true;
        conflictLocalMarkdown = editorRef?.getMarkdownContent?.() ?? markdown;
        status =
          "Revision conflict: Markdown on disk changed. Reload from disk, then decide again.";
      } else {
        status = `error: could not record decision (${e})`;
      }
    } finally {
      reviewBusy = false;
    }
  }

  async function reloadFromDiskKeepingLocalCopy() {
    if (!doc || !isTauri) return;
    conflictLocalMarkdown = editorRef?.getMarkdownContent?.() ?? markdown;
    try {
      const snap = await reloadDocument(doc.meta.id);
      // Same path: keep session; do not force conflict language.
      applyDocumentInPlace(snap);
      status = conflictLocalMarkdown
        ? "Reloaded from disk. Your previous local text is kept in memory for copy (conflict buffer)."
        : "Updated from disk";
    } catch (e) {
      status = `Reload failed: ${e}`;
    }
  }

  async function seedCommentsFromDisk(filePath: string, cmap: ReturnType<typeof commentsMap>) {
    if (!isTauri) return;
    try {
      const disk = await loadComments(filePath);
      const records: CommentRecord[] = disk.map(dtoToRecord);
      mergeDiskIntoMap(cmap, records);
      commentList = listComments(cmap, true);
      pendingRehydrate = true;
      tryRehydrateMarks();
    } catch {
      /* no sidecar */
    }
  }

  function onEditorReady() {
    tryRehydrateMarks();
  }

  function tryRehydrateMarks() {
    if (!pendingRehydrate || !editorRef?.rehydrateMarks) return;
    pendingRehydrate = false;
    const open = commentList.filter((c) => !c.resolved);
    const { applied, orphaned } = editorRef.rehydrateMarks(open);
    orphanedMarkIds = orphaned;
    if (open.length === 0) return;
    const parts: string[] = [];
    const pend = countPending(open);
    if (pend.suggestions) {
      parts.push(
        `${pend.suggestions} suggestion${pend.suggestions === 1 ? "" : "s"} pending`,
      );
    }
    if (applied.length) parts.push(`${applied.length} mark(s) restored`);
    if (orphaned.length) {
      parts.push(`${orphaned.length} quote(s) not found in text`);
    }
    if (parts.length) status = parts.join("; ");
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

  async function reconcileCommentsFromSession() {
    if (!isTauri || !doc || !session) return;
    const list = listComments(commentsMap(session.doc), true).map(recordToDto);
    try {
      const res = await reconcileSessionAnnotations(doc.meta.path, list);
      const map = commentsMap(session.doc);
      for (const c of res.comments) {
        applyDurableRecord(map, dtoToRecord(c));
      }
      if (res.conflicts.length) {
        status = `Annotation conflict(s): ${res.conflicts.length}. Refreshed durable state; review latest revisions.`;
      }
    } catch (e) {
      status = `error: could not reconcile annotations (${e})`;
    }
  }

  async function applyOpResult(op: { annotation: CommentDto }) {
    if (!session) return;
    applyDurableRecord(commentsMap(session.doc), dtoToRecord(op.annotation));
  }

  async function refreshAnnotationsFromDisk() {
    if (!isTauri || !doc || !session) return;
    try {
      const disk = await loadComments(doc.meta.path);
      const map = commentsMap(session.doc);
      for (const c of disk) {
        applyDurableRecord(map, dtoToRecord(c));
      }
    } catch {
      /* ignore */
    }
  }

  function onEditorUpdate(md: string) {
    if (ignoreProgrammaticUpdates) return;
    if (md === markdown) return;
    markdown = md;
    dirty = true;
    if (runReview?.decisionCurrent && runReview.latest) {
      runReview = {
        ...runReview,
        decisionCurrent: false,
        reviewState: "stale",
      };
    }
    scheduleSave();
  }

  function clearSaveTimer() {
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
  }

  function scheduleSave() {
    clearSaveTimer();
    if (!canAutosave(isTauri, hasRemotePeers, true, saving)) return;
    saveTimer = setTimeout(() => {
      void handleSave(true);
    }, AUTOSAVE_MS);
  }

  async function handleOpen() {
    if (!isTauri) {
      status = "File dialogs require the Tauri desktop app";
      return;
    }
    const picked = await pickMarkdownFile();
    if (picked) await loadPath(picked);
  }

  async function handleSave(fromAutosave = false) {
    if (!doc) return;
    if (fromAutosave && (hasRemotePeers || saving)) return;

    if (!isTauri) {
      dirty = false;
      status = fromAutosave ? "Autosaved (browser)" : "Saved (browser; comments session-only)";
      return;
    }

    if (externalConflict && baseContentHash) {
      conflictLocalMarkdown = editorRef?.getMarkdownContent?.() ?? markdown;
      status =
        "Cannot Save: file changed on disk. Reload from disk (local text kept for recovery) or resolve the conflict.";
      return;
    }

    const md = editorRef?.getMarkdownContent?.() ?? markdown;
    saving = true;
    try {
      const snap = await saveDocument(
        doc.meta.id,
        md,
        true,
        baseContentHash,
      );
      doc = snap;
      // Authoritative saved hash from backend before watcher classification.
      const savedHash = hashFromSnap(snap);
      if (savedHash) {
        baseContentHash = savedHash;
        lastHandledExternalHash = null;
      }
      const now = editorRef?.getMarkdownContent?.() ?? markdown;
      if (now === md) {
        markdown = snap.content;
        dirty = false;
        status = fromAutosave
          ? "Autosaved"
          : hasRemotePeers
            ? "Saved (host; autosave paused for peers)"
            : "Saved";
      } else {
        markdown = now;
        dirty = true;
        status = "Saved; newer edits still pending";
        scheduleSave();
      }
      await reconcileCommentsFromSession();
      // Review state refresh; base hash already set from Save.
      if (isTauri) {
        try {
          runReview = await getRunReview(doc.meta.path);
        } catch {
          /* non-fatal */
        }
      }
      externalConflict = false;
      if (!fromAutosave && isTauri) {
        status = `${status}; ledger: ${doc.meta.path}.moraine.json`;
      }
      if (historyOpen) await refreshHistory();
    } catch (e) {
      if (isRevisionConflictError(e)) {
        externalConflict = true;
        conflictLocalMarkdown = md;
        status =
          "Revision conflict on Save: disk content changed. Reload from disk; local text is retained for recovery.";
      } else {
        status = `error: save failed (${e})`;
      }
    } finally {
      saving = false;
    }
  }

  function previewQuote(q: string, max = 48): string {
    const t = q.replace(/\s+/g, " ").trim();
    return t.length <= max ? t : `${t.slice(0, max)}…`;
  }

  async function addAnnotation(kind: "comment" | "suggestion") {
    if (!session || !editorRef) return;
    const quote = editorRef.getSelectionQuote?.();
    if (!quote) {
      status =
        kind === "suggestion"
          ? "Select text first, then Suggest"
          : "Select text first, then Comment";
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
      status = "Cancelled";
      return;
    }
    if (kind === "comment" && !body.trim()) {
      status = "Comment text is empty";
      return;
    }

    const id = newCommentId();
    if (!editorRef.applyCommentMark?.(id, kind)) {
      status = "Could not attach highlight";
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
    upsertComment(commentsMap(session.doc), provisional);
    commentsOpen = true;
    historyOpen = false;

    if (isTauri && doc) {
      try {
        const op = await createAnnotation(
          doc.meta.path,
          id,
          provisional.body,
          provisional.author,
          provisional.quote,
          kind,
        );
        await applyOpResult(op);
      } catch (e) {
        commentsMap(session.doc).delete(id);
        editorRef?.clearCommentMark?.(id);
        status = `error: could not create annotation (${e})`;
        return;
      }
    }

    const open = countPending(listComments(commentsMap(session.doc), true));
    if (kind === "suggestion") {
      status =
        body.trim() === ""
          ? `Suggestion added (delete “${previewQuote(quote)}”); ${open.suggestions} pending`
          : `Suggestion added; ${open.suggestions} pending`;
    } else {
      status = isTauri
        ? `Comment added; ${open.comments} open`
        : `Comment added (browser: session only)`;
    }
  }

  async function resolveComment(id: string) {
    if (!session) return;
    const rec = commentsMap(session.doc).get(id);
    if (!rec) return;
    const prev = { ...rec };
    editorRef?.clearCommentMark?.(id);
    if (isTauri && doc) {
      try {
        const op = await resolveAnnotation(doc.meta.path, id, rec.revision ?? 1);
        await applyOpResult(op);
        status = "Comment resolved";
      } catch (e) {
        upsertComment(commentsMap(session.doc), prev);
        if (isAnnotationConflictError(e)) {
          await refreshAnnotationsFromDisk();
          status = "Annotation conflict: refreshed from disk. Resolve again if still needed.";
        } else {
          status = `error: could not resolve (${e})`;
        }
      }
    } else {
      upsertComment(commentsMap(session.doc), { ...rec, resolved: true });
      status = "Comment resolved";
    }
  }

  async function reopenComment(id: string) {
    if (!session) return;
    const rec = commentsMap(session.doc).get(id);
    if (!rec) return;
    const prev = { ...rec };
    if (isTauri && doc) {
      try {
        const op = await reopenAnnotation(doc.meta.path, id, rec.revision ?? 1);
        await applyOpResult(op);
        status = "Thread reopened";
      } catch (e) {
        upsertComment(commentsMap(session.doc), prev);
        if (isAnnotationConflictError(e)) {
          await refreshAnnotationsFromDisk();
          status = "Annotation conflict: refreshed from disk.";
        } else {
          status = `error: could not reopen (${e})`;
        }
      }
    } else {
      upsertComment(commentsMap(session.doc), { ...rec, resolved: false });
      status = "Thread reopened";
    }
  }

  async function acceptSuggestion(id: string) {
    if (!session || !doc) return;
    const rec = commentsMap(session.doc).get(id);
    if (!rec || rec.kind !== "suggestion") return;
    if (rec.disposition === "accepting") {
      status =
        "This suggestion has an incomplete acceptance. Use Cancel acceptance or complete recovery after Save.";
      return;
    }
    if (!isTauri) {
      const ok = editorRef?.acceptSuggestion?.(id, rec.body, rec.quote) ?? false;
      if (!ok) {
        status = "Accept failed: quoted text not found in document";
        return;
      }
      upsertComment(commentsMap(session.doc), {
        ...rec,
        resolved: true,
        disposition: "accepted",
      });
      dirty = true;
      status = "Suggestion accepted (browser session only)";
      return;
    }

    // Phase A: reserve before mutating Markdown
    const expectedHash = baseContentHash;
    if (!expectedHash) {
      status = "Save the document first so acceptance can bind to a content revision.";
      return;
    }
    let begin;
    try {
      begin = await beginAcceptSuggestion(
        doc.meta.path,
        id,
        rec.revision ?? 1,
        expectedHash,
      );
      await applyOpResult(begin);
    } catch (e) {
      if (isAnnotationConflictError(e)) {
        await refreshAnnotationsFromDisk();
        status = "Could not begin accept (conflict or content hash mismatch). Refreshed from disk.";
      } else {
        status = `Could not begin accept (${e})`;
      }
      return;
    }

    // Phase B: apply edit + save
    const ok = editorRef?.acceptSuggestion?.(id, rec.body, rec.quote) ?? false;
    if (!ok) {
      try {
        const cancelled = await cancelAcceptSuggestion(
          doc.meta.path,
          id,
          begin.annotation.revision ?? 2,
          begin.acceptanceOpId,
        );
        await applyOpResult(cancelled);
      } catch {
        /* best effort */
      }
      orphanedMarkIds = [...new Set([...orphanedMarkIds, id])];
      status = "Accept cancelled: quoted text not found. Reservation released.";
      return;
    }

    dirty = true;
    try {
      await handleSave(false);
    } catch {
      /* handleSave sets status */
    }
    if (dirty || externalConflict) {
      try {
        const cancelled = await cancelAcceptSuggestion(
          doc.meta.path,
          id,
          begin.annotation.revision ?? 2,
          begin.acceptanceOpId,
        );
        await applyOpResult(cancelled);
      } catch {
        status =
          "Save failed after reservation. Suggestion remains incomplete (accepting). Cancel or retry.";
        return;
      }
      status = "Save failed; acceptance cancelled. Markdown not finalized as accepted.";
      return;
    }

    // Phase C: finalize against saved hash
    const savedHash = baseContentHash ?? runReview?.contentHash;
    if (!savedHash) {
      status = "Missing saved content hash; suggestion left incomplete. Cancel or complete after Save.";
      return;
    }
    try {
      // revision after begin is begin.annotation.revision
      const cur = commentsMap(session.doc).get(id);
      const expectRev = cur?.revision ?? begin.annotation.revision ?? 1;
      const op = await completeAcceptSuggestion(
        doc.meta.path,
        id,
        expectRev,
        begin.acceptanceOpId,
        savedHash,
      );
      await applyOpResult(op);
      orphanedMarkIds = orphanedMarkIds.filter((x) => x !== id);
      const left = countPending(listComments(commentsMap(session.doc), true)).suggestions;
      status =
        left > 0
          ? `Suggestion accepted; ${left} still pending`
          : "Suggestion accepted";
    } catch (e) {
      await refreshAnnotationsFromDisk();
      status = `Incomplete acceptance: finalize failed (${e}). Cancel or retry after checking the document.`;
    }
  }

  async function cancelIncompleteAcceptance(id: string) {
    if (!session || !doc || !isTauri) return;
    const rec = commentsMap(session.doc).get(id);
    if (!rec?.acceptanceOpId) {
      status = "No active acceptance to cancel.";
      return;
    }
    recoveryBusy = true;
    try {
      // Prefer backend recovery check (disk hash), not editor buffer.
      const st = await getAcceptanceRecoveryStatus(doc.meta.path, id);
      if (!st.cancelSafe) {
        await refreshAnnotationsFromDisk();
        await refreshRunReview(doc.meta.path);
        status =
          "Cannot cancel: Markdown changed after acceptance began. Finalize against the saved document, or restore the original revision first.";
        return;
      }
      const op = await cancelAcceptSuggestion(
        doc.meta.path,
        id,
        rec.revision ?? st.revision,
        rec.acceptanceOpId,
      );
      await applyOpResult(op);
      status = "Acceptance cancelled; suggestion is pending again.";
    } catch (e) {
      await refreshAnnotationsFromDisk();
      await refreshRunReview(doc.meta.path);
      status = `Could not cancel acceptance (${e})`;
    } finally {
      recoveryBusy = false;
    }
  }

  async function finalizeIncompleteAcceptance(id: string) {
    if (!session || !doc || !isTauri) return;
    recoveryBusy = true;
    try {
      const st = await getAcceptanceRecoveryStatus(doc.meta.path, id);
      if (!st.acceptanceOpId) {
        status = "No acceptance operation to finalize.";
        return;
      }
      if (st.cancelSafe) {
        status =
          "Document still matches the base revision. Cancel acceptance if the suggestion was not applied, or apply and Save before finalizing.";
        return;
      }
      const op = await completeAcceptSuggestion(
        doc.meta.path,
        id,
        st.revision,
        st.acceptanceOpId,
        st.currentContentHash,
      );
      await applyOpResult(op);
      await refreshRunReview(doc.meta.path);
      status = "Acceptance finalized for the current saved document revision.";
    } catch (e) {
      await refreshAnnotationsFromDisk();
      await refreshRunReview(doc.meta.path);
      status = `Could not finalize acceptance (${e})`;
    } finally {
      recoveryBusy = false;
    }
  }

  async function refreshAcceptanceRecovery(id: string) {
    if (!doc || !isTauri) return;
    recoveryBusy = true;
    try {
      await refreshAnnotationsFromDisk();
      await refreshRunReview(doc.meta.path);
      const st = await getAcceptanceRecoveryStatus(doc.meta.path, id);
      status = st.cancelSafe
        ? "Recovery status: Markdown still at base hash (Cancel is safe)."
        : "Recovery status: Markdown differs from base (Finalize or restore original).";
    } catch (e) {
      status = `Could not refresh recovery status (${e})`;
    } finally {
      recoveryBusy = false;
    }
  }

  async function rejectSuggestion(id: string) {
    if (!session) return;
    const rec = commentsMap(session.doc).get(id);
    if (!rec || rec.kind !== "suggestion") return;
    const prev = { ...rec };
    editorRef?.clearCommentMark?.(id);
    if (isTauri && doc) {
      try {
        const op = await rejectSuggestionApi(doc.meta.path, id, rec.revision ?? 1);
        await applyOpResult(op);
      } catch (e) {
        upsertComment(commentsMap(session.doc), prev);
        if (isAnnotationConflictError(e)) {
          await refreshAnnotationsFromDisk();
          status = "Annotation conflict: refreshed from disk.";
        } else {
          status = `error: could not reject (${e})`;
        }
        return;
      }
    } else {
      upsertComment(commentsMap(session.doc), { ...rec, resolved: true });
    }
    orphanedMarkIds = orphanedMarkIds.filter((x) => x !== id);
    const left = countPending(listComments(commentsMap(session.doc), true)).suggestions;
    status =
      left > 0
        ? `Suggestion rejected; ${left} still pending`
        : "Suggestion rejected";
  }

  function focusComment(id: string) {
    editorRef?.focusComment?.(id);
  }

  async function refreshHistory() {
    if (!doc) return;
    historyLoading = true;
    try {
      historyEntries = await historyList(doc.meta.path);
    } catch (e) {
      status = `History failed: ${e}`;
      historyEntries = [];
    } finally {
      historyLoading = false;
    }
  }

  async function toggleHistory() {
    historyOpen = !historyOpen;
    if (historyOpen) {
      commentsOpen = false;
      await refreshHistory();
    }
  }

  function toggleComments() {
    commentsOpen = !commentsOpen;
    if (commentsOpen) historyOpen = false;
  }

  async function restoreEntry(id: string) {
    if (!doc) return;
    try {
      const content = await historyRestoreContent(doc.meta.path, id);
      applyDocument(
        {
          ...doc,
          content,
          meta: { ...doc.meta, dirty: true, byteLen: content.length },
        },
        true,
      );
      status = "Restored snapshot (unsaved)";
    } catch (e) {
      status = `Restore failed: ${e}`;
    }
  }

  function countWords(text: string): number {
    const t = text.trim();
    if (!t) return 0;
    return t.split(/\s+/).length;
  }
</script>

<div class="flex h-screen flex-col">
  <Toolbar
    {title}
    {path}
    {dirty}
    {saving}
    {viewMode}
    {historyOpen}
    commentsOpen={commentsOpen}
    remotePeers={peerCount}
    {isTauri}
    onOpen={handleOpen}
    onSave={() => handleSave(false)}
    onComment={() => addAnnotation("comment")}
    onSuggest={() => addAnnotation("suggestion")}
    onToggleComments={toggleComments}
    onToggleHistory={toggleHistory}
    onViewMode={(m) => (viewMode = m)}
  />

  {#if isTauri}
    <RunReviewPanel
      review={runReview}
      busy={reviewBusy}
      dirty={dirty}
      externalConflict={externalConflict}
      saving={saving}
      onDecide={onRunDecide}
      onReload={reloadFromDiskKeepingLocalCopy}
    />
  {/if}

  <div class="flex min-h-0 flex-1">
    <main class="flex min-w-0 flex-1">
      {#if viewMode === "edit" || viewMode === "split"}
        <div
          class={`min-w-0 ${viewMode === "edit" ? "w-full" : "w-1/2"} ${viewMode === "split" ? "border-r" : ""}`}
          style="border-color: var(--border);"
        >
          {#if session}
            <Editor
              bind:this={editorRef}
              {session}
              initialMarkdown={markdown}
              onUpdate={onEditorUpdate}
              onReady={onEditorReady}
            />
          {:else}
            <div class="p-6 text-sm" style="color: var(--muted);">Loading editor…</div>
          {/if}
        </div>
      {/if}

      {#if viewMode === "preview" || viewMode === "split"}
        <div class={`min-w-0 ${viewMode === "preview" ? "w-full" : "w-1/2"}`}>
          <Preview content={markdown} />
        </div>
      {/if}
    </main>

    {#if commentsOpen}
      <CommentsPanel
        comments={commentList}
        orphanedIds={orphanedMarkIds}
        showResolved={showResolvedComments}
        currentDiskHash={baseContentHash}
        recoveryBusy={recoveryBusy}
        onToggleShowResolved={() => (showResolvedComments = !showResolvedComments)}
        onResolve={resolveComment}
        onReopen={reopenComment}
        onAccept={acceptSuggestion}
        onReject={rejectSuggestion}
        onCancelAccept={cancelIncompleteAcceptance}
        onFinalizeAccept={finalizeIncompleteAcceptance}
        onRefreshRecovery={refreshAcceptanceRecovery}
        onFocus={focusComment}
        onClose={() => (commentsOpen = false)}
      />
    {/if}

    {#if historyOpen}
      <HistoryPanel
        entries={historyEntries}
        loading={historyLoading}
        onRestore={restoreEntry}
        onRefresh={refreshHistory}
        onClose={() => (historyOpen = false)}
      />
    {/if}
  </div>

  <StatusBar
    {wordCount}
    {charCount}
    collabPeers={peerCount}
    peerNames={peerLabel}
    roomId={session?.roomId ?? null}
    autosavePaused={hasRemotePeers}
    pendingComments={pending.comments}
    pendingSuggestions={pending.suggestions}
    orphanedMarks={orphanedMarkIds.length}
    message={status}
  />
</div>
