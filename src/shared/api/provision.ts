/** Provisioning / onboarding API (Tauri invoke wrappers → moraine-provision). */

import { isTauriRuntime } from "./index";

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauriRuntime()) {
    return browserProvisionStub<T>(cmd, args);
  }
  const { invoke: tauriInvoke } = await import("@tauri-apps/api/core");
  return tauriInvoke<T>(cmd, args);
}

export interface DetectedAgentDto {
  kind: string;
  id: string;
  displayName: string;
  detected: boolean;
  executable?: string | null;
  version?: string | null;
  status: string;
  statusMessage: string;
}

export interface ProjectCandidateDto {
  path: string;
  name: string;
  initialized: boolean;
  isGit: boolean;
}

export interface ServiceStateDto {
  installed: boolean;
  running: boolean;
  binaryPresent: boolean;
  binaryPath?: string | null;
  unitPath?: string | null;
  version?: string | null;
  statusMessage: string;
  platform: string;
}

export interface SuiteStateDto {
  prefix: string;
  cliPath: string;
  cliPresent: boolean;
  servicePath: string;
  servicePresent: boolean;
  desktopPath: string;
  desktopPresent: boolean;
  manifestPath: string;
  manifestPresent: boolean;
  version?: string | null;
  componentsCoherent: boolean;
}

export interface SystemStateDto {
  suite: SuiteStateDto;
  service: ServiceStateDto;
  agents: DetectedAgentDto[];
  projects: ProjectCandidateDto[];
  readiness: string;
}

export interface SetupIntentDto {
  project: string;
  agent?: string;
  enableAutostart?: boolean;
  skipService?: boolean;
}

export interface ProvisionOperationDto {
  id: string;
  kind: string;
  productLabel: string;
  detail: string;
  reversible: boolean;
}

export interface SetupStateWitnessDto {
  project: string;
  absoluteCli: string;
  projectInitialized: boolean;
  serviceInstalled: boolean;
  serviceRunning: boolean;
  enableAutostart: boolean;
  skipService: boolean;
}

export interface SetupPlanDto {
  planId: string;
  intent: {
    project: string;
    agent: string;
    enableAutostart: boolean;
    skipService?: boolean;
  };
  operations: ProvisionOperationDto[];
  warnings: { code: string; message: string; technicalDetail?: string | null }[];
  absoluteCli: string;
  productSummary: string[];
  stateWitness: SetupStateWitnessDto;
}

export interface SetupReceiptDto {
  transactionId: string;
  completed: {
    id: string;
    kind: string;
    productLabel: string;
    success: boolean;
    message?: string | null;
  }[];
  readiness: string;
  failedOperation?: string | null;
  error?: string | null;
  journalPath: string;
}

export interface VerificationStepDto {
  id: string;
  productLabel: string;
  passed: boolean;
  message: string;
  technicalDetail?: string | null;
}

export interface VerificationReportDto {
  ok: boolean;
  readiness: string;
  steps: VerificationStepDto[];
  runId?: string | null;
  projectPath?: string | null;
  userMessage: string;
}

export interface RepairActionDto {
  id: string;
  label: string;
  kind: string;
  project?: string | null;
  agent?: string | null;
}

export interface HealthCheckDto {
  id: string;
  status: string;
  userMessage: string;
  technicalDetail: string;
  repair?: RepairActionDto | null;
}

export interface HealthReportDto {
  ok: boolean;
  checks: HealthCheckDto[];
  readiness: string;
}

export interface RepairResultDto {
  ok: boolean;
  actionId: string;
  userMessage: string;
  technicalDetail?: string | null;
}

export async function provisionInspect(): Promise<SystemStateDto> {
  return invoke("provision_inspect");
}

export async function provisionPlan(intent: SetupIntentDto): Promise<SetupPlanDto> {
  return invoke("provision_plan", { intent });
}

export async function provisionApply(intent: SetupIntentDto): Promise<SetupReceiptDto> {
  return invoke("provision_apply", { intent });
}

export async function provisionApplyPlan(plan: SetupPlanDto): Promise<SetupReceiptDto> {
  return invoke("provision_apply_plan", { plan });
}

export async function provisionRollback(receipt: SetupReceiptDto): Promise<void> {
  return invoke("provision_rollback", { receipt });
}

export async function provisionVerify(intent: SetupIntentDto): Promise<VerificationReportDto> {
  return invoke("provision_verify", { intent });
}

export async function provisionHealth(
  project?: string | null,
  agent?: string | null,
): Promise<HealthReportDto> {
  return invoke("provision_health", {
    project: project ?? null,
    agent: agent ?? null,
  });
}

export async function provisionRepair(action: RepairActionDto): Promise<RepairResultDto> {
  return invoke("provision_repair", { action });
}

export async function provisionEnable(intent: SetupIntentDto): Promise<SetupReceiptDto> {
  return invoke("provision_enable", { intent });
}

export async function provisionInitProject(
  path: string,
): Promise<{ ok: boolean; projectRoot: string; projectId: string; created: boolean }> {
  return invoke("provision_init_project", { path });
}

function browserProvisionStub<T>(cmd: string, _args?: Record<string, unknown>): T {
  switch (cmd) {
    case "provision_inspect":
      return {
        suite: {
          prefix: "",
          cliPath: "",
          cliPresent: false,
          servicePath: "",
          servicePresent: false,
          desktopPath: "",
          desktopPresent: false,
          manifestPath: "",
          manifestPresent: false,
          componentsCoherent: true,
        },
        service: {
          installed: false,
          running: false,
          binaryPresent: false,
          statusMessage: "Browser mode — open the desktop app to set up Moraine",
          platform: "browser",
        },
        agents: [
          {
            kind: "codex",
            id: "codex",
            displayName: "Codex",
            detected: false,
            status: "notFound",
            statusMessage: "Open the desktop app to detect agents",
          },
        ],
        projects: [],
        readiness: "notConfigured",
      } as T;
    case "provision_health":
      return {
        ok: true,
        checks: [],
        readiness: "notConfigured",
      } as T;
    case "provision_plan":
    case "provision_apply":
    case "provision_apply_plan":
    case "provision_enable":
    case "provision_verify":
    case "provision_init_project":
    case "provision_repair":
      throw new Error("Setup requires the Moraine desktop app");
    case "provision_rollback":
      return undefined as T;
    default:
      return undefined as T;
  }
}
