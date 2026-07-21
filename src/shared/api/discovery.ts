/** Framework-neutral discovery API (Tauri invoke wrappers). */

import { isTauriRuntime } from "./index";

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauriRuntime()) {
    return browserDiscoveryStub<T>(cmd, args);
  }
  const { invoke: tauriInvoke } = await import("@tauri-apps/api/core");
  return tauriInvoke<T>(cmd, args);
}

export interface ProjectRunCounts {
  active: number;
  ready: number;
  recent: number;
}

export interface ProjectSummaryDto {
  projectId: string;
  name: string;
  rootPath: string;
  available: boolean;
  runCounts: ProjectRunCounts;
  openFindingCount: number;
  lastActivityAt?: string | null;
  warning?: string | null;
}

export interface RunSummaryDto {
  runId: string;
  projectId: string;
  objective: string;
  lifecycle: string;
  provisional: boolean;
  captureCoverage: string;
  recordPath: string;
  absolutePath: string;
  startedAt?: string | null;
  updatedAt?: string | null;
  checkpointCount: number;
  evidenceCount: number;
  openFindingCount: number;
  riskCount: number;
  openQuestionCount: number;
  appendOnlyOpCount: number;
  integrity: string;
  recoveryRequired: boolean;
  error?: string | null;
  contentHash?: string | null;
}

export interface TimelineEntryDto {
  id: string;
  timestamp: string;
  kind: string;
  actorCategory?: string | null;
  targetId?: string | null;
  summary: string;
  detail?: string | null;
  provenance?: string | null;
}

export interface RunDetailDto {
  summary: RunSummaryDto;
  timeline: TimelineEntryDto[];
  isProtocolRun: boolean;
  objective?: string | null;
  risks: string[];
  openQuestions: string[];
}

export interface DiscoveryStatusDto {
  online: boolean;
  revision: number;
  mode: string;
  message?: string | null;
}

export async function discoveryStatus(): Promise<DiscoveryStatusDto> {
  return invoke("discovery_status");
}

export async function discoveryProjects(scanRoot?: string | null): Promise<ProjectSummaryDto[]> {
  return invoke("discovery_projects", { scanRoot: scanRoot ?? null });
}

export async function discoveryRuns(args: {
  projectId: string;
  rootPath?: string | null;
  category?: string | null;
  openFindingsOnly?: boolean;
  hasRisks?: boolean;
  hasQuestions?: boolean;
  query?: string | null;
  captureCoverage?: string | null;
}): Promise<RunSummaryDto[]> {
  return invoke("discovery_runs", {
    projectId: args.projectId,
    rootPath: args.rootPath ?? null,
    category: args.category ?? null,
    openFindingsOnly: args.openFindingsOnly ?? false,
    hasRisks: args.hasRisks ?? false,
    hasQuestions: args.hasQuestions ?? false,
    query: args.query ?? null,
    captureCoverage: args.captureCoverage ?? null,
  });
}

export async function discoveryRunDetail(args: {
  path?: string | null;
  runId?: string | null;
  projectRoot?: string | null;
}): Promise<RunDetailDto> {
  return invoke("discovery_run_detail", {
    path: args.path ?? null,
    runId: args.runId ?? null,
    projectRoot: args.projectRoot ?? null,
  });
}

export async function discoveryRebuildIndex(scanRoot?: string | null): Promise<unknown> {
  return invoke("discovery_rebuild_index", { scanRoot: scanRoot ?? null });
}

export async function discoveryRescanProject(projectId: string): Promise<unknown> {
  return invoke("discovery_rescan_project", { projectId });
}

export async function discoveryAddExistingProject(path: string): Promise<ProjectSummaryDto> {
  return invoke("discovery_add_existing_project", { path });
}

function browserDiscoveryStub<T>(cmd: string, _args?: Record<string, unknown>): T {
  switch (cmd) {
    case "discovery_status":
      return {
        online: false,
        revision: 0,
        mode: "browser",
        message: "Browser mode: discovery requires the Tauri host",
      } as T;
    case "discovery_projects":
      return [] as T;
    case "discovery_runs":
      return [] as T;
    case "discovery_run_detail":
      throw new Error("discovery requires the Tauri desktop host");
    case "discovery_rebuild_index":
    case "discovery_rescan_project":
      return { ok: false, mode: "browser" } as T;
    case "discovery_add_existing_project":
      throw new Error("add project requires the Tauri desktop host");
    default:
      return undefined as T;
  }
}
