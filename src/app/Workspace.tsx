import { useCallback, useEffect, useState } from "react";
import {
  discoveryAddExistingProject,
  discoveryProjects,
  discoveryRebuildIndex,
  discoveryRescanProject,
  discoveryRunDetail,
  discoveryRuns,
  discoveryStatus,
  subscribeDiscoveryRevision,
  type ProjectSummaryDto,
  type RunDetailDto,
  type RunSummaryDto,
} from "@/shared/api/discovery";
import { ProjectList } from "@/features/projects/ProjectList";
import { RunList } from "@/features/run-list/RunList";
import { LedgerTimeline } from "@/features/ledger/LedgerTimeline";
import { ProtocolLedgerPanel } from "@/features/ledger/ProtocolLedgerPanel";
import { CheckpointFindingsPanel } from "@/features/findings/CheckpointFindingsPanel";
import { isTauri } from "@/shared/api";

export interface WorkspaceProps {
  /** When set, select this run path in the detail pane. */
  openPath?: string | null;
  onOpenRunPath?: (path: string) => void;
}

/**
 * Projects → Runs → Ledger workspace (default desktop without welcome-md).
 */
export function Workspace({ openPath, onOpenRunPath }: WorkspaceProps) {
  const [offline, setOffline] = useState(true);
  const [statusMsg, setStatusMsg] = useState<string | null>(null);
  const [projects, setProjects] = useState<ProjectSummaryDto[]>([]);
  const [selectedProject, setSelectedProject] = useState<ProjectSummaryDto | null>(null);
  const [runs, setRuns] = useState<RunSummaryDto[]>([]);
  const [selectedRun, setSelectedRun] = useState<RunSummaryDto | null>(null);
  const [detail, setDetail] = useState<RunDetailDto | null>(null);
  const [category, setCategory] = useState("recent");
  const [query, setQuery] = useState("");
  const [openFindingsOnly, setOpenFindingsOnly] = useState(false);
  const [hasRisks, setHasRisks] = useState(false);
  const [hasQuestions, setHasQuestions] = useState(false);
  const [captureCoverage, setCaptureCoverage] = useState("");
  const [refreshToken, setRefreshToken] = useState(0);
  const [error, setError] = useState<string | null>(null);

  const refreshProjects = useCallback(async () => {
    try {
      const st = await discoveryStatus();
      setOffline(!st.online);
      setStatusMsg(st.message ?? null);
      const list = await discoveryProjects(null);
      setProjects(list);
      setSelectedProject((prev) => {
        if (prev && list.some((p) => p.projectId === prev.projectId)) return prev;
        return list[0] ?? null;
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setOffline(true);
    }
  }, []);

  const refreshRuns = useCallback(async () => {
    if (!selectedProject) {
      setRuns([]);
      return;
    }
    try {
      const list = await discoveryRuns({
        projectId: selectedProject.projectId,
        rootPath: selectedProject.rootPath,
        category,
        openFindingsOnly,
        hasRisks,
        hasQuestions,
        query: query || null,
        captureCoverage: captureCoverage || null,
      });
      setRuns(list);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setRuns([]);
    }
  }, [
    selectedProject,
    category,
    openFindingsOnly,
    hasRisks,
    hasQuestions,
    query,
    captureCoverage,
  ]);

  useEffect(() => {
    void refreshProjects();
  }, [refreshProjects, refreshToken]);

  useEffect(() => {
    void refreshRuns();
  }, [refreshRuns]);

  // Bounded index-revision polling (not per-run FS watchers). Cleanup on unmount / Strict Mode.
  useEffect(() => {
    let lastSeen = -1;
    let wasOnline: boolean | null = null;
    const unsub = subscribeDiscoveryRevision(
      (st) => {
        setOffline(!st.online);
        setStatusMsg(st.message ?? null);
        if (wasOnline === null) {
          wasOnline = st.online;
          lastSeen = st.revision;
          return; // mount load already fetches projects/runs
        }
        const revChanged = st.revision !== lastSeen;
        const cameOnline = wasOnline === false && st.online;
        lastSeen = st.revision;
        wasOnline = st.online;
        if (revChanged || cameOnline) {
          setRefreshToken((t) => t + 1);
        }
      },
      { intervalMs: 3000 },
    );
    return unsub;
  }, []);

  useEffect(() => {
    if (!selectedRun) {
      setDetail(null);
      return;
    }
    void (async () => {
      try {
        const d = await discoveryRunDetail({
          path: selectedRun.absolutePath,
          runId: selectedRun.runId,
          projectRoot: selectedProject?.rootPath,
        });
        setDetail(d);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
        setDetail(null);
      }
    })();
  }, [selectedRun, selectedProject, refreshToken]);

  // Open path from host: select matching run if present
  useEffect(() => {
    if (!openPath || runs.length === 0) return;
    const match = runs.find((r) => r.absolutePath === openPath);
    if (match) setSelectedRun(match);
  }, [openPath, runs]);

  return (
    <div className="flex min-h-0 flex-1" data-testid="ledger-workspace">
      <ProjectList
        projects={projects}
        selectedId={selectedProject?.projectId ?? null}
        offline={offline}
        onSelect={setSelectedProject}
        onRescan={(p) => {
          void discoveryRescanProject(p.projectId).then(() => setRefreshToken((t) => t + 1));
        }}
        onAdd={() => {
          void (async () => {
            if (!isTauri) {
              setError("Add project requires the Tauri desktop host");
              return;
            }
            const { open } = await import("@tauri-apps/plugin-dialog");
            const selected = await open({ directory: true, multiple: false });
            if (!selected || typeof selected !== "string") return;
            try {
              const p = await discoveryAddExistingProject(selected);
              setProjects((prev) => {
                if (prev.some((x) => x.projectId === p.projectId)) return prev;
                return [...prev, p];
              });
              setSelectedProject(p);
            } catch (e) {
              setError(e instanceof Error ? e.message : String(e));
            }
          })();
        }}
        onRebuild={() => {
          void discoveryRebuildIndex(null).then(() => setRefreshToken((t) => t + 1));
        }}
      />
      <RunList
        runs={runs}
        selectedId={selectedRun?.runId ?? null}
        category={category}
        query={query}
        openFindingsOnly={openFindingsOnly}
        hasRisks={hasRisks}
        hasQuestions={hasQuestions}
        captureCoverage={captureCoverage}
        onCategory={setCategory}
        onQuery={setQuery}
        onOpenFindingsOnly={setOpenFindingsOnly}
        onHasRisks={setHasRisks}
        onHasQuestions={setHasQuestions}
        onCaptureCoverage={setCaptureCoverage}
        onSelect={(r) => {
          setSelectedRun(r);
          onOpenRunPath?.(r.absolutePath);
        }}
      />
      <section className="flex min-w-0 flex-1 flex-col overflow-auto text-xs">
        {error ? (
          <div className="border-b px-3 py-2" style={{ color: "#b45309" }}>
            {error}{" "}
            <button type="button" className="underline" onClick={() => setError(null)}>
              dismiss
            </button>
          </div>
        ) : null}
        {statusMsg ? (
          <div className="border-b px-3 py-1" style={{ color: "var(--muted)" }}>
            {statusMsg}
          </div>
        ) : null}
        {!selectedRun ? (
          <div className="p-4" style={{ color: "var(--muted)" }}>
            Select a project and run to inspect the structured ledger. Capture continues while this
            desktop is closed.
          </div>
        ) : (
          <>
            <header className="border-b px-3 py-2" style={{ borderColor: "var(--border)" }}>
              <div className="font-semibold text-sm" style={{ color: "var(--text)" }}>
                {selectedRun.objective || selectedRun.runId}
              </div>
              <div style={{ color: "var(--muted)" }}>
                {selectedRun.lifecycle} · {selectedRun.captureCoverage}
                {selectedRun.provisional ? " · provisional" : ""} · integrity{" "}
                {selectedRun.integrity}
              </div>
              {selectedRun.recoveryRequired ? (
                <div style={{ color: "#b45309" }}>Recovery required for incomplete agent op</div>
              ) : null}
            </header>
            {detail?.isProtocolRun ? (
              <>
                <ProtocolLedgerPanel
                  path={selectedRun.absolutePath}
                  refreshToken={refreshToken}
                  onMutated={() => setRefreshToken((t) => t + 1)}
                />
                <CheckpointFindingsPanel
                  path={selectedRun.absolutePath}
                  refreshToken={refreshToken}
                />
                <div className="border-t px-3 py-2" style={{ borderColor: "var(--border)" }}>
                  <div className="mb-1 font-medium">Timeline</div>
                  <LedgerTimeline entries={detail.timeline} />
                </div>
                {(detail.risks.length > 0 || detail.openQuestions.length > 0) && (
                  <div className="border-t px-3 py-2 grid gap-1" style={{ borderColor: "var(--border)" }}>
                    {detail.risks.length > 0 ? (
                      <div>
                        <div className="font-medium">Risks</div>
                        <ul className="list-disc pl-4">
                          {detail.risks.map((r) => (
                            <li key={r}>{r}</li>
                          ))}
                        </ul>
                      </div>
                    ) : null}
                    {detail.openQuestions.length > 0 ? (
                      <div>
                        <div className="font-medium">Open questions</div>
                        <ul className="list-disc pl-4">
                          {detail.openQuestions.map((q) => (
                            <li key={q}>{q}</li>
                          ))}
                        </ul>
                      </div>
                    ) : null}
                  </div>
                )}
              </>
            ) : (
              <div className="p-3" style={{ color: "#b45309" }}>
                Legacy or non-protocol record. Open via file path for Legacy document mode editing.
                {selectedRun.error ? ` ${selectedRun.error}` : ""}
              </div>
            )}
          </>
        )}
      </section>
    </div>
  );
}
