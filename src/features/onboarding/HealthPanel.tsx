/** In-app health checks with Fix actions (doctor → product language). */

import { useCallback, useEffect, useState } from "react";
import {
  provisionHealth,
  provisionRepair,
  type HealthCheckDto,
  type HealthReportDto,
} from "@/shared/api/provision";

export interface HealthPanelProps {
  projectPath?: string | null;
  onRepaired?: () => void;
}

export function HealthPanel({ projectPath, onRepaired }: HealthPanelProps) {
  const [report, setReport] = useState<HealthReportDto | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const r = await provisionHealth(projectPath ?? null, "codex");
      setReport(r);
    } catch (e) {
      setMessage(e instanceof Error ? e.message : String(e));
    }
  }, [projectPath]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const fix = async (check: HealthCheckDto) => {
    if (!check.repair) return;
    setBusyId(check.id);
    setMessage(null);
    try {
      const result = await provisionRepair(check.repair);
      setMessage(result.userMessage);
      await refresh();
      if (result.ok) onRepaired?.();
    } catch (e) {
      setMessage(e instanceof Error ? e.message : String(e));
    } finally {
      setBusyId(null);
    }
  };

  if (!report) {
    return (
      <div className="text-[11px]" style={{ color: "var(--muted)" }} data-testid="health-panel">
        Checking health…
      </div>
    );
  }

  return (
    <div className="text-xs" data-testid="health-panel">
      <div className="mb-2 flex items-center justify-between">
        <span className="font-semibold">Health</span>
        <button
          type="button"
          className="text-[10px] font-semibold"
          style={{ color: "var(--accent)", border: "none", background: "none", cursor: "pointer" }}
          onClick={() => void refresh()}
        >
          Refresh
        </button>
      </div>
      <ul className="grid gap-1.5">
        {report.checks.map((c) => (
          <li
            key={c.id}
            className="flex items-start justify-between gap-2 rounded border px-2 py-1.5"
            style={{ borderColor: "var(--border)", background: "var(--bg)" }}
          >
            <div className="min-w-0">
              <div
                style={{
                  color:
                    c.status === "fail"
                      ? "#b91c1c"
                      : c.status === "warn"
                        ? "#b45309"
                        : c.status === "pass"
                          ? "#16a34a"
                          : "var(--muted)",
                }}
              >
                {c.userMessage}
              </div>
              <details className="mt-0.5 text-[10px]" style={{ color: "var(--muted)" }}>
                <summary>Details</summary>
                <code className="block break-all">{c.technicalDetail}</code>
              </details>
            </div>
            {c.repair ? (
              <button
                type="button"
                disabled={busyId === c.id}
                onClick={() => void fix(c)}
                className="shrink-0 rounded px-2 py-0.5 text-[10px] font-semibold"
                style={{
                  background: "var(--accent, #2563eb)",
                  color: "#fff",
                  border: "none",
                  cursor: "pointer",
                  opacity: busyId === c.id ? 0.6 : 1,
                }}
              >
                {busyId === c.id ? "…" : c.repair.label}
              </button>
            ) : null}
          </li>
        ))}
      </ul>
      {message ? (
        <p className="mt-2 text-[10px]" style={{ color: "var(--muted)" }}>
          {message}
        </p>
      ) : null}
    </div>
  );
}
