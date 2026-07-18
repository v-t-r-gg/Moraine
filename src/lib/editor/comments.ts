import type * as Y from "yjs";

export const COMMENTS_MAP = "comments";

export type AnnotationKind = "comment" | "suggestion";

export type SuggestionDisposition =
  | "pending"
  | "accepting"
  | "accepted"
  | "rejected"
  | "resolved_legacy";

export interface CommentRecord {
  id: string;
  body: string;
  author: string;
  quote: string;
  createdAt: string;
  resolved: boolean;
  kind: AnnotationKind;
  revision: number;
  disposition?: SuggestionDisposition | null;
  acceptanceOpId?: string | null;
  acceptanceBaseHash?: string | null;
  acceptanceStartedAt?: string | null;
  appliedContentHash?: string | null;
  acceptanceCompletedAt?: string | null;
}

export type CommentMap = Y.Map<CommentRecord>;

export function commentsMap(doc: Y.Doc): CommentMap {
  return doc.getMap(COMMENTS_MAP) as CommentMap;
}

export function listComments(map: CommentMap, includeResolved = true): CommentRecord[] {
  const out: CommentRecord[] = [];
  map.forEach((value) => {
    const rec = normalize(value);
    if (!includeResolved && isResolvedView(rec)) return;
    out.push(rec);
  });
  out.sort((a, b) => b.createdAt.localeCompare(a.createdAt));
  return out;
}

export function isResolvedView(rec: CommentRecord): boolean {
  if (rec.kind === "suggestion") {
    const d = rec.disposition ?? (rec.resolved ? "resolved_legacy" : "pending");
    return d === "accepted" || d === "rejected" || d === "resolved_legacy";
  }
  return rec.resolved;
}

export function dispositionLabel(rec: CommentRecord): string {
  if (rec.kind !== "suggestion") {
    return rec.resolved ? "Resolved" : "Open";
  }
  switch (rec.disposition ?? (rec.resolved ? "resolved_legacy" : "pending")) {
    case "pending":
      return "Pending";
    case "accepting":
      return "Accepting (incomplete)";
    case "accepted":
      return "Accepted";
    case "rejected":
      return "Rejected";
    case "resolved_legacy":
      return "Resolved (legacy)";
    default:
      return "Suggestion";
  }
}

function normalize(value: CommentRecord): CommentRecord {
  const kind: AnnotationKind = value.kind === "suggestion" ? "suggestion" : "comment";
  let disposition = value.disposition ?? null;
  if (kind === "suggestion" && !disposition) {
    disposition = value.resolved ? "resolved_legacy" : "pending";
  }
  if (kind === "comment") disposition = null;
  const resolved =
    kind === "suggestion"
      ? disposition === "accepted" ||
        disposition === "rejected" ||
        disposition === "resolved_legacy"
      : !!value.resolved;
  return {
    ...value,
    kind,
    revision: value.revision && value.revision > 0 ? value.revision : 1,
    disposition,
    resolved,
  };
}

export function applyDurableRecord(map: CommentMap, record: CommentRecord): void {
  map.set(record.id, normalize(record));
}

export function upsertComment(map: CommentMap, record: CommentRecord): void {
  map.set(record.id, normalize(record));
}

export function setResolved(map: CommentMap, id: string, resolved: boolean): void {
  const cur = map.get(id);
  if (!cur) return;
  const n = normalize(cur);
  if (n.kind === "suggestion") {
    map.set(id, {
      ...n,
      resolved,
      disposition: resolved ? "resolved_legacy" : "pending",
    });
  } else {
    map.set(id, { ...n, resolved });
  }
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

export type StructuredError = {
  kind: string;
  annotationId?: string | null;
  expectedRevision?: number | null;
  actualRevision?: number | null;
  message?: string;
};

export function parseStructuredError(err: unknown): StructuredError | null {
  const s = String(err);
  // Tauri may wrap JSON in "error: " or similar.
  const start = s.indexOf("{");
  const end = s.lastIndexOf("}");
  if (start < 0 || end <= start) return null;
  try {
    const obj = JSON.parse(s.slice(start, end + 1)) as StructuredError;
    if (obj && typeof obj.kind === "string") return obj;
  } catch {
    /* not JSON */
  }
  return null;
}

export function isAnnotationConflictError(err: unknown): boolean {
  const e = parseStructuredError(err);
  if (e) {
    return (
      e.kind === "annotation_conflict" ||
      e.kind === "incomplete_acceptance" ||
      e.kind === "document_revision_conflict"
    );
  }
  const s = String(err);
  return (
    s.includes("annotation_conflict") ||
    s.includes("incomplete_acceptance") ||
    s.includes("document_revision_conflict")
  );
}

export function applyAcceptToText(
  fullText: string,
  quote: string,
  replacement: string,
): string | null {
  const i = fullText.indexOf(quote);
  if (i < 0 || !quote) return null;
  return fullText.slice(0, i) + replacement + fullText.slice(i + quote.length);
}

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
    if (isResolvedView(r)) continue;
    if (r.kind === "suggestion") suggestions++;
    else comments++;
  }
  return { comments, suggestions };
}
