import { useCallback, useEffect, useRef, useState } from "react";
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
import { isTauri } from "@/shared/api";
import { ProjectList } from "@/features/projects/ProjectList";
import { RunList } from "@/features/run-list/RunList";
import { RunReviewHeader } from "./RunReviewHeader";
import { RunReviewNavigation } from "./RunReviewNavigation";
import { RunOverviewPanel } from "./RunOverviewPanel";
import { CheckpointsReviewPanel } from "./CheckpointsReviewPanel";
import { EvidenceReviewPanel } from "./EvidenceReviewPanel";
import { FindingsReviewPanel } from "./FindingsReviewPanel";
import { HistoryReviewPanel } from "./HistoryReviewPanel";
import { ReviewNotice } from "./ReviewNotice";
import type { ReviewSection } from "./labels";

export interface ReviewWorkspaceProps {
  openPath?: string | null;
  onOpenRunPath?: (path: string) => void;
  focusProjectPath?: string | null;
}

/**
 * Coordinated Projects → Runs → Review workspace for external evaluators.
 */
export function ReviewWorkspace({
  openPath,
  onOpenRunPath,
  focusProjectPath,
}: ReviewWorkspaceProps) {
  const [offline, setOffline] = useState(true);
  const [statusMsg, setStatusMsg] = useState<string | null>(null);
  const [projects, setProjects] = useState<ProjectSummaryDto[]>([]);
  const [selectedProject, setSelectedProject] = useState<ProjectSummaryDto | null>(null);
  const [runs, setRuns] = useState<RunSummaryDto[]>([]);
  const [selectedRun, setSelectedRun] = useState<RunSummaryDto | null>(null);
  const [detail, setDetail] = useState<RunDetailDto | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [category, setCategory] = useState("recent");
  const [query, setQuery] = useState("");
  const [openFindingsOnly, setOpenFindingsOnly] = useState(false);
  const [hasRisks, setHasRisks] = useState(false);
  const [hasQuestions, setHasQuestions] = useState(false);
  const [captureCoverage, setCaptureCoverage] = useState("");
  const [refreshToken, setRefreshToken] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [section, setSection] = useState<ReviewSection>("overview");
  const [selectedCheckpointId, setSelectedCheckpointId] = useState<string | null>(null);
  const [selectedFindingId, setSelectedFindingId] = useState<string | null>(null);
  const [rebuildState, setRebuildState] = useState<"idle" | "running" | "done" | "failed">("idle");
  const [projectHasRuns, setProjectHasRuns] = useState(true);

  const generationRef = useRef(0);
  const fileActionsAvailable = isTauri;

  const bumpRefresh = useCallback(() => {
    setRefreshToken((t) => t + 1);
  }, []);

  const selectRun = useCallback(
    (r: RunSummaryDto | null) => {
      setSelectedRun(r);
      setSelectedCheckpointId(null);
      setSelectedFindingId(null);
      setSection("overview");
      setDetail(null);
      setDetailError(null);
      if (r) onOpenRunPath?.(r.absolutePath);
    },
    [onOpenRunPath],
  );

  const selectProject = useCallback((p: ProjectSummaryDto | null) => {
    setSelectedProject(p);
    setSelectedRun(null);
    setSelectedCheckpointId(null);
    setSelectedFindingId(null);
    setDetail(null);
    setDetailError(null);
    setSection("overview");
    setRuns([]);
  }, []);

  const refreshProjects = useCallback(async () => {
    const gen = ++generationRef.current;
    try {
      const st = await discoveryStatus();
      if (gen !== generationRef.current) return;
      setOffline(!st.online);
      setStatusMsg(st.message ?? null);
      let list = await discoveryProjects(null);
      if (gen !== generationRef.current) return;
      if (focusProjectPath && !list.some((p) => p.rootPath === focusProjectPath)) {
        try {
          const added = await discoveryAddExistingProject(focusProjectPath);
          list = [...list, added];
        } catch {
          /* may need full enable */
        }
      }
      if (gen !== generationRef.current) return;
      setProjects(list);
      setSelectedProject((prev) => {
        if (focusProjectPath) {
          const focus = list.find((p) => p.rootPath === focusProjectPath);
          if (focus) return focus;
        }
        if (prev && list.some((p) => p.projectId === prev.projectId)) return prev;
        return list[0] ?? null;
      });
    } catch (e) {
      if (gen !== generationRef.current) return;
      setError(e instanceof Error ? e.message : String(e));
      setOffline(true);
    }
  }, [focusProjectPath]);

  const refreshRuns = useCallback(async () => {
    if (!selectedProject) {
      setRuns([]);
      setProjectHasRuns(true);
      return;
    }
    const gen = generationRef.current;
    const projectId = selectedProject.projectId;
    try {
      const unfiltered = await discoveryRuns({
        projectId,
        rootPath: selectedProject.rootPath,
        category: "recent",
        openFindingsOnly: false,
        hasRisks: false,
        hasQuestions: false,
        query: null,
        captureCoverage: null,
      });
      if (gen !== generationRef.current || selectedProject.projectId !== projectId) return;
      setProjectHasRuns(unfiltered.length > 0);

      const list = await discoveryRuns({
        projectId,
        rootPath: selectedProject.rootPath,
        category,
        openFindingsOnly,
        hasRisks,
        hasQuestions,
        query: query || null,
        captureCoverage: captureCoverage || null,
      });
      if (gen !== generationRef.current || selectedProject.projectId !== projectId) return;
      setRuns(list);
      setSelectedRun((prev) => {
        if (prev && list.some((r) => r.runId === prev.runId)) {
          return list.find((r) => r.runId === prev.runId) ?? prev;
        }
        if (prev && !list.some((r) => r.runId === prev.runId)) {
          setDetail(null);
          setSelectedCheckpointId(null);
          setSelectedFindingId(null);
          return null;
        }
        return prev;
      });
    } catch (e) {
      if (gen !== generationRef.current) return;
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
  }, [refreshRuns, refreshToken]);

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
          return;
        }
        const revChanged = st.revision !== lastSeen;
        const cameOnline = wasOnline === false && st.online;
        lastSeen = st.revision;
        wasOnline = st.online;
        if (revChanged || cameOnline) {
          bumpRefresh();
        }
      },
      { intervalMs: 3000 },
    );
    return unsub;
  }, [bumpRefresh]);

  useEffect(() => {
    if (!selectedRun) {
      setDetail(null);
      setDetailError(null);
      return;
    }
    const gen = generationRef.current;
    const runId = selectedRun.runId;
    void (async () => {
      try {
        const d = await discoveryRunDetail({
          path: selectedRun.absolutePath,
          runId: selectedRun.runId,
          projectRoot: selectedProject?.rootPath,
        });
        if (gen !== generationRef.current) return;
        if (selectedRun.runId !== runId) return;
        setDetail(d);
        setDetailError(null);
      } catch (e) {
        if (gen !== generationRef.current) return;
        if (selectedRun.runId !== runId) return;
        const msg = e instanceof Error ? e.message : String(e);
        setDetailError(msg);
        setDetail(null);
        setError(msg);
      }
    })();
  }, [selectedRun, selectedProject, refreshToken]);

  useEffect(() => {
    if (!openPath || runs.length === 0) return;
    const match = runs.find((r) => r.absolutePath === openPath);
    if (match && match.runId !== selectedRun?.runId) selectRun(match);
  }, [openPath, runs, selectedRun?.runId, selectRun]);

  async function copyText(text: string) {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      setError("Could not copy to clipboard");
    }
  }

  async function revealRecord() {
    if (!selectedRun || !selectedProject) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("reveal_run_record", {
        projectRoot: selectedProject.rootPath,
        recordPath: selectedRun.recordPath,
        absolutePath: selectedRun.absolutePath,
        runId: selectedRun.runId,
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function openMarkdown() {
    if (!selectedRun || !selectedProject) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("open_run_markdown", {
        projectRoot: selectedProject.rootPath,
        recordPath: selectedRun.recordPath,
        absolutePath: selectedRun.absolutePath,
        runId: selectedRun.runId,
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  const navCounts =
    selectedRun != null
      ? {
          checkpoints: String(selectedRun.checkpointCount),
          evidence: String(selectedRun.evidenceCount),
          findings:
            selectedRun.openFindingCount > 0
              ? `${selectedRun.openFindingCount} open`
              : String(selectedRun.openFindingCount),
        }
      : undefined;

  return (
    <div className="flex min-h-0 flex-1" data-testid="ledger-workspace">
      <ProjectList
        projects={projects}
        selectedId={selectedProject?.projectId ?? null}
        offline={offline}
        rebuildState={rebuildState}
        onSelect={selectProject}
        onRescan={(p) => {
          void discoveryRescanProject(p.projectId).then(() => bumpRefresh());
        }}
        onAdd={() => {
          void (async () => {
            if (!isTauri) {
              setError("Add project requires the Moraine desktop app");
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
              selectProject(p);
            } catch (e) {
              setError(e instanceof Error ? e.message : String(e));
            }
          })();
        }}
        onRebuild={() => {
          setRebuildState("running");
          void discoveryRebuildIndex(null)
            .then(() => {
              setRebuildState("done");
              bumpRefresh();
            })
            .catch((e) => {
              setRebuildState("failed");
              setError(e instanceof Error ? e.message : String(e));
            });
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
        projectHasRuns={projectHasRuns}
        hasProject={!!selectedProject}
        onCategory={setCategory}
        onQuery={setQuery}
        onOpenFindingsOnly={setOpenFindingsOnly}
        onHasRisks={setHasRisks}
        onHasQuestions={setHasQuestions}
        onCaptureCoverage={setCaptureCoverage}
        onResetFilters={() => {
          setCategory("recent");
          setQuery("");
          setOpenFindingsOnly(false);
          setHasRisks(false);
          setHasQuestions(false);
          setCaptureCoverage("");
        }}
        onSelect={selectRun}
      />
      <section className="flex min-w-0 flex-1 flex-col overflow-auto text-xs">
        {error ? (
          <div className="border-b px-3 py-2" style={{ color: "#b45309" }} data-testid="workspace-error">
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
        {offline ? (
          <div className="border-b px-3 py-1" style={{ color: "#b45309" }} data-testid="offline-banner">
            Discovery service offline — showing direct project scan when available. Capture can
            continue locally.
          </div>
        ) : null}
        {!selectedRun ? (
          <div className="p-4" style={{ color: "var(--muted)" }} data-testid="review-empty">
            {!selectedProject
              ? "Add or select a project to begin review. Moraine looks for project-local records and does not upload the directory."
              : "Select a run to inspect objective, agent, capture fidelity, checkpoints, evidence, findings, and history."}
          </div>
        ) : (
          <>
            <RunReviewHeader
              run={selectedRun}
              detail={detail}
              detailError={detailError}
              onCopyRunId={() => void copyText(selectedRun.runId)}
              onCopyPath={() => void copyText(selectedRun.recordPath)}
              onRevealRecord={() => void revealRecord()}
              onOpenMarkdown={() => void openMarkdown()}
              fileActionsAvailable={fileActionsAvailable}
            />
            <RunReviewNavigation
              section={section}
              onSection={setSection}
              counts={navCounts}
            />
            {section === "overview" ? (
              <RunOverviewPanel
                run={selectedRun}
                detail={detail}
                detailError={detailError}
                onGoFindings={() => setSection("findings")}
                onGoCheckpoints={() => setSection("checkpoints")}
                onGoHistory={() => setSection("history")}
              />
            ) : null}
            {section === "checkpoints" ? (
              <CheckpointsReviewPanel
                path={selectedRun.absolutePath}
                refreshToken={refreshToken}
                selectedCheckpointId={selectedCheckpointId}
                onSelectCheckpoint={setSelectedCheckpointId}
                onMutated={bumpRefresh}
              />
            ) : null}
            {section === "evidence" ? (
              <EvidenceReviewPanel
                timeline={detail?.timeline ?? []}
                loading={!detail && !detailError}
                error={detailError}
              />
            ) : null}
            {section === "findings" ? (
              <FindingsReviewPanel
                path={selectedRun.absolutePath}
                refreshToken={refreshToken}
                selectedFindingId={selectedFindingId}
                onSelectFinding={setSelectedFindingId}
                onMutated={bumpRefresh}
              />
            ) : null}
            {section === "history" ? (
              <HistoryReviewPanel
                timeline={detail?.timeline ?? []}
                loading={!detail && !detailError}
                error={detailError}
              />
            ) : null}
            {selectedRun.integrity !== "current" && section === "overview" ? (
              <div className="px-3 pb-3">
                <ReviewNotice
                  tone="warn"
                  title="Record integrity needs attention"
                  body="This run’s durable sidecar is not marked current. Review what Moraine could load below; do not treat missing panels as deleted data."
                />
              </div>
            ) : null}
          </>
        )}
      </section>
    </div>
  );
}
