<script lang="ts">
  import {
    changeFindingState,
    createFinding,
    getFinding,
    getRunCheckpoints,
    type FindingDetailDto,
    type FindingKind,
    type FindingListItemDto,
    type RunCheckpointsDetailDto,
  } from "$lib/api";

  interface Props {
    path: string | null;
    /** Bump to reload checkpoints/findings after external changes. */
    refreshToken?: number;
  }

  let { path, refreshToken = 0 }: Props = $props();

  const KINDS: { value: FindingKind; label: string }[] = [
    { value: "clarification", label: "Clarification" },
    { value: "inconsistency", label: "Inconsistency" },
    { value: "missing_evidence", label: "Missing evidence" },
    { value: "risk_concern", label: "Risk concern" },
    { value: "factual_correction", label: "Factual correction" },
    { value: "other", label: "Other" },
  ];

  let detail = $state<RunCheckpointsDetailDto | null>(null);
  let selectedOpId = $state<string | null>(null);
  let selectedFindingId = $state<string | null>(null);
  let thread = $state<FindingDetailDto | null>(null);
  let formKind = $state<FindingKind>("clarification");
  let formBody = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);
  let status = $state<string | null>(null);

  async function load() {
    if (!path) {
      detail = null;
      selectedOpId = null;
      selectedFindingId = null;
      thread = null;
      return;
    }
    error = null;
    try {
      detail = await getRunCheckpoints(path);
      if (selectedOpId && !detail.checkpoints.some((c) => c.opId === selectedOpId)) {
        selectedOpId = null;
      }
      if (!selectedOpId && detail.checkpoints.length > 0) {
        selectedOpId = detail.checkpoints[0].opId;
      }
      if (selectedFindingId) {
        await loadThread(selectedFindingId);
      }
    } catch (e) {
      detail = null;
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function loadThread(findingId: string) {
    if (!path) return;
    try {
      thread = await getFinding(path, findingId);
      selectedFindingId = findingId;
    } catch (e) {
      thread = null;
      error = e instanceof Error ? e.message : String(e);
    }
  }

  $effect(() => {
    // Track path + refreshToken so reloads re-fetch.
    const _p = path;
    const _r = refreshToken;
    void _p;
    void _r;
    void load();
  });

  function findingsForCheckpoint(opId: string): FindingListItemDto[] {
    if (!detail) return [];
    return detail.findings.filter((f) => f.target.checkpointOpId === opId);
  }

  function when(iso: string): string {
    try {
      return new Date(iso).toLocaleString();
    } catch {
      return iso;
    }
  }

  async function submitFinding() {
    if (!path || !selectedOpId) return;
    const body = formBody.trim();
    if (!body) {
      error = "Finding body is required.";
      return;
    }
    busy = true;
    error = null;
    status = null;
    try {
      const result = await createFinding(path, selectedOpId, formKind, body);
      formBody = "";
      status = "Finding created.";
      await load();
      await loadThread(result.findingId);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function markState(state: "open" | "addressed" | "archived") {
    if (!path || !selectedFindingId) return;
    busy = true;
    error = null;
    try {
      await changeFindingState(path, selectedFindingId, state);
      status = `Marked ${state}.`;
      await load();
      await loadThread(selectedFindingId);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section
  class="border-b px-3 py-2 text-xs"
  style="background: var(--panel); border-color: var(--border);"
>
  <div class="mb-1 flex items-center justify-between gap-2">
    <span class="font-semibold">Checkpoint findings</span>
    <span class="rounded px-1.5 py-0.5 font-medium" style="background: var(--accent-soft); color: var(--muted);">
      review context · no verdict
    </span>
  </div>

  {#if !path}
    <p style="color: var(--muted);">Open a run record to attach findings to checkpoints.</p>
  {:else if error && !detail}
    <p style="color: #b45309;">{error}</p>
  {:else if !detail || detail.checkpoints.length === 0}
    <p style="color: var(--muted);">No structured checkpoints on this run yet.</p>
  {:else}
    <div class="grid gap-2">
      <div>
        <label class="mb-0.5 block font-medium" style="color: var(--muted);" for="cp-select">
          Checkpoint
        </label>
        <select
          id="cp-select"
          class="w-full rounded border px-2 py-1"
          style="background: var(--bg); border-color: var(--border); color: var(--text);"
          bind:value={selectedOpId}
          onchange={() => {
            selectedFindingId = null;
            thread = null;
          }}
        >
          {#each detail.checkpoints as cp (cp.opId)}
            <option value={cp.opId}>
              {cp.summary.slice(0, 80)}{cp.summary.length > 80 ? "…" : ""}
              ({cp.openFindingCount} open / {cp.findingCount})
            </option>
          {/each}
        </select>
      </div>

      {#if selectedOpId}
        {@const related = findingsForCheckpoint(selectedOpId)}
        <div>
          <div class="mb-0.5 font-medium" style="color: var(--muted);">Findings on checkpoint</div>
          {#if related.length === 0}
            <p style="color: var(--muted);">None yet.</p>
          {:else}
            <ul class="grid gap-1">
              {#each related as f (f.findingId)}
                <li>
                  <button
                    type="button"
                    class="w-full rounded border px-2 py-1 text-left"
                    style="background: {selectedFindingId === f.findingId
                      ? 'var(--accent-soft)'
                      : 'var(--bg)'}; border-color: var(--border); color: var(--text);"
                    onclick={() => loadThread(f.findingId)}
                  >
                    <span class="font-medium">{f.kind}</span>
                    · <span style="color: var(--muted);">{f.state}</span>
                    · {f.responseCount} response{f.responseCount === 1 ? "" : "s"}
                    <div class="truncate" style="color: var(--muted);">{f.body}</div>
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        </div>

        <form
          class="grid gap-1 rounded border p-2"
          style="border-color: var(--border);"
          onsubmit={(e) => {
            e.preventDefault();
            void submitFinding();
          }}
        >
          <div class="font-medium">Add finding</div>
          <label class="grid gap-0.5">
            <span style="color: var(--muted);">Kind</span>
            <select
              class="rounded border px-2 py-1"
              style="background: var(--bg); border-color: var(--border); color: var(--text);"
              bind:value={formKind}
            >
              {#each KINDS as k (k.value)}
                <option value={k.value}>{k.label}</option>
              {/each}
            </select>
          </label>
          <label class="grid gap-0.5">
            <span style="color: var(--muted);">Body</span>
            <textarea
              class="min-h-[3.5rem] w-full rounded border px-2 py-1"
              style="background: var(--bg); border-color: var(--border); color: var(--text);"
              bind:value={formBody}
              placeholder="Descriptive review context only"
              maxlength={4000}
            ></textarea>
          </label>
          <button type="submit" class="btn self-start" disabled={busy}>
            {busy ? "Saving…" : "Create finding"}
          </button>
        </form>

        {#if thread}
          <div class="rounded border p-2" style="border-color: var(--border);">
            <div class="mb-1 flex flex-wrap items-center justify-between gap-1">
              <span class="font-medium">Finding thread</span>
              <span style="color: var(--muted);">{thread.kind} · {thread.state}</span>
            </div>
            <div class="mb-1 grid gap-1">
              {#each thread.thread as item (item.id)}
                <div
                  class="rounded px-2 py-1"
                  style="background: var(--bg); border: 1px solid var(--border);"
                >
                  <div class="mb-0.5 flex justify-between gap-2" style="color: var(--muted);">
                    <span>
                      {item.itemKind === "finding" ? "Human finding" : "Agent response"}
                      {#if item.findingKind}
                        · {item.findingKind}
                      {/if}
                    </span>
                    <span>{when(item.createdAt)}</span>
                  </div>
                  <div style="color: var(--text); white-space: pre-wrap;">{item.body}</div>
                </div>
              {/each}
            </div>
            <div class="flex flex-wrap gap-1">
              {#if thread.state !== "addressed"}
                <button type="button" class="btn" disabled={busy} onclick={() => markState("addressed")}>
                  Mark addressed
                </button>
              {/if}
              {#if thread.state !== "archived"}
                <button type="button" class="btn" disabled={busy} onclick={() => markState("archived")}>
                  Archive
                </button>
              {/if}
              {#if thread.state !== "open"}
                <button type="button" class="btn" disabled={busy} onclick={() => markState("open")}>
                  Reopen
                </button>
              {/if}
            </div>
          </div>
        {/if}
      {/if}

      {#if status}
        <p style="color: var(--muted);">{status}</p>
      {/if}
      {#if error}
        <p style="color: #b45309;">{error}</p>
      {/if}
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
    cursor: pointer;
  }
  .btn:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }
</style>
