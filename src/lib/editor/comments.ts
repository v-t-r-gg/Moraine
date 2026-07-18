import type * as Y from "yjs";

export const COMMENTS_MAP = "comments";

export type AnnotationKind = "comment" | "suggestion";

export interface CommentRecord {
  id: string;
  /** Comment text, or suggested replacement when kind is suggestion. */
  body: string;
  author: string;
  /** Original selection (suggestion: text to replace). */
  quote: string;
  createdAt: string;
  resolved: boolean;
  kind: AnnotationKind;
}

export type CommentMap = Y.Map<CommentRecord>;

export function commentsMap(doc: Y.Doc): CommentMap {
  return doc.getMap(COMMENTS_MAP) as CommentMap;
}

export function listComments(map: CommentMap, includeResolved = true): CommentRecord[] {
  const out: CommentRecord[] = [];
  map.forEach((value) => {
    const rec = normalize(value);
    if (!includeResolved && rec.resolved) return;
    out.push(rec);
  });
  out.sort((a, b) => b.createdAt.localeCompare(a.createdAt));
  return out;
}

function normalize(value: CommentRecord): CommentRecord {
  return {
    ...value,
    kind: value.kind === "suggestion" ? "suggestion" : "comment",
  };
}

export function upsertComment(map: CommentMap, record: CommentRecord): void {
  map.set(record.id, normalize(record));
}

export function setResolved(map: CommentMap, id: string, resolved: boolean): void {
  const cur = map.get(id);
  if (!cur) return;
  map.set(id, { ...normalize(cur), resolved });
}

export function removeComment(map: CommentMap, id: string): void {
  map.delete(id);
}

export function newCommentId(): string {
  return crypto.randomUUID();
}

export function mergeDiskIntoMap(map: CommentMap, disk: CommentRecord[]): void {
  for (const c of disk) {
    if (!map.has(c.id)) {
      map.set(c.id, normalize(c));
    }
  }
}

/** Pure accept/reject helpers for tests (doc text transformation). */
export function applyAcceptToText(
  fullText: string,
  quote: string,
  replacement: string,
): string | null {
  const i = fullText.indexOf(quote);
  if (i < 0 || !quote) return null;
  return fullText.slice(0, i) + replacement + fullText.slice(i + quote.length);
}
