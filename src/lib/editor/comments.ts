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
  /** Durable concurrency token from the ledger (default 1). */
  revision: number;
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
    revision: value.revision && value.revision > 0 ? value.revision : 1,
  };
}

/** Apply durable op result into the Yjs map (source of truth after host mutation). */
export function applyDurableRecord(map: CommentMap, record: CommentRecord): void {
  map.set(record.id, normalize(record));
}

export function isAnnotationConflictError(err: unknown): boolean {
  return String(err).includes("annotation_conflict");
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

/** Map plain-text index of quote to PM positions (contiguous text only). */
export function findQuoteRangeInDoc(
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  doc: any,
  quote: string,
): { from: number; to: number } | null {
  if (!quote) return null;
  let plain = "";
  const indexToPos: number[] = [];
  doc.descendants((node: { isText?: boolean; text?: string }, pos: number) => {
    if (!node.isText || !node.text) return;
    for (let i = 0; i < node.text.length; i++) {
      indexToPos.push(pos + i);
      plain += node.text[i];
    }
  });
  const idx = plain.indexOf(quote);
  if (idx < 0) return null;
  const from = indexToPos[idx];
  const last = indexToPos[idx + quote.length - 1];
  if (from == null || last == null) return null;
  return { from, to: last + 1 };
}

export function countPending(records: CommentRecord[]): {
  comments: number;
  suggestions: number;
} {
  let comments = 0;
  let suggestions = 0;
  for (const r of records) {
    if (r.resolved) continue;
    if (r.kind === "suggestion") suggestions++;
    else comments++;
  }
  return { comments, suggestions };
}
