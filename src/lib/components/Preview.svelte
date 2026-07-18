<script lang="ts">
  import { renderMarkdown } from "$lib/editor/markdown";

  interface Props {
    content: string;
  }

  let { content }: Props = $props();

  let html = $derived(renderMarkdown(content || ""));
</script>

<div
  class="moraine-scroll h-full overflow-auto"
  style="background: var(--bg);"
>
  <article
    class="prose prose-slate dark:prose-invert mx-auto max-w-3xl px-5 py-6"
  >
    {@html html}
  </article>
</div>
