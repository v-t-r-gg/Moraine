<script lang="ts">
  interface Props {
    wordCount: number;
    charCount: number;
    collabPeers: number;
    roomId: string | null;
    autosavePaused: boolean;
    message: string | null;
  }

  let {
    wordCount,
    charCount,
    collabPeers,
    roomId,
    autosavePaused,
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
    <span class="text-ice-500">{collabPeers} peer{collabPeers === 1 ? "" : "s"}</span>
  {/if}
  {#if autosavePaused}
    <span style="color: #f59e0b;" title="Disk autosave resumes when peers leave">
      autosave paused
    </span>
  {/if}
  <span class="ml-auto truncate">{message ?? "Ready"}</span>
</footer>
