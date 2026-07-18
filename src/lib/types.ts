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
  dataDir: string;
  historyDir: string;
  configDir: string;
}

export interface FileChangedEvent {
  path: string;
  change: string;
  documentId: string | null;
}

export type ViewMode = "edit" | "preview" | "split";
