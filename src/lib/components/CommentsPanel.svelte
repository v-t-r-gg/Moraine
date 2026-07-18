<script lang="ts">
  import type { CommentRecord } from "$lib/editor/comments";

  interface Props {
    comments: CommentRecord[];
    showResolved: boolean;
    onToggleShowResolved: () => void;
    onResolve: (id: string) => void;
    onReopen: (id: string) => void;
    onFocus: (id: string) => void;
    onClose: () => void;
  }

  let {
    comments,
    showResolved,
    onToggleShowResolved,
    onResolve,
    onReopen,
    onFocus,
    onClose,
  }: Props = $props();

  const visible = $derived(
    showResolved ? comments : comments.filter((c) => !c.resolved),
  );

  function when(iso: string): string {
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
    <h2 class="text-sm font-semibold">Comments</h2>
    <button type="button" class="icon-btn" onclick={onClose} title="Close">✕</button>
  </div>

  <label class="flex items-center gap-2 px-3 py-2 text-[11px]" style="color: var(--muted);">
    <input type="checkbox" checked={showResolved} onchange={onToggleShowResolved} />
    Show resolved
  </label>

  <div class="moraine-scroll flex-1 overflow-auto p-2">
    {#if visible.length === 0}
      <p class="px-2 text-xs" style="color: var(--muted);">
        Select text and click Comment to start a thread.
      </p>
    {:else}
      <ul class="space-y-2">
        {#each visible as c (c.id)}
          <li
            class="rounded-lg border p-2 text-xs"
            style="border-color: var(--border); opacity: {c.resolved ? 0.65 : 1};"
          >
            <button
              type="button"
              class="w-full text-left"
              onclick={() => onFocus(c.id)}
            >
              <div class="font-medium" style="color: var(--accent);">
                “{c.quote.slice(0, 80)}{c.quote.length > 80 ? "…" : ""}”
              </div>
              <div class="mt-1 whitespace-pre-wrap">{c.body}</div>
              <div class="mt-1" style="color: var(--muted);">
                {c.author} · {when(c.createdAt)}
                {#if c.resolved}
                  · resolved
                {/if}
              </div>
            </button>
            <div class="mt-1.5 flex gap-2">
              {#if c.resolved}
                <button
                  type="button"
                  class="link"
                  onclick={() => onReopen(c.id)}
                >
                  Reopen
                </button>
              {:else}
                <button
                  type="button"
                  class="link"
                  onclick={() => onResolve(c.id)}
                >
                  Resolve
                </button>
              {/if}
            </div>
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
  }
  .icon-btn:hover {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .link {
    border: none;
    background: none;
    padding: 0;
    color: var(--accent);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
  }
</style>
