/**
 * Desktop review workspace entry.
 *
 * Implementation lives under features/review-workspace so product panels stay
 * focused; this file keeps the historical import path stable.
 */
export { ReviewWorkspace as Workspace } from "@/features/review-workspace/ReviewWorkspace";
export type { ReviewWorkspaceProps as WorkspaceProps } from "@/features/review-workspace/ReviewWorkspace";
