<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import Toolbar from "$lib/components/Toolbar.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import Preview from "$lib/components/Preview.svelte";
  import HistoryPanel from "$lib/components/HistoryPanel.svelte";
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
  import type { DocumentSnapshot, HistoryEntryMeta, ViewMode } from "$lib/types";

  const WELCOME_MD = `# Welcome to Moraine

**Moraine** is a local-first, Git-native collaborative Markdown editor.

## Phase 0–1

- Rich Markdown editing (Tiptap / ProseMirror)
- Open / save local \`.md\` files with auto-save
- Filesystem watcher
- Yjs multi-tab collab simulation
- Local edit history
- CLI: \`moraine cat|edit|history|watch\`

Start typing, toggle **Preview**, or open a file from disk.
`;

  let doc = $state<DocumentSnapshot | null>(null);
  let markdown = $state("");
  let dirty = $state(false);
  let saving = $state(false);
  let viewMode = $state<ViewMode>("edit");
  let historyOpen = $state(false);
  let historyEntries = $state<HistoryEntryMeta[]>([]);
  let historyLoading = $state(false);
  let status = $state<string | null>(null);
  let session = $state<YjsSession | null>(null);
  let peerCount = $state(0);
  let editorRef = $state<Editor | undefined>(undefined);
  let syncUrl = $state<string | null>(null);
  let forcedRoomId = $state<string | null>(null);

  let autosaveTimer: ReturnType<typeof setTimeout> | null = null;
  let unlistenFile: (() => void) | null = null;

  const title = $derived(doc?.meta.title ?? "Moraine");
  const path = $derived(doc?.meta.path ?? null);
  const wordCount = $derived(countWords(markdown));
  const charCount = $derived(markdown.length);

  onMount(async () => {
    const collab = collabFromLocation();
    syncUrl = collab.syncUrl;
    forcedRoomId = collab.roomId;
    try {
      const info = await appInfo();
      const mode = isTauri ? "" : " · browser";
      const room = forcedRoomId ? ` · room ${forcedRoomId}` : "";
      const sync = syncUrl ? ` · sync ${syncUrl}` : "";
      status = `${info.name} ${info.version}${mode}${room}${sync}`;
    } catch {
      status = "Moraine";
    }

    await loadInitial();

    unlistenFile = onFileChanged(async (ev) => {
      if (!doc || !ev.documentId || ev.documentId !== doc.meta.id) return;
      if (dirty) {
        status = "File changed on disk (unsaved local edits kept)";
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
        /* open_document creates missing files */
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
    doc = snap;
    markdown = snap.content;
    dirty = snap.meta.dirty;

    if (resetSession) {
      session?.destroy();
      const room = forcedRoomId ?? roomIdForPath(snap.meta.path);
      const s = createYjsSession(room, { syncUrl });
      session = s;
      peerCount = 0;
      s.awareness.on("change", () => {
        peerCount = Math.max(0, s.awareness.getStates().size - 1);
      });
    }
  }

  function onEditorUpdate(md: string) {
    if (md === markdown) return;
    markdown = md;
    dirty = true;
    scheduleAutosave();
  }

  function scheduleAutosave() {
    if (!isTauri || !doc) return;
    if (autosaveTimer) clearTimeout(autosaveTimer);
    autosaveTimer = setTimeout(() => {
      void handleSave(true);
    }, 1200);
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
    if (!isTauri) {
      dirty = false;
      status = fromAutosave ? "Autosaved (browser)" : "Saved (browser)";
      return;
    }
    saving = true;
    try {
      const md = editorRef?.getMarkdownContent?.() ?? markdown;
      const snap = await saveDocument(doc.meta.id, md, true);
      doc = snap;
      markdown = snap.content;
      dirty = false;
      status = fromAutosave ? "Autosaved" : "Saved";
      if (historyOpen) await refreshHistory();
    } catch (e) {
      status = `Save failed: ${e}`;
    } finally {
      saving = false;
    }
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
    if (historyOpen) await refreshHistory();
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
    {isTauri}
    onOpen={handleOpen}
    onSave={() => handleSave(false)}
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
    message={status}
  />
</div>
