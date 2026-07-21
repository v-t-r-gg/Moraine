/** Detect revision conflict messages from Tauri/core. */
export function isRevisionConflictError(err: unknown): boolean {
  const s = String(err);
  return s.includes("revision_conflict") || s.includes("Revision conflict");
}
