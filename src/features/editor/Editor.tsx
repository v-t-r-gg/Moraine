import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
} from "react";
import { Editor as TiptapEditor, Extension } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import Placeholder from "@tiptap/extension-placeholder";
import Collaboration from "@tiptap/extension-collaboration";
import CollaborationCursor from "@tiptap/extension-collaboration-cursor";
import { Markdown } from "tiptap-markdown";
import { CommentMark, findMarkRange, type MarkKind } from "@/features/editor/commentMark";
import { findQuoteRangeInDoc, type CommentRecord } from "@/features/editor/comments";
import type { YjsSession } from "@/features/editor/yjsSession";
import {
  createManagedRegionPlugin,
  isProtocolRunMarkdown,
  suggestionAcceptTouchesManaged,
} from "@/features/editor/managedRegion";

export type ViewportState = {
  scrollTop: number;
  selectionFrom: number | null;
  selectionTo: number | null;
};

export type EditorHandle = {
  setMarkdown: (md: string) => void;
  getMarkdownContent: () => string;
  isProtocolMode: () => boolean;
  selectionTouchesManagedRegion: () => boolean;
  getViewportState: () => ViewportState;
  restoreViewportState: (state: ViewportState) => void;
  getSelectionQuote: () => string | null;
  applyCommentMark: (id: string, kind?: MarkKind) => boolean;
  clearCommentMark: (id: string) => void;
  focusComment: (id: string) => void;
  suggestionTargetsManagedRegion: (id: string, quote?: string) => boolean;
  acceptSuggestion: (id: string, replacement: string, quote?: string) => boolean;
  rehydrateMarks: (records: CommentRecord[]) => { applied: string[]; orphaned: string[] };
};

export interface EditorProps {
  session: YjsSession | null;
  initialMarkdown: string;
  editable?: boolean;
  onUpdate: (markdown: string) => void;
  onReady?: (editor: TiptapEditor) => void;
}

function getMarkdown(ed: TiptapEditor): string {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const storage = ed.storage as any;
  if (storage.markdown?.getMarkdown) {
    return storage.markdown.getMarkdown() as string;
  }
  return ed.getText();
}

export const Editor = forwardRef<EditorHandle, EditorProps>(function Editor(
  { session, initialMarkdown, editable = true, onUpdate, onReady },
  ref,
) {
  const elementRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const editorRef = useRef<TiptapEditor | null>(null);
  const protocolModeRef = useRef(false);
  const lastRoomRef = useRef<string | null>(null);
  const onUpdateRef = useRef(onUpdate);
  const onReadyRef = useRef(onReady);
  const initialMdRef = useRef(initialMarkdown);
  const [, setTick] = useState(0);

  onUpdateRef.current = onUpdate;
  onReadyRef.current = onReady;
  initialMdRef.current = initialMarkdown;

  useEffect(() => {
    const el = elementRef.current;
    const s = session;
    if (!el || !s) return;

    // Same room: keep editor instance (Strict Mode remount reuses room).
    if (editorRef.current && lastRoomRef.current === s.roomId) {
      return;
    }

    editorRef.current?.destroy();
    editorRef.current = null;
    lastRoomRef.current = s.roomId;
    protocolModeRef.current = isProtocolRunMarkdown(initialMdRef.current);

    const user = s.awareness.getLocalState()?.user as
      | { name: string; color: string }
      | undefined;

    let seeded = false;

    const ManagedRegionExt = Extension.create({
      name: "moraineManagedRegion",
      addProseMirrorPlugins() {
        return [createManagedRegionPlugin(() => protocolModeRef.current)];
      },
    });

    const editor = new TiptapEditor({
      element: el,
      editable,
      extensions: [
        StarterKit.configure({ history: false }),
        Placeholder.configure({
          placeholder: "Write or review the agent run record…",
        }),
        Markdown.configure({
          html: false,
          transformPastedText: true,
          transformCopiedText: true,
        }),
        CommentMark,
        ManagedRegionExt,
        Collaboration.configure({ document: s.doc }),
        CollaborationCursor.configure({
          provider: { awareness: s.awareness } as never,
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
        if (fragment.length === 0 && initialMdRef.current && !seeded) {
          seeded = true;
          ed.commands.setContent(initialMdRef.current);
          protocolModeRef.current = isProtocolRunMarkdown(initialMdRef.current);
        }
        onReadyRef.current?.(ed);
        setTick((t) => t + 1);
      },
      onUpdate: ({ editor: ed }) => {
        onUpdateRef.current(getMarkdown(ed));
      },
    });

    editorRef.current = editor;

    return () => {
      editor.destroy();
      if (editorRef.current === editor) {
        editorRef.current = null;
      }
      lastRoomRef.current = null;
    };
  }, [session, editable]);

  useImperativeHandle(ref, () => ({
    setMarkdown(md: string) {
      const editor = editorRef.current;
      if (!editor) return;
      protocolModeRef.current = isProtocolRunMarkdown(md);
      editor.commands.setContent(md);
    },
    getMarkdownContent() {
      const editor = editorRef.current;
      if (!editor) return "";
      return getMarkdown(editor);
    },
    isProtocolMode() {
      return protocolModeRef.current;
    },
    selectionTouchesManagedRegion() {
      const editor = editorRef.current;
      if (!editor || !protocolModeRef.current) return false;
      const { from, to } = editor.state.selection;
      return suggestionAcceptTouchesManaged(editor.state.doc, from, to);
    },
    getViewportState() {
      const scrollEl = scrollRef.current;
      const sel = editorRef.current?.state.selection;
      return {
        scrollTop: scrollEl?.scrollTop ?? 0,
        selectionFrom: sel?.from ?? null,
        selectionTo: sel?.to ?? null,
      };
    },
    restoreViewportState(state: ViewportState) {
      const scrollEl = scrollRef.current;
      if (scrollEl) {
        const max = Math.max(0, scrollEl.scrollHeight - scrollEl.clientHeight);
        scrollEl.scrollTop = Math.min(state.scrollTop, max);
      }
      const editor = editorRef.current;
      if (editor && state.selectionFrom != null && state.selectionTo != null) {
        const maxPos = editor.state.doc.content.size;
        const from = Math.min(state.selectionFrom, maxPos);
        const to = Math.min(state.selectionTo, maxPos);
        try {
          editor.commands.setTextSelection({ from, to });
        } catch {
          /* selection no longer valid */
        }
      }
    },
    getSelectionQuote() {
      const editor = editorRef.current;
      if (!editor || editor.state.selection.empty) return null;
      const { from, to } = editor.state.selection;
      const text = editor.state.doc.textBetween(from, to, " ");
      return text.trim() ? text : null;
    },
    applyCommentMark(id, kind = "comment") {
      const editor = editorRef.current;
      if (!editor || editor.state.selection.empty) return false;
      return editor.chain().focus().setComment(id, kind).run();
    },
    clearCommentMark(id) {
      editorRef.current?.commands.unsetCommentById(id);
    },
    focusComment(id) {
      const editor = editorRef.current;
      if (!editor) return;
      const found = findMarkRange(editor.state, id);
      if (found) {
        editor.chain().focus().setTextSelection(found).scrollIntoView().run();
      }
    },
    suggestionTargetsManagedRegion(id, quote) {
      const editor = editorRef.current;
      if (!editor || !protocolModeRef.current) return false;
      let range = findMarkRange(editor.state, id);
      if (!range && quote) {
        range = findQuoteRangeInDoc(editor.state.doc, quote);
      }
      if (!range) return false;
      return suggestionAcceptTouchesManaged(editor.state.doc, range.from, range.to);
    },
    acceptSuggestion(id, replacement, quote) {
      const editor = editorRef.current;
      if (!editor) return false;
      if (this.suggestionTargetsManagedRegion(id, quote)) return false;
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
    },
    rehydrateMarks(records) {
      const applied: string[] = [];
      const orphaned: string[] = [];
      const editor = editorRef.current;
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
    },
  }));

  return (
    <div
      ref={scrollRef}
      className="moraine-scroll h-full overflow-auto"
      style={{ background: "var(--bg)" }}
    >
      <div ref={elementRef} className="h-full" />
    </div>
  );
});
