<script lang="ts">
  import { onDestroy } from "svelte";
  import { Editor } from "@tiptap/core";
  import StarterKit from "@tiptap/starter-kit";
  import Placeholder from "@tiptap/extension-placeholder";
  import Collaboration from "@tiptap/extension-collaboration";
  import CollaborationCursor from "@tiptap/extension-collaboration-cursor";
  import { Markdown } from "tiptap-markdown";
  import { CommentMark, findMarkRange, type MarkKind } from "$lib/editor/commentMark";
  import { findQuoteRangeInDoc, type CommentRecord } from "$lib/editor/comments";
  import type { YjsSession } from "$lib/editor/yjsSession";

  interface Props {
    session: YjsSession | null;
    initialMarkdown: string;
    editable?: boolean;
    onUpdate: (markdown: string) => void;
    onReady?: (editor: Editor) => void;
  }

  let {
    session,
    initialMarkdown,
    editable = true,
    onUpdate,
    onReady,
  }: Props = $props();

  let element: HTMLDivElement | undefined = $state();
  let editor: Editor | null = null;
  let lastRoom: string | null = null;

  $effect(() => {
    const s = session;
    const el = element;
    if (!el || !s) return;

    if (editor && lastRoom === s.roomId) return;

    editor?.destroy();
    editor = null;
    lastRoom = s.roomId;

    const user = s.awareness.getLocalState()?.user as
      | { name: string; color: string }
      | undefined;

    let seeded = false;

    editor = new Editor({
      element: el,
      editable,
      extensions: [
        StarterKit.configure({
          // Yjs owns undo/redo via the collaboration extension.
          history: false,
        }),
        Placeholder.configure({
          placeholder: "Write or review the agent run record…",
        }),
        Markdown.configure({
          html: false,
          transformPastedText: true,
          transformCopiedText: true,
        }),
        CommentMark,
        Collaboration.configure({
          document: s.doc,
        }),
        CollaborationCursor.configure({
          provider: {
            awareness: s.awareness,
          } as never,
          user: {
            name: user?.name ?? "You",
            color: user?.color ?? "#0ea5e9",
          },
        }),
      ],
      editorProps: {
        attributes: {
          class: "moraine-prosemirror focus:outline-none",
          spellcheck: "true",
        },
      },
      onCreate: ({ editor: ed }) => {
        const fragment = s.doc.getXmlFragment("default");
        if (fragment.length === 0 && initialMarkdown && !seeded) {
          seeded = true;
          ed.commands.setContent(initialMarkdown);
        }
        onReady?.(ed);
      },
      onUpdate: ({ editor: ed }) => {
        onUpdate(getMarkdown(ed));
      },
    });

    return () => {
      editor?.destroy();
      editor = null;
      lastRoom = null;
    };
  });

  function getMarkdown(ed: Editor): string {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const storage = ed.storage as any;
    if (storage.markdown?.getMarkdown) {
      return storage.markdown.getMarkdown() as string;
    }
    return ed.getText();
  }

  export function setMarkdown(md: string) {
    if (!editor) return;
    editor.commands.setContent(md);
  }

  export function getMarkdownContent(): string {
    if (!editor) return "";
    return getMarkdown(editor);
  }

  export function getSelectionQuote(): string | null {
    if (!editor || editor.state.selection.empty) return null;
    const { from, to } = editor.state.selection;
    const text = editor.state.doc.textBetween(from, to, " ");
    return text.trim() ? text : null;
  }

  export function applyCommentMark(id: string, kind: MarkKind = "comment"): boolean {
    if (!editor || editor.state.selection.empty) return false;
    return editor.chain().focus().setComment(id, kind).run();
  }

  export function clearCommentMark(id: string): void {
    editor?.commands.unsetCommentById(id);
  }

  export function focusComment(id: string): void {
    if (!editor) return;
    const found = findMarkRange(editor.state, id);
    if (found) {
      editor.chain().focus().setTextSelection(found).scrollIntoView().run();
    }
  }

  /** Replace marked range (or first quote match) with replacement text. */
  export function acceptSuggestion(
    id: string,
    replacement: string,
    quote?: string,
  ): boolean {
    if (!editor) return false;
    let range = findMarkRange(editor.state, id);
    if (!range && quote) {
      range = findQuoteRangeInDoc(editor.state.doc, quote);
    }
    if (!range) return false;
    const { from, to } = range;
    return editor
      .chain()
      .focus()
      .command(({ tr, dispatch }) => {
        tr.insertText(replacement, from, to);
        const type = tr.doc.type.schema.marks.comment;
        if (type) {
          tr.doc.descendants((node, pos) => {
            if (!node.isText) return;
            for (const mark of node.marks) {
              if (mark.type === type && mark.attrs.id === id) {
                tr.removeMark(pos, pos + node.nodeSize, type);
              }
            }
          });
        }
        if (dispatch) dispatch(tr);
        return true;
      })
      .run();
  }

  /**
   * Re-apply marks from sidecar records by quote search.
   * Returns ids that could not be placed (quote missing / ambiguous).
   */
  export function rehydrateMarks(
    records: CommentRecord[],
  ): { applied: string[]; orphaned: string[] } {
    const applied: string[] = [];
    const orphaned: string[] = [];
    if (!editor) return { applied, orphaned };

    for (const r of records) {
      if (r.resolved) continue;
      if (findMarkRange(editor.state, r.id)) {
        applied.push(r.id);
        continue;
      }
      const range = findQuoteRangeInDoc(editor.state.doc, r.quote);
      if (!range) {
        orphaned.push(r.id);
        continue;
      }
      const ok = editor
        .chain()
        .setTextSelection(range)
        .setComment(r.id, r.kind === "suggestion" ? "suggestion" : "comment")
        .run();
      if (ok) applied.push(r.id);
      else orphaned.push(r.id);
    }
    return { applied, orphaned };
  }

  onDestroy(() => {
    editor?.destroy();
    editor = null;
  });
</script>

<div class="moraine-scroll h-full overflow-auto" style="background: var(--bg);">
  <div bind:this={element} class="h-full"></div>
</div>
