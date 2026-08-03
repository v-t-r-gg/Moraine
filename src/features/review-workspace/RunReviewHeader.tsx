import type { RunDetailDto, RunSummaryDto } from "@/shared/api/discovery";
import {
  agentLabel,
  coverageLabel,
  formatWhen,
  integrityLabel,
  lifecycleLabel,
} from "./labels";

export interface RunReviewHeaderProps {
  run: RunSummaryDto;
  detail: RunDetailDto | null;
  detailError?: string | null;
  onCopyRunId: () => void;
  onCopyPath: () => void;
  onRevealRecord: () => void;
  onOpenMarkdown: () => void;
  fileActionsAvailable: boolean;
}

export function RunReviewHeader({
  run,
  detail,
  detailError,
  onCopyRunId,
  onCopyPath,
  onRevealRecord,
  onOpenMarkdown,
  fileActionsAvailable,
}: RunReviewHeaderProps) {
  const objective = detail?.objective || run.objective || "(no objective recorded)";
  const integration = detail?.captureFidelity?.integration;
  const fidelity =
    detail?.captureFidelity != null
      ? coverageLabel(detail.captureFidelity.legacyCoverage)
      : coverageLabel(run.captureCoverage);

  return (
    <header
      className="border-b px-3 py-2 text-xs"
      style={{ borderColor: "var(--border)" }}
      data-testid="run-review-header"
    >
      <div className="text-sm font-semibold" style={{ color: "var(--text)" }}>
        {objective}
      </div>
      <div className="mt-1 flex flex-wrap gap-x-3 gap-y-1" style={{ color: "var(--muted)" }}>
        <span data-testid="header-lifecycle">{lifecycleLabel(run.lifecycle)}</span>
        {run.provisional ? (
          <span data-testid="header-provisional" className="font-medium" style={{ color: "#b45309" }}>
            Provisional
          </span>
        ) : null}
        <span data-testid="header-agent">{agentLabel(integration)}</span>
        <span data-testid="header-fidelity">{fidelity}</span>
        <span data-testid="header-integrity">{integrityLabel(run.integrity)}</span>
      </div>
      <div className="mt-1 flex flex-wrap gap-x-3 gap-y-1" style={{ color: "var(--muted)" }}>
        <span>Started {formatWhen(run.startedAt)}</span>
        <span>Updated {formatWhen(run.updatedAt)}</span>
        <span>
          {run.openFindingCount} open finding{run.openFindingCount === 1 ? "" : "s"}
        </span>
        <span>
          {run.riskCount} risk{run.riskCount === 1 ? "" : "s"}
        </span>
        <span>
          {run.openQuestionCount} open question{run.openQuestionCount === 1 ? "" : "s"}
        </span>
      </div>
      {run.recoveryRequired ? (
        <div className="mt-1 font-medium" style={{ color: "#b45309" }} data-testid="header-recovery">
          Recovery required for an incomplete agent operation. Durable files remain on disk.
        </div>
      ) : null}
      {detailError ? (
        <div className="mt-1" style={{ color: "#b45309" }} data-testid="header-detail-error">
          {detailError}
        </div>
      ) : null}
      <div className="mt-2 flex flex-wrap gap-2">
        <button type="button" className="underline" onClick={onCopyRunId} data-testid="copy-run-id">
          Copy run ID
        </button>
        <button type="button" className="underline" onClick={onCopyPath} data-testid="copy-record-path">
          Copy record path
        </button>
        {fileActionsAvailable ? (
          <>
            <button
              type="button"
              className="underline"
              onClick={onRevealRecord}
              data-testid="reveal-record"
            >
              Reveal in file manager
            </button>
            <button
              type="button"
              className="underline"
              onClick={onOpenMarkdown}
              data-testid="open-markdown"
            >
              Open Markdown record
            </button>
          </>
        ) : null}
      </div>
      <div className="mt-1 truncate" style={{ color: "var(--muted)" }} title={run.recordPath}>
        {run.recordPath}
      </div>
      <div className="truncate font-mono text-[10px]" style={{ color: "var(--muted)" }}>
        {run.runId}
      </div>
    </header>
  );
}
