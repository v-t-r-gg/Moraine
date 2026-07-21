/** Revision-based classification of filesystem change events. */

export type DiskWatchClassification =
  | "ignore_same_hash"
  | "ignore_duplicate"
  | "ignore_sidecar"
  | "ignore_while_saving"
  | "external_clean"
  | "external_dirty";

export interface DiskWatchEvent {
  path: string;
  change: string;
  documentId: string | null;
  /** Authoritative SHA-256 of current Markdown bytes on disk, when available. */
  diskContentHash?: string | null;
  knownContentHash?: string | null;
  contentChanged?: boolean | null;
}

export function isMarkdownSidecarPath(path: string): boolean {
  return (
    path.endsWith(".moraine.json") ||
    path.endsWith(".moraine.json.lock") ||
    path.endsWith(".comments.json") ||
    path.endsWith(".comments.json.migrated") ||
    path.includes(".tmp")
  );
}

/**
 * Classify a disk event for an open document.
 * Prefer content hash comparison over raw filesystem notifications.
 */
export function classifyDiskEvent(opts: {
  event: DiskWatchEvent;
  openDocumentId: string | null;
  knownPersistedHash: string | null;
  lastHandledExternalHash: string | null;
  dirty: boolean;
  saving: boolean;
}): DiskWatchClassification {
  const { event, openDocumentId, knownPersistedHash, lastHandledExternalHash, dirty, saving } =
    opts;

  if (isMarkdownSidecarPath(event.path)) {
    return "ignore_sidecar";
  }
  if (!event.documentId || !openDocumentId || event.documentId !== openDocumentId) {
    return "ignore_same_hash";
  }

  // Backend may already state content did not change.
  if (event.contentChanged === false) {
    return "ignore_same_hash";
  }

  const diskHash = event.diskContentHash ?? null;
  if (diskHash && knownPersistedHash && diskHash === knownPersistedHash) {
    return "ignore_same_hash";
  }
  if (diskHash && lastHandledExternalHash && diskHash === lastHandledExternalHash) {
    return "ignore_duplicate";
  }

  // During Save, defer classification; caller should re-check after Save completes.
  if (saving) {
    return "ignore_while_saving";
  }

  // If we lack a disk hash, fall back conservatively only when backend marked changed.
  if (!diskHash && event.contentChanged !== true) {
    return "ignore_same_hash";
  }

  if (dirty) {
    return "external_dirty";
  }
  return "external_clean";
}

export interface ViewportState {
  scrollTop: number;
  selectionFrom: number | null;
  selectionTo: number | null;
}
