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
  };

  interface Props {
    review: RunReview | null;
    busy: boolean;
    onDecide: (decision: string, reviewer: string, reason: string) => void;
  }

  let { review, busy, onDecide }: Props = $props();

  let reviewer = $state("Reviewer");
  let reason = $state("");

  function shortHash(h: string): string {
    return h.length > 12 ? `${h.slice(0, 12)}…` : h;
  }

  function stateLabel(s: string, current: boolean): string {
    if (s === "unreviewed") return "Unreviewed";
    if (s === "stale" || !current) return "Stale (content changed)";
    if (s === "approved") return "Approved";
    if (s === "changes_requested") return "Changes requested";
    if (s === "rejected") return "Rejected";
    return s;
  }

  function when(iso: string): string {
    try {
      return new Date(iso).toLocaleString();
    } catch {
      return iso;
    }
  }

  function submit(decision: string) {
    onDecide(decision, reviewer.trim() || "Reviewer", reason.trim());
  }
</script>

<section
  class="border-b px-3 py-2 text-xs"
  style="background: var(--panel); border-color: var(--border);"
>
  <div class="mb-1 flex items-center justify-between gap-2">
    <span class="font-semibold">Run review</span>
    {#if review}
      <span
        class="rounded px-1.5 py-0.5 font-medium"
        style={review.decisionCurrent && review.reviewState !== "unreviewed"
          ? "background: color-mix(in srgb, #16a34a 20%, transparent); color: #16a34a;"
          : review.reviewState === "stale" || !review.decisionCurrent
            ? "background: color-mix(in srgb, #f59e0b 25%, transparent); color: #b45309;"
            : "background: var(--accent-soft); color: var(--muted);"}
      >
        {stateLabel(review.reviewState, review.decisionCurrent)}
      </span>
    {/if}
  </div>

  {#if !review}
    <p style="color: var(--muted);">Open a file to load run review state.</p>
  {:else}
    <div class="grid gap-0.5" style="color: var(--muted);">
      <div title={review.runId}>
        run <span class="font-mono text-[11px]" style="color: var(--text);">{review.runId.slice(0, 8)}…</span>
      </div>
      <div title={review.contentHash}>
        rev <span class="font-mono text-[11px]" style="color: var(--text);">{shortHash(review.contentHash)}</span>
      </div>
      {#if review.latest}
        <div>
          latest <strong style="color: var(--text);">{review.latest.decision}</strong>
          by {review.latest.reviewerLabel}
          · {when(review.latest.createdAt)}
          · {review.decisionCount} decision{review.decisionCount === 1 ? "" : "s"}
        </div>
        {#if review.latest.reason}
          <div style="color: var(--text);">reason: {review.latest.reason}</div>
        {/if}
        {#if !review.decisionCurrent}
          <div style="color: #b45309; font-weight: 600;">
            Prior decision applies to an older revision. Record a new decision for this content.
          </div>
        {/if}
      {:else}
        <div>No run-level decision yet (comment/suggestion Accept is separate).</div>
      {/if}
    </div>

    <div class="mt-2 flex flex-wrap items-end gap-2">
      <label class="flex flex-col gap-0.5">
        <span style="color: var(--muted);">Reviewer label</span>
        <input
          class="rounded border px-2 py-1"
          style="border-color: var(--border); background: var(--bg); color: var(--text);"
          bind:value={reviewer}
          disabled={busy}
        />
      </label>
      <label class="flex min-w-[12rem] flex-1 flex-col gap-0.5">
        <span style="color: var(--muted);">Reason (optional)</span>
        <input
          class="rounded border px-2 py-1"
          style="border-color: var(--border); background: var(--bg); color: var(--text);"
          bind:value={reason}
          disabled={busy}
        />
      </label>
      <button type="button" class="btn" disabled={busy} onclick={() => submit("approved")}>
        Approve
      </button>
      <button type="button" class="btn" disabled={busy} onclick={() => submit("changes_requested")}>
        Request changes
      </button>
      <button type="button" class="btn" disabled={busy} onclick={() => submit("rejected")}>
        Reject
      </button>
    </div>
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
  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .btn:hover:not(:disabled) {
    border-color: var(--accent);
  }
</style>
