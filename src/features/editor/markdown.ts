import { marked } from "marked";

marked.setOptions({
  gfm: true,
  breaks: false,
});

export function renderMarkdown(source: string): string {
  return marked.parse(source, { async: false }) as string;
}

export function htmlToPlain(html: string): string {
  if (typeof document === "undefined") {
    return html.replace(/<[^>]+>/g, "");
  }
  const el = document.createElement("div");
  el.innerHTML = html;
  return el.textContent ?? "";
}
