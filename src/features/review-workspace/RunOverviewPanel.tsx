import type { RunDetailDto, RunSummaryDto } from "@/shared/api/discovery";
import { CaptureFidelityPanel } from "@/features/runs/CaptureFidelityPanel";
import { ReviewNotice } from "./ReviewNotice";
import { coverageLabel, formatWhen, observationLabel } from "./labels";

export interface RunOverviewPanelProps {
  run: RunSummaryDto;
  detail: RunDetailDto | null;
  detailError?: string | null;
  onGoFindings: () => void;
  onGoCheckpoints: () => void;
  onGoHistory: () => void;
}

export function RunOverviewPanel({
  run,
  detail,
  detailError,
  onGoFindings,
  onGoCheckpoints,
  onGoHistory,
}: RunOverviewPanelProps) {
  if (detailError) {
    return (
      <div className="p-3" data-testid="review-overview">
        <ReviewNotice
          tone="error"
          title="Could not load full run detail"
          body={`${detailError}. The selected project and run remain selected. Durable project records stay on disk; try Rescan or open the Markdown record.`}
          testId="overview-detail-error"
        />
      </div>
    );
  }

  if (!detail) {
    return (
      <div className="p-3" style={{ color: "var(--muted)" }} data-testid="review-overview">
        Loading run overview…
      </div>
    );
  }

  if (!detail.isProtocolRun) {
    return (
      <div className="p-3" data-testid="review-overview">
        <ReviewNotice
          tone="warn"
          title="Legacy or non-protocol record"
          body="This file is not a protocol agent run. Open it in Legacy document mode for free-form editing. Capture fidelity and findings are unavailable."
        />
        {run.error ? <p className="mt-2 text-xs">{run.error}</p> : null}
      </div>
    );
  }

  const recentTimeline = detail.timeline.slice(0, 6);

  return (
    <div className="grid gap-3 p-3 text-xs" data-testid="review-overview">
      <section>
        <h2 className="font-medium">Objective</h2>
        <p style={{ color: "var(--muted)" }}>
          {detail.objective || run.objective || "(no objective recorded)"}
        </p>
        <p className="mt-1" style={{ color: "var(--muted)" }}>
          Capture status: {coverageLabel(detail.captureFidelity?.legacyCoverage ?? run.captureCoverage)}
        </p>
      </section>

      {detail.captureFidelityError ? (
        <ReviewNotice
          tone="error"
          title="Capture fidelity unavailable"
          body={`Moraine could not derive a fidelity report (${detail.captureFidelityError}). The run ledger may still be reviewable; session envelope state needs attention.`}
          testId="overview-fidelity-error"
        />
      ) : null}

      {detail.captureFidelity ? (
        <CaptureFidelityPanel report={detail.captureFidelity} />
      ) : !detail.captureFidelityError ? (
        <ReviewNotice
          tone="info"
          title="No fidelity report"
          body="No capture fidelity report is attached to this run detail."
        />
      ) : null}

      {detail.captureFidelity?.gaps && detail.captureFidelity.gaps.length > 0 ? (
        <section data-testid="overview-gaps">
          <h2 className="font-medium">Observation gaps</h2>
          <ul className="mt-1 list-disc pl-4" style={{ color: "var(--muted)" }}>
            {detail.captureFidelity.gaps.map((g) => (
              <li key={g.dimension + g.reason}>
                <strong>{g.dimension.replace(/_/g, " ")}</strong>: {g.reason}
              </li>
            ))}
          </ul>
          <p className="mt-1 text-[10px]" style={{ color: "var(--muted)" }}>
            Gaps distinguish not observed, not supported by this adapter, and unknown (historical or
            unavailable) states — they are not a score.
          </p>
        </section>
      ) : null}

      {detail.captureFidelity?.dimensions ? (
        <section className="sr-only" aria-hidden={false}>
          {detail.captureFidelity.dimensions.map((d) => (
            <span key={d.dimension}>
              {d.dimension}:{observationLabel(d.observation)}
            </span>
          ))}
        </section>
      ) : null}

      {detail.risks.length > 0 ? (
        <section data-testid="overview-risks">
          <h2 className="font-medium">Risks</h2>
          <ul className="list-disc pl-4">
            {detail.risks.map((r) => (
              <li key={r}>{r}</li>
            ))}
          </ul>
        </section>
      ) : null}

      {detail.openQuestions.length > 0 ? (
        <section data-testid="overview-questions">
          <h2 className="font-medium">Open questions</h2>
          <ul className="list-disc pl-4">
            {detail.openQuestions.map((q) => (
              <li key={q}>{q}</li>
            ))}
          </ul>
        </section>
      ) : null}

      <section>
        <h2 className="font-medium">
          Recent activity{" "}
          <button type="button" className="underline font-normal" onClick={onGoHistory}>
            full history
          </button>
        </h2>
        <ul className="mt-1 grid gap-1">
          {recentTimeline.map((e) => (
            <li key={e.id} style={{ color: "var(--muted)" }}>
              {formatWhen(e.timestamp)} · {e.kind} · {e.summary}
            </li>
          ))}
          {recentTimeline.length === 0 ? (
            <li style={{ color: "var(--muted)" }}>No timeline entries yet.</li>
          ) : null}
        </ul>
      </section>

      <div className="flex flex-wrap gap-3">
        <button type="button" className="underline" onClick={onGoCheckpoints}>
          Checkpoints ({run.checkpointCount})
        </button>
        <button type="button" className="underline" onClick={onGoFindings}>
          Findings ({run.openFindingCount} open)
        </button>
      </div>
    </div>
  );
}
