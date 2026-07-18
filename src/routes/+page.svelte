<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import Toolbar from "$lib/components/Toolbar.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import Preview from "$lib/components/Preview.svelte";
  import HistoryPanel from "$lib/components/HistoryPanel.svelte";
  import CommentsPanel from "$lib/components/CommentsPanel.svelte";
  import Editor from "$lib/components/Editor.svelte";
  import {
    appInfo,
    historyList,
    historyRestoreContent,
    isTauri,
    loadComments,
    onFileChanged,
    openDocument,
    pickMarkdownFile,
    reloadDocument,
    saveComments,
    saveDocument,
    takeStartupPath,
    writeFile,
    type CommentDto,
  } from "$lib/api";
  import {
    createYjsSession,
    resolveSessionConfig,
    roomIdForPath,
    type SessionConfig,
    type YjsSession,
  } from "$lib/editor/yjsSession";
  import {
    commentsMap,
    countPending,
    listComments,
    mergeDiskIntoMap,
    newCommentId,
    setResolved,
    upsertComment,
    type CommentRecord,
  } from "$lib/editor/comments";
  import {
    AUTOSAVE_MS,
    canAutosave,
    peerNames,
    remotePeerCount,
  } from "$lib/editor/hostSave";
  import type { DocumentSnapshot, HistoryEntryMeta, ViewMode } from "$lib/types";

  const WELCOME_MD = `# Agent run record

This is a **run record**: a durable Markdown log of agent work for human review.

## How to use

1. Agents write or update \`.md\` files (CLI or any tool).
2. Optional live room: \`moraine share this-file.md\` (relay must be running).
3. Humans open the file or join URL, then **Comment** / **Suggest**, **Review**, **Save**.
4. Annotations persist in \`file.md.comments.json\` on host Save.

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

  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let unlistenFile: (() => void) | null = null;
  let unsubComments: (() => void) | null = null;
  let ignoreNextEditorSync = false;
  let prevPeers = 0;
  let pendingRehydrate = false;

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
    unlistenFile = onFileChanged(async (ev) => {
      if (!doc || !ev.documentId || ev.documentId !== doc.meta.id) return;
      if (dirty || saving) {
        status = "File changed on disk (keeping local edits)";
        return;
      }
      try {
        applyDocument(await reloadDocument(doc.meta.id), true);
        status = "Reloaded from disk";
      } catch (e) {
        status = `Reload failed: ${e}`;
      }
    });
  });

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

  function applyDocument(snap: DocumentSnapshot, resetSession: boolean) {
    ignoreNextEditorSync = true;
    doc = snap;
    markdown = snap.content;
    dirty = snap.meta.dirty;

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
    }

    queueMicrotask(() => {
      ignoreNextEditorSync = false;
    });
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
    };
  }

  async function persistComments() {
    if (!isTauri || !doc || !session) return;
    const list = listComments(commentsMap(session.doc), true).map(recordToDto);
    try {
      await saveComments(doc.meta.path, list);
    } catch (e) {
      status = `error: could not write review sidecar (${e})`;
    }
  }

  function onEditorUpdate(md: string) {
    if (ignoreNextEditorSync) return;
    if (md === markdown) return;
    markdown = md;
    dirty = true;
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

    const md = editorRef?.getMarkdownContent?.() ?? markdown;
    saving = true;
    try {
      const snap = await saveDocument(doc.meta.id, md, true);
      doc = snap;
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
      await persistComments();
      if (!fromAutosave && isTauri) {
        status = `${status}; comments: ${doc.meta.path}.comments.json`;
      }
      if (historyOpen) await refreshHistory();
    } catch (e) {
      status = `error: save failed (${e})`;
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
    upsertComment(commentsMap(session.doc), {
      id,
      body: body.trim(),
      author: localAuthor,
      quote,
      createdAt: new Date().toISOString(),
      resolved: false,
      kind,
    });
    commentsOpen = true;
    historyOpen = false;

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
    if (isTauri) await persistComments();
  }

  async function resolveComment(id: string) {
    if (!session) return;
    editorRef?.clearCommentMark?.(id);
    setResolved(commentsMap(session.doc), id, true);
    status = "Comment resolved";
    if (isTauri) await persistComments();
  }

  async function reopenComment(id: string) {
    if (!session) return;
    setResolved(commentsMap(session.doc), id, false);
    status = "Thread reopened";
    if (isTauri) await persistComments();
  }

  async function acceptSuggestion(id: string) {
    if (!session) return;
    const rec = commentsMap(session.doc).get(id);
    if (!rec || rec.kind !== "suggestion") return;
    const ok = editorRef?.acceptSuggestion?.(id, rec.body, rec.quote) ?? false;
    if (!ok) {
      status = "Accept failed: quoted text not found in document";
      orphanedMarkIds = [...new Set([...orphanedMarkIds, id])];
      return;
    }
    setResolved(commentsMap(session.doc), id, true);
    orphanedMarkIds = orphanedMarkIds.filter((x) => x !== id);
    dirty = true;
    scheduleSave();
    const left = countPending(listComments(commentsMap(session.doc), true)).suggestions;
    status =
      left > 0
        ? `Suggestion accepted; ${left} still pending`
        : "Suggestion accepted";
    if (isTauri) await persistComments();
  }

  async function rejectSuggestion(id: string) {
    if (!session) return;
    editorRef?.clearCommentMark?.(id);
    setResolved(commentsMap(session.doc), id, true);
    orphanedMarkIds = orphanedMarkIds.filter((x) => x !== id);
    const left = countPending(listComments(commentsMap(session.doc), true)).suggestions;
    status =
      left > 0
        ? `Suggestion rejected; ${left} still pending`
        : "Suggestion rejected";
    if (isTauri) await persistComments();
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
        onToggleShowResolved={() => (showResolvedComments = !showResolvedComments)}
        onResolve={resolveComment}
        onReopen={reopenComment}
        onAccept={acceptSuggestion}
        onReject={rejectSuggestion}
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
