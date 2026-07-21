import { useMemo } from "react";
import { renderMarkdown } from "@/features/editor/markdown";

export function Preview({ content }: { content: string }) {
  const html = useMemo(() => renderMarkdown(content || ""), [content]);
  return (
    <div className="moraine-scroll h-full overflow-auto" style={{ background: "var(--bg)" }}>
      <article
        className="prose prose-slate dark:prose-invert mx-auto max-w-3xl px-5 py-6"
        dangerouslySetInnerHTML={{ __html: html }}
      />
    </div>
  );
}
