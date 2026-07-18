import type * as Y from "yjs";

export const COMMENTS_MAP = "comments";

export interface CommentRecord {
  id: string;
  body: string;
  author: string;
  quote: string;
  createdAt: string;
  resolved: boolean;
}

export type CommentMap = Y.Map<CommentRecord>;

export function commentsMap(doc: Y.Doc): CommentMap {
  return doc.getMap(COMMENTS_MAP) as CommentMap;
}

export function listComments(map: CommentMap, includeResolved = true): CommentRecord[] {
  const out: CommentRecord[] = [];
  map.forEach((value) => {
    if (!includeResolved && value.resolved) return;
    out.push(value);
  });
  out.sort((a, b) => b.createdAt.localeCompare(a.createdAt));
  return out;
}

export function upsertComment(map: CommentMap, record: CommentRecord): void {
  map.set(record.id, record);
}

export function setResolved(map: CommentMap, id: string, resolved: boolean): void {
  const cur = map.get(id);
  if (!cur) return;
  map.set(id, { ...cur, resolved });
}

export function removeComment(map: CommentMap, id: string): void {
  map.delete(id);
}

export function newCommentId(): string {
  return crypto.randomUUID();
}

/** Seed map from disk without overwriting live ids. */
export function mergeDiskIntoMap(
  map: CommentMap,
  disk: CommentRecord[],
): void {
  for (const c of disk) {
    if (!map.has(c.id)) {
      map.set(c.id, c);
    }
  }
}

export function mapToList(map: CommentMap): CommentRecord[] {
  return listComments(map, true);
}
