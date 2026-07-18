<script lang="ts">
  import type { ViewMode } from "$lib/types";

  interface Props {
    title: string;
    path: string | null;
    dirty: boolean;
    saving: boolean;
    viewMode: ViewMode;
    historyOpen: boolean;
    commentsOpen: boolean;
    remotePeers: number;
    isTauri: boolean;
    onOpen: () => void;
    onSave: () => void;
    onComment: () => void;
    onSuggest: () => void;
    onToggleComments: () => void;
    onToggleHistory: () => void;
    onViewMode: (mode: ViewMode) => void;
  }

  let {
    title,
    path,
    dirty,
    saving,
    viewMode,
    historyOpen,
    commentsOpen,
    remotePeers,
    isTauri,
    onOpen,
    onSave,
    onComment,
    onSuggest,
    onToggleComments,
    onToggleHistory,
    onViewMode,
  }: Props = $props();
</script>

<header
  class="flex h-12 shrink-0 items-center gap-2 border-b px-3"
  style="background: var(--panel); border-color: var(--border);"
>
  <div class="flex items-center gap-2 min-w-0 flex-1">
    <span
      class="flex h-7 w-7 items-center justify-center rounded-md text-sm font-bold text-white"
      style="background: linear-gradient(135deg, #0ea5e9, #6366f1);"
      title="Moraine"
    >
      M
    </span>
    <div class="min-w-0">
      <div class="flex items-center gap-1.5 truncate text-sm font-semibold">
        <span class="truncate">{title}</span>
        {#if dirty}
          <span class="text-ice-500" title="Unsaved changes">●</span>
        {/if}
        {#if remotePeers > 0}
          <span
            class="rounded px-1.5 py-0.5 text-[10px] font-medium"
            style="background: var(--accent-soft); color: var(--accent);"
            title="Remote collaborators present; autosave paused"
          >
            live
          </span>
        {/if}
      </div>
      {#if path}
        <div class="truncate text-[11px]" style="color: var(--muted);" title={path}>
          {path}
        </div>
      {/if}
    </div>
  </div>

  <div class="flex items-center gap-1">
    <div
      class="mr-1 flex rounded-lg border p-0.5 text-xs"
      style="border-color: var(--border);"
      role="group"
      aria-label="View mode"
    >
      {#each [["edit", "Edit"], ["split", "Split"], ["preview", "Preview"]] as [mode, label]}
        <button
          type="button"
          class="rounded-md px-2.5 py-1 transition"
          class:font-semibold={viewMode === mode}
          style={viewMode === mode
            ? "background: var(--accent-soft); color: var(--accent);"
            : "color: var(--muted);"}
          onclick={() => onViewMode(mode as ViewMode)}
        >
          {label}
        </button>
      {/each}
    </div>

    <button type="button" class="btn" onclick={onOpen} title={isTauri ? "Open Markdown file" : "Open (Tauri only)"}>
      Open
    </button>
    <button
      type="button"
      class="btn btn-primary"
      onclick={onSave}
      disabled={saving || (!dirty && isTauri)}
      title={remotePeers > 0 ? "Save to disk (autosave paused while peers present)" : "Save"}
    >
      {saving ? "Saving…" : "Save"}
    </button>
    <button type="button" class="btn" onclick={onComment} title="Comment on selection">
      Comment
    </button>
    <button type="button" class="btn" onclick={onSuggest} title="Suggest replacement for selection">
      Suggest
    </button>
    <button
      type="button"
      class="btn"
      class:ring-1={commentsOpen}
      onclick={onToggleComments}
      title="Comments and suggestions"
    >
      Review
    </button>
    <button
      type="button"
      class="btn"
      class:ring-1={historyOpen}
      onclick={onToggleHistory}
      title="Edit history"
    >
      History
    </button>
  </div>
</header>

<style>
  .btn {
    border-radius: 0.5rem;
    border: 1px solid var(--border);
    background: var(--panel);
    color: var(--text);
    padding: 0.35rem 0.75rem;
    font-size: 0.8rem;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s, border-color 0.15s;
  }
  .btn:hover:not(:disabled) {
    border-color: var(--accent);
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .btn-primary {
    background: var(--accent);
    border-color: transparent;
    color: #fff;
  }
  .btn-primary:hover:not(:disabled) {
    filter: brightness(1.05);
  }
</style>
