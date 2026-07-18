<script lang="ts">
  import type { HistoryEntryMeta } from "$lib/types";

  interface Props {
    entries: HistoryEntryMeta[];
    loading: boolean;
    onRestore: (id: string) => void;
    onRefresh: () => void;
    onClose: () => void;
  }

  let { entries, loading, onRestore, onRefresh, onClose }: Props = $props();

  function formatWhen(iso: string): string {
    try {
      return new Date(iso).toLocaleString();
    } catch {
      return iso;
    }
  }
</script>

<aside
  class="flex h-full w-72 shrink-0 flex-col border-l"
  style="background: var(--panel); border-color: var(--border);"
>
  <div
    class="flex items-center justify-between border-b px-3 py-2"
    style="border-color: var(--border);"
  >
    <h2 class="text-sm font-semibold">History</h2>
    <div class="flex gap-1">
      <button type="button" class="icon-btn" onclick={onRefresh} title="Refresh">↻</button>
      <button type="button" class="icon-btn" onclick={onClose} title="Close">✕</button>
    </div>
  </div>

  <div class="moraine-scroll flex-1 overflow-auto p-2">
    {#if loading}
      <p class="px-2 text-xs" style="color: var(--muted);">Loading…</p>
    {:else if entries.length === 0}
      <p class="px-2 text-xs" style="color: var(--muted);">
        No snapshots yet. Saves create history entries automatically.
      </p>
    {:else}
      <ul class="space-y-1">
        {#each entries as entry (entry.id)}
          <li
            class="rounded-lg border p-2 text-xs"
            style="border-color: var(--border);"
          >
            <div class="font-medium">{formatWhen(entry.createdAt)}</div>
            <div style="color: var(--muted);">
              {entry.source}
              {#if entry.label}
                · {entry.label}
              {/if}
              · {entry.byteLen} B
            </div>
            <button
              type="button"
              class="mt-1.5 text-[11px] font-semibold"
              style="color: var(--accent);"
              onclick={() => onRestore(entry.id)}
            >
              Restore
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</aside>

<style>
  .icon-btn {
    border: none;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
    padding: 0.2rem 0.4rem;
    border-radius: 0.35rem;
    font-size: 0.85rem;
  }
  .icon-btn:hover {
    background: var(--accent-soft);
    color: var(--accent);
  }
</style>
