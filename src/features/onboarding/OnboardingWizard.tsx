/**
 * First-run / Enable Moraine wizard.
 * Product language only — no systemd, MCP, PATH, hooks, or .moraine education.
 */

import {
  useCallback,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import {
  provisionApply,
  provisionInspect,
  provisionPlan,
  provisionRollback,
  type DetectedAgentDto,
  type SetupPlanDto,
  type SetupReceiptDto,
  type SystemStateDto,
} from "@/shared/api/provision";
import { isTauri } from "@/shared/api";

export type WizardStep =
  | "welcome"
  | "agents"
  | "project"
  | "plan"
  | "apply"
  | "complete"
  | "failed";

export interface OnboardingWizardProps {
  onComplete: (projectPath: string) => void;
  onDismiss?: () => void;
  /** Pre-selected folder (optional). */
  initialProject?: string | null;
}

export function OnboardingWizard({
  onComplete,
  onDismiss,
  initialProject,
}: OnboardingWizardProps) {
  const [step, setStep] = useState<WizardStep>("welcome");
  const [system, setSystem] = useState<SystemStateDto | null>(null);
  const [projectPath, setProjectPath] = useState(initialProject ?? "");
  const [plan, setPlan] = useState<SetupPlanDto | null>(null);
  const [receipt, setReceipt] = useState<SetupReceiptDto | null>(null);
  const [progressLabel, setProgressLabel] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const st = await provisionInspect();
        if (!cancelled) setSystem(st);
      } catch (e) {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : String(e));
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const pickFolder = useCallback(async () => {
    if (!isTauri) {
      setError("Folder selection requires the Moraine desktop app");
      return;
    }
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select a project folder",
      });
      if (typeof selected === "string" && selected) {
        setProjectPath(selected);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const goPlan = useCallback(async () => {
    if (!projectPath.trim()) {
      setError("Choose a project folder first");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const p = await provisionPlan({
        project: projectPath.trim(),
        agent: "codex",
        enableAutostart: true,
      });
      setPlan(p);
      setStep("plan");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }, [projectPath]);

  const runApply = useCallback(async () => {
    if (!projectPath.trim()) return;
    setStep("apply");
    setBusy(true);
    setError(null);
    setProgressLabel("Preparing…");
    try {
      // Stream product-level progress from plan operation labels.
      if (plan) {
        for (const op of plan.operations) {
          setProgressLabel(op.productLabel);
        }
      }
      setProgressLabel("Applying setup…");
      const r = await provisionApply({
        project: projectPath.trim(),
        agent: "codex",
        enableAutostart: true,
      });
      setReceipt(r);
      if (r.readiness === "ready") {
        setProgressLabel("Ready");
        setStep("complete");
      } else {
        setError(r.error ?? "Setup did not complete successfully");
        setStep("failed");
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setStep("failed");
    } finally {
      setBusy(false);
    }
  }, [projectPath, plan]);

  const retry = useCallback(async () => {
    if (receipt && receipt.readiness === "rollbackRequired") {
      try {
        await provisionRollback(receipt);
      } catch {
        /* best-effort */
      }
    }
    setReceipt(null);
    setError(null);
    setStep("plan");
  }, [receipt]);

  const agents: DetectedAgentDto[] = system?.agents ?? [];
  const recentProjects = system?.projects ?? [];

  return (
    <div
      className="flex h-full flex-col"
      data-testid="onboarding-wizard"
      data-step={step}
      style={{ background: "var(--bg)", color: "var(--fg)" }}
    >
      <header
        className="flex h-11 shrink-0 items-center justify-between border-b px-4"
        style={{ borderColor: "var(--border)", background: "var(--panel)" }}
      >
        <span className="text-sm font-semibold">Enable Moraine</span>
        {onDismiss ? (
          <button
            type="button"
            className="text-[11px]"
            style={{ color: "var(--muted)" }}
            onClick={onDismiss}
          >
            Not now
          </button>
        ) : null}
      </header>

      <div className="moraine-scroll flex-1 overflow-auto px-6 py-8">
        <div className="mx-auto max-w-lg">
          {step === "welcome" ? (
            <WelcomeStep onNext={() => setStep("agents")} />
          ) : null}

          {step === "agents" ? (
            <AgentsStep
              agents={agents}
              onBack={() => setStep("welcome")}
              onNext={() => setStep("project")}
            />
          ) : null}

          {step === "project" ? (
            <ProjectStep
              projectPath={projectPath}
              recent={recentProjects}
              onPick={pickFolder}
              onChangePath={setProjectPath}
              onBack={() => setStep("agents")}
              onNext={() => void goPlan()}
              busy={busy}
            />
          ) : null}

          {step === "plan" && plan ? (
            <PlanStep
              plan={plan}
              onBack={() => setStep("project")}
              onApply={() => void runApply()}
              busy={busy}
            />
          ) : null}

          {step === "apply" ? (
            <ApplyStep
              progressLabel={progressLabel}
              receipt={receipt}
            />
          ) : null}

          {step === "complete" ? (
            <CompleteStep
              projectPath={projectPath}
              onDone={() => onComplete(projectPath)}
            />
          ) : null}

          {step === "failed" ? (
            <FailedStep
              error={error}
              onRetry={() => void retry()}
              onBack={() => setStep("project")}
            />
          ) : null}

          {error && step !== "failed" ? (
            <p className="mt-4 text-xs" style={{ color: "#b91c1c" }} role="alert">
              {error}
            </p>
          ) : null}
        </div>
      </div>
    </div>
  );
}

function WelcomeStep({ onNext }: { onNext: () => void }) {
  return (
    <div data-testid="wizard-welcome">
      <h1 className="text-xl font-semibold">Welcome to Moraine</h1>
      <p className="mt-3 text-sm leading-relaxed" style={{ color: "var(--muted)" }}>
        Moraine keeps a local record of coding-agent work next to your project.
      </p>
      <p className="mt-2 text-sm leading-relaxed" style={{ color: "var(--muted)" }}>
        You can browse runs and findings here any time — capture can continue even
        when this window is closed.
      </p>
      <div className="mt-8">
        <PrimaryButton onClick={onNext}>Continue</PrimaryButton>
      </div>
    </div>
  );
}

function AgentsStep({
  agents,
  onBack,
  onNext,
}: {
  agents: DetectedAgentDto[];
  onBack: () => void;
  onNext: () => void;
}) {
  const codex = agents.find((a) => a.id === "codex") ?? {
    id: "codex",
    displayName: "Codex",
    detected: false,
    status: "notFound",
    statusMessage: "Not detected yet",
    kind: "codex",
  };
  return (
    <div data-testid="wizard-agents">
      <h1 className="text-xl font-semibold">Coding agent</h1>
      <p className="mt-2 text-sm" style={{ color: "var(--muted)" }}>
        Moraine connects to the agent you already use.
      </p>
      <div
        className="mt-6 rounded-lg border p-4"
        style={{ borderColor: "var(--border)", background: "var(--panel)" }}
      >
        <div className="flex items-center justify-between">
          <span className="font-medium">{codex.displayName}</span>
          <span
            className="text-[11px] font-semibold"
            style={{ color: codex.detected ? "#16a34a" : "#b45309" }}
          >
            {codex.detected ? "Detected" : "Not found"}
          </span>
        </div>
        <p className="mt-1 text-xs" style={{ color: "var(--muted)" }}>
          {codex.statusMessage}
        </p>
        {codex.executable ? (
          <details className="mt-2 text-[10px]" style={{ color: "var(--muted)" }}>
            <summary>Advanced details</summary>
            <code className="mt-1 block break-all">{codex.executable}</code>
            {codex.version ? <div>Version: {codex.version}</div> : null}
          </details>
        ) : null}
      </div>
      {!codex.detected ? (
        <p className="mt-3 text-xs" style={{ color: "var(--muted)" }}>
          You can continue — Moraine will prepare the connection for when Codex is
          available.
        </p>
      ) : null}
      <div className="mt-8 flex gap-2">
        <SecondaryButton onClick={onBack}>Back</SecondaryButton>
        <PrimaryButton onClick={onNext}>Continue</PrimaryButton>
      </div>
    </div>
  );
}

function ProjectStep({
  projectPath,
  recent,
  onPick,
  onChangePath,
  onBack,
  onNext,
  busy,
}: {
  projectPath: string;
  recent: { path: string; name: string; initialized: boolean; isGit: boolean }[];
  onPick: () => void;
  onChangePath: (p: string) => void;
  onBack: () => void;
  onNext: () => void;
  busy: boolean;
}) {
  const name =
    projectPath.split("/").filter(Boolean).pop() ||
    projectPath ||
    "your project";
  return (
    <div data-testid="wizard-project">
      <h1 className="text-xl font-semibold">Select a project</h1>
      <p className="mt-2 text-sm" style={{ color: "var(--muted)" }}>
        Choose the folder where your coding agent works. Moraine will keep records
        next to that project.
      </p>
      <div className="mt-6 flex gap-2">
        <PrimaryButton onClick={onPick}>Choose folder…</PrimaryButton>
      </div>
      {projectPath ? (
        <div
          className="mt-4 rounded-lg border p-3 text-xs"
          style={{ borderColor: "var(--border)", background: "var(--panel)" }}
        >
          <div className="font-medium">{name}</div>
          <div className="mt-1 break-all" style={{ color: "var(--muted)" }}>
            {projectPath}
          </div>
          <p className="mt-2" style={{ color: "var(--muted)" }}>
            Moraine will prepare local records for this folder.
          </p>
        </div>
      ) : null}
      {recent.length > 0 ? (
        <div className="mt-6">
          <div className="text-[11px] font-semibold" style={{ color: "var(--muted)" }}>
            Nearby projects
          </div>
          <ul className="mt-2 grid gap-1">
            {recent.slice(0, 8).map((p) => (
              <li key={p.path}>
                <button
                  type="button"
                  className="w-full rounded border px-2 py-1.5 text-left text-xs"
                  style={{
                    borderColor: "var(--border)",
                    background:
                      projectPath === p.path ? "var(--accent-soft)" : "var(--bg)",
                  }}
                  onClick={() => onChangePath(p.path)}
                >
                  <span className="font-medium">{p.name}</span>
                  {p.isGit ? (
                    <span className="ml-1" style={{ color: "var(--muted)" }}>
                      · git
                    </span>
                  ) : null}
                </button>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      <div className="mt-8 flex gap-2">
        <SecondaryButton onClick={onBack}>Back</SecondaryButton>
        <PrimaryButton onClick={onNext} disabled={!projectPath || busy}>
          {busy ? "Checking…" : "Continue"}
        </PrimaryButton>
      </div>
    </div>
  );
}

function PlanStep({
  plan,
  onBack,
  onApply,
  busy,
}: {
  plan: SetupPlanDto;
  onBack: () => void;
  onApply: () => void;
  busy: boolean;
}) {
  return (
    <div data-testid="wizard-plan">
      <h1 className="text-xl font-semibold">Planned changes</h1>
      <p className="mt-2 text-sm" style={{ color: "var(--muted)" }}>
        Moraine will:
      </p>
      <ul className="mt-4 grid gap-2 text-sm">
        {plan.productSummary.map((line) => (
          <li key={line} className="flex gap-2">
            <span style={{ color: "#16a34a" }}>✓</span>
            <span>{line}</span>
          </li>
        ))}
      </ul>
      {plan.warnings.length > 0 ? (
        <div className="mt-4 text-xs" style={{ color: "#b45309" }}>
          {plan.warnings.map((w) => (
            <p key={w.code}>{w.message}</p>
          ))}
        </div>
      ) : null}
      <div className="mt-8 flex gap-2">
        <SecondaryButton onClick={onBack}>Back</SecondaryButton>
        <PrimaryButton onClick={onApply} disabled={busy}>
          Enable Moraine
        </PrimaryButton>
      </div>
    </div>
  );
}

function ApplyStep({
  progressLabel,
  receipt,
}: {
  progressLabel: string;
  receipt: SetupReceiptDto | null;
}) {
  return (
    <div data-testid="wizard-apply">
      <h1 className="text-xl font-semibold">Setting up…</h1>
      <p className="mt-4 text-sm" style={{ color: "var(--muted)" }}>
        {progressLabel || "Working…"}
      </p>
      {receipt ? (
        <ul className="mt-4 grid gap-1 text-xs">
          {receipt.completed.map((c) => (
            <li key={c.id}>
              {c.success ? "✓" : "✗"} {c.productLabel}
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}

function CompleteStep({
  projectPath,
  onDone,
}: {
  projectPath: string;
  onDone: () => void;
}) {
  return (
    <div data-testid="wizard-complete">
      <h1 className="text-xl font-semibold">Moraine is ready</h1>
      <p className="mt-3 text-sm leading-relaxed" style={{ color: "var(--muted)" }}>
        Capture works for this project. Use your coding agent as usual — new runs
        will appear here.
      </p>
      <p className="mt-2 text-xs break-all" style={{ color: "var(--muted)" }}>
        {projectPath}
      </p>
      <div className="mt-8">
        <PrimaryButton onClick={onDone}>Open project</PrimaryButton>
      </div>
    </div>
  );
}

function FailedStep({
  error,
  onRetry,
  onBack,
}: {
  error: string | null;
  onRetry: () => void;
  onBack: () => void;
}) {
  return (
    <div data-testid="wizard-failed">
      <h1 className="text-xl font-semibold">Setup needs attention</h1>
      <p className="mt-3 text-sm" style={{ color: "#b91c1c" }}>
        {error ?? "Something went wrong during setup."}
      </p>
      <p className="mt-2 text-xs" style={{ color: "var(--muted)" }}>
        You can retry. Moraine will reverse incomplete changes when possible.
      </p>
      <div className="mt-8 flex gap-2">
        <SecondaryButton onClick={onBack}>Choose another folder</SecondaryButton>
        <PrimaryButton onClick={onRetry}>Retry</PrimaryButton>
      </div>
    </div>
  );
}

function PrimaryButton({
  children,
  onClick,
  disabled,
}: {
  children: ReactNode;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className="rounded px-3 py-1.5 text-sm font-semibold disabled:opacity-50"
      style={{
        background: "var(--accent, #2563eb)",
        color: "#fff",
        border: "none",
        cursor: disabled ? "not-allowed" : "pointer",
      }}
    >
      {children}
    </button>
  );
}

function SecondaryButton({
  children,
  onClick,
}: {
  children: ReactNode;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="rounded px-3 py-1.5 text-sm font-medium"
      style={{
        background: "var(--bg)",
        color: "var(--fg)",
        border: "1px solid var(--border)",
        cursor: "pointer",
      }}
    >
      {children}
    </button>
  );
}
