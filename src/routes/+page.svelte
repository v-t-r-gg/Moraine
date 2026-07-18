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
    onFileChanged,
    openDocument,
    pickMarkdownFile,
    reloadDocument,
    saveDocument,
    takeStartupPath,
    writeFile,
  } from "$lib/api";
  import {
    collabFromLocation,
    createYjsSession,
    roomIdForPath,
    type YjsSession,
  } from "$lib/editor/yjsSession";
  import {
    commentsMap,
    listComments,
    newCommentId,
    setResolved,
    upsertComment,
    type CommentRecord,
  } from "$lib/editor/comments";
  import {
    AUTOSAVE_MS,
    remotePeerCount,
    shouldScheduleAutosave,
    statusForPeerTransition,
  } from "$lib/editor/hostSave";
  import type { DocumentSnapshot, HistoryEntryMeta, ViewMode } from "$lib/types";

  const WELCOME_MD = `# Welcome to Moraine

Local-first Markdown with collab, host-save policy, and comments.

## Try

- \`moraine share this-file.md\` then open the join URL
- Select text then Comment
- With peers: autosave pauses; Save still writes the host file
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
  let status = $state<string | null>(null);
  let session = $state<YjsSession | null>(null);
  let peerCount = $state(0);
  let editorRef = $state<Editor | undefined>(undefined);
  let syncUrl = $state<string | null>(null);
  let forcedRoomId = $state<string | null>(null);
  let localAuthor = $state("You");

  let autosaveTimer: ReturnType<typeof setTimeout> | null = null;
  let unlistenFile: (() => void) | null = null;
  let unsubComments: (() => void) | null = null;
  let prevPeerCount = 0;
  let suppressEditorDirty = false;
  let saveToken = 0;

  const title = $derived(doc?.meta.title ?? "Moraine");
  const path = $derived(doc?.meta.path ?? null);
  const wordCount = $derived(countWords(markdown));
  const charCount = $derived(markdown.length);
  const autosavePaused = $derived(peerCount > 0);

  onMount(async () => {
    const collab = collabFromLocation();
    syncUrl = collab.syncUrl;
    forcedRoomId = collab.roomId;
    try {
      const info = await appInfo();
      const bits = [
        info.name,
        info.version,
        !isTauri ? "browser" : null,
        forcedRoomId ? `room ${forcedRoomId}` : null,
        syncUrl ? `sync ${syncUrl}` : null,
      ].filter(Boolean);
      status = bits.join(" · ");
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
        const reloaded = await reloadDocument(doc.meta.id);
        applyDocument(reloaded, true);
        status = "Reloaded from disk";
      } catch (e) {
        status = `Reload failed: ${e}`;
      }
    });
  });

  onDestroy(() => {
    if (autosaveTimer) clearTimeout(autosaveTimer);
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
        /* create via open */
      }
      await loadPath(demo);
    } else {
      await loadPath("welcome.md");
    }
  }

  async function loadPath(filePath: string) {
    try {
      const snap = await openDocument(filePath);
      applyDocument(snap, true);
      status = `Opened ${snap.meta.title}`;
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
    suppressEditorDirty = true;
    doc = snap;
    markdown = snap.content;
    dirty = snap.meta.dirty;

    if (resetSession) {
      session?.destroy();
      unsubComments?.();
      unsubComments = null;
      clearAutosaveTimer();

      const room = forcedRoomId ?? roomIdForPath(snap.meta.path);
      const s = createYjsSession(room, { syncUrl });
      session = s;
      peerCount = 0;
      prevPeerCount = 0;

      const user = s.awareness.getLocalState()?.user as { name?: string } | undefined;
      localAuthor = user?.name ?? "You";

      const cmap = commentsMap(s.doc);
      const refresh = () => {
        commentList = listComments(cmap, true);
      };
      refresh();
      cmap.observe(refresh);
      unsubComments = () => cmap.unobserve(refresh);

      s.awareness.on("change", () => {
        onPeerCountChange(remotePeerCount(s.awareness.getStates().size));
      });
    }

    queueMicrotask(() => {
      suppressEditorDirty = false;
    });
  }

  function onPeerCountChange(next: number) {
    const was = prevPeerCount;
    peerCount = next;
    prevPeerCount = next;

    const msg = statusForPeerTransition(was, next, dirty);
    if (msg) status = msg;

    if (next > 0) {
      clearAutosaveTimer();
    } else if (was > 0 && dirty) {
      scheduleAutosave();
    }
  }

  function onEditorUpdate(md: string) {
    if (suppressEditorDirty) return;
    if (md === markdown) return;
    markdown = md;
    dirty = true;
    scheduleAutosave();
  }

  function clearAutosaveTimer() {
    if (autosaveTimer) {
      clearTimeout(autosaveTimer);
      autosaveTimer = null;
    }
  }

  function scheduleAutosave() {
    clearAutosaveTimer();
    if (
      !shouldScheduleAutosave({
        isHost: isTauri,
        peerCount,
        dirty: true,
        saving,
      })
    ) {
      return;
    }
    autosaveTimer = setTimeout(() => {
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
    if (fromAutosave && peerCount > 0) return;
    if (fromAutosave && saving) return;

    if (!isTauri) {
      dirty = false;
      status = fromAutosave ? "Autosaved (browser)" : "Saved (browser)";
      return;
    }

    const token = ++saveToken;
    const md = editorRef?.getMarkdownContent?.() ?? markdown;
    saving = true;
    try {
      const snap = await saveDocument(doc.meta.id, md, true);
      if (token !== saveToken) return;

      doc = snap;
      // If the user typed during the await, keep dirty and current markdown.
      const now = editorRef?.getMarkdownContent?.() ?? markdown;
      if (now === md) {
        markdown = snap.content;
        dirty = false;
        status = fromAutosave
          ? "Autosaved"
          : peerCount > 0
            ? "Saved (host write while peers present)"
            : "Saved";
      } else {
        markdown = now;
        dirty = true;
        status = "Saved; newer edits still pending";
        scheduleAutosave();
      }
      if (historyOpen) await refreshHistory();
    } catch (e) {
      if (token === saveToken) status = `Save failed: ${e}`;
    } finally {
      if (token === saveToken) saving = false;
    }
  }

  function handleComment() {
    if (!session || !editorRef) return;
    const quote = editorRef.getSelectionQuote?.();
    if (!quote) {
      status = "Select text to comment";
      return;
    }
    const body = window.prompt("Comment", "");
    if (body == null) return;
    const text = body.trim();
    if (!text) return;

    const id = newCommentId();
    if (!editorRef.applyCommentMark?.(id)) {
      status = "Could not attach comment mark";
      return;
    }

    // Mark is in the collab doc; metadata is a separate Y.Map (both sync).
    // Do not mark the host file dirty solely for metadata (map is not in markdown).
    upsertComment(commentsMap(session.doc), {
      id,
      body: text,
      author: localAuthor,
      quote,
      createdAt: new Date().toISOString(),
      resolved: false,
    });
    commentsOpen = true;
    status = "Comment added";
  }

  function resolveComment(id: string) {
    if (!session) return;
    editorRef?.clearCommentMark?.(id);
    setResolved(commentsMap(session.doc), id, true);
    status = "Comment resolved";
  }

  function reopenComment(id: string) {
    if (!session) return;
    setResolved(commentsMap(session.doc), id, false);
    status = "Comment reopened (highlight cleared earlier)";
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
    onComment={handleComment}
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
        showResolved={showResolvedComments}
        onToggleShowResolved={() => (showResolvedComments = !showResolvedComments)}
        onResolve={resolveComment}
        onReopen={reopenComment}
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
    roomId={session?.roomId ?? null}
    {autosavePaused}
    message={status}
  />
</div>
