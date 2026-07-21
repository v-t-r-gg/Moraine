export interface DocumentMeta {
  id: string;
  path: string;
  title: string;
  dirty: boolean;
  lastSavedAt: string | null;
  lastModifiedOnDisk: string | null;
  byteLen: number;
}

export interface DocumentSnapshot {
  meta: DocumentMeta;
  content: string;
  /** SHA-256 of exact UTF-8 content bytes (same rules as run ledger). */
  contentHash?: string;
}

export interface HistoryEntryMeta {
  id: string;
  createdAt: string;
  label: string | null;
  contentHash: number;
  source: string;
  byteLen: number;
}

export interface HistoryEntry extends HistoryEntryMeta {
  content: string;
}

export interface AppInfo {
  name: string;
  version: string;
  gitCommit?: string;
  dataDir: string;
  historyDir: string;
  configDir: string;
  /** C2 About / Diagnostics */
  serviceOnline?: boolean;
  serviceVersion?: string | null;
  serviceCompatible?: boolean;
  doctorHint?: string;
}

export interface FileChangedEvent {
  path: string;
  change: string;
  documentId: string | null;
  diskContentHash?: string | null;
  knownContentHash?: string | null;
  contentChanged?: boolean | null;
}

export type ViewMode = "edit" | "preview" | "split";
