<script lang="ts">
  export type RunReview = {
    runId: string;
    contentHash: string;
    reviewState: string;
    decisionCurrent: boolean;
    decisionCount: number;
    latest: {
      id: string;
      decision: string;
      reviewerLabel: string;
      reason: string | null;
      createdAt: string;
      contentHash: string;
    } | null;
    sidecar: string;
    initialized: boolean;
  };

  interface Props {
    review: RunReview | null;
    externalConflict: boolean;
    onReload?: () => void;
  }

  let { review, externalConflict, onReload }: Props = $props();

  function shortHash(h: string): string {
    return h.length > 12 ? `${h.slice(0, 12)}…` : h;
  }

  function when(iso: string): string {
    try {
      return new Date(iso).toLocaleString();
    } catch {
      return iso;
    }
  }
</script>

<section
  class="border-b px-3 py-2 text-xs"
  style="background: var(--panel); border-color: var(--border);"
>
  <div class="mb-1 flex items-center justify-between gap-2">
    <span class="font-semibold">Run ledger</span>
    {#if review}
      <span class="rounded px-1.5 py-0.5 font-medium" style="background: var(--accent-soft); color: var(--muted);">
        inspect · no verdict
      </span>
    {/if}
  </div>

  {#if !review}
    <p style="color: var(--muted);">Open a file to load run identity.</p>
  {:else}
    <div class="grid gap-0.5" style="color: var(--muted);">
      <div title={review.runId}>
        run <span class="font-mono text-[11px]" style="color: var(--text);">{review.runId.slice(0, 8)}…</span>
      </div>
      <div title={review.contentHash}>
        rev <span class="font-mono text-[11px]" style="color: var(--text);">{shortHash(review.contentHash)}</span>
        <span style="color: var(--muted);"> (saved revision)</span>
      </div>
      {#if review.latest && review.decisionCount > 0}
        <div class="mt-1" style="color: var(--muted);">
          Legacy decision history (compatibility):
          <strong style="color: var(--text);">{review.latest.decision}</strong>
          by {review.latest.reviewerLabel}
          · {when(review.latest.createdAt)}
          · {review.decisionCount} recorded
          {#if !review.decisionCurrent}
            · <span style="color: #b45309; font-weight: 600;">stale vs current revision</span>
          {/if}
        </div>
        {#if review.latest.reason}
          <div style="color: var(--text);">reason: {review.latest.reason}</div>
        {/if}
      {/if}
    </div>

    {#if externalConflict}
      <div class="mt-1 font-medium" style="color: #b45309;">
        The file changed on disk.
        {#if onReload}
          <button type="button" class="btn ml-1" onclick={() => onReload()}>Reload from disk</button>
        {/if}
      </div>
    {/if}
  {/if}
</section>

<style>
  .btn {
    border-radius: 0.4rem;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
    padding: 0.3rem 0.6rem;
    font-size: 0.75rem;
    font-weight: 600;
    cursor: pointer;
  }
  .btn:hover {
    border-color: var(--accent);
  }
  .ml-1 {
    margin-left: 0.25rem;
  }
</style>
