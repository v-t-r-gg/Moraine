<script lang="ts">
  interface Props {
    wordCount: number;
    charCount: number;
    collabPeers: number;
    peerNames: string;
    roomId: string | null;
    autosavePaused: boolean;
    pendingComments: number;
    pendingSuggestions: number;
    orphanedMarks: number;
    message: string | null;
  }

  let {
    wordCount,
    charCount,
    collabPeers,
    peerNames,
    roomId,
    autosavePaused,
    pendingComments,
    pendingSuggestions,
    orphanedMarks,
    message,
  }: Props = $props();
</script>

<footer
  class="flex h-7 shrink-0 items-center gap-3 border-t px-3 text-[11px]"
  style="background: var(--panel); border-color: var(--border); color: var(--muted);"
>
  <span>{wordCount} words</span>
  <span>{charCount} chars</span>
  {#if roomId}
    <span title="Yjs room">room:{roomId}</span>
  {/if}
  {#if collabPeers > 0}
    <span class="text-ice-500" title={peerNames || undefined}>
      You + {collabPeers} other{collabPeers === 1 ? "" : "s"}
      {#if peerNames}
        ({peerNames})
      {/if}
    </span>
  {/if}
  {#if pendingSuggestions > 0}
    <span style="color: #16a34a; font-weight: 600;">
      {pendingSuggestions} suggestion{pendingSuggestions === 1 ? "" : "s"} pending
    </span>
  {/if}
  {#if pendingComments > 0}
    <span style="color: #d97706;">
      {pendingComments} comment{pendingComments === 1 ? "" : "s"}
    </span>
  {/if}
  {#if orphanedMarks > 0}
    <span title="Quoted text no longer found in the document" style="color: #dc2626;">
      {orphanedMarks} mark{orphanedMarks === 1 ? "" : "s"} missing
    </span>
  {/if}
  {#if autosavePaused}
    <span style="color: #f59e0b; font-weight: 600;">
      Live collab: autosave paused
    </span>
  {/if}
  <span class="ml-auto truncate">{message ?? "Ready"}</span>
</footer>
