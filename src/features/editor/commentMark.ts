import { Mark, mergeAttributes } from "@tiptap/core";

export type MarkKind = "comment" | "suggestion";

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    comment: {
      setComment: (id: string, kind?: MarkKind) => ReturnType;
      unsetComment: () => ReturnType;
      unsetCommentById: (id: string) => ReturnType;
    };
  }
}

export const CommentMark = Mark.create({
  name: "comment",
  inclusive: false,
  excludes: "",

  addAttributes() {
    return {
      id: {
        default: null,
        parseHTML: (el) => el.getAttribute("data-comment-id"),
        renderHTML: (attrs) => (attrs.id ? { "data-comment-id": attrs.id } : {}),
      },
      kind: {
        default: "comment",
        parseHTML: (el) => el.getAttribute("data-kind") || "comment",
        renderHTML: (attrs) => ({ "data-kind": attrs.kind || "comment" }),
      },
    };
  },

  parseHTML() {
    return [{ tag: "span[data-comment-id]" }];
  },

  renderHTML({ HTMLAttributes }) {
    const kind = (HTMLAttributes["data-kind"] as string) || "comment";
    const cls =
      kind === "suggestion" ? "moraine-suggestion-mark" : "moraine-comment-mark";
    return [
      "span",
      mergeAttributes(HTMLAttributes, { class: cls }),
      0,
    ];
  },

  addCommands() {
    return {
      setComment:
        (id: string, kind: MarkKind = "comment") =>
        ({ commands }) =>
          commands.setMark(this.name, { id, kind }),
      unsetComment:
        () =>
        ({ commands }) =>
          commands.unsetMark(this.name),
      unsetCommentById:
        (id: string) =>
        ({ tr, state, dispatch }) => {
          const type = state.schema.marks[this.name];
          if (!type) return false;
          let changed = false;
          state.doc.descendants((node, pos) => {
            if (!node.isText) return;
            node.marks.forEach((mark) => {
              if (mark.type === type && mark.attrs.id === id) {
                tr.removeMark(pos, pos + node.nodeSize, type);
                changed = true;
              }
            });
          });
          if (changed && dispatch) dispatch(tr);
          return changed;
        },
    };
  },
});

export function findMarkRange(
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  state: any,
  id: string,
): { from: number; to: number } | null {
  const type = state.schema.marks.comment;
  if (!type) return null;
  let from: number | null = null;
  let to: number | null = null;
  state.doc.descendants((node: { isText: boolean; marks: { type: unknown; attrs: { id?: string } }[]; nodeSize: number }, pos: number) => {
    if (!node.isText) return;
    for (const mark of node.marks) {
      if (mark.type === type && mark.attrs.id === id) {
        if (from == null) from = pos;
        to = pos + node.nodeSize;
      }
    }
  });
  if (from == null || to == null) return null;
  return { from, to };
}
