import { lazy, Suspense, useCallback, useEffect, useState } from "react";
import { Workspace } from "@/app/Workspace";
import { ServiceHealthBanner } from "@/app/ServiceHealthBanner";
import { useProductBootstrap } from "@/app/useProductBootstrap";
import { SURFACE_LEGACY_DOCUMENT } from "@/app/surfaceFreeze";
import { StatusBar } from "@/features/shell/StatusBar";
import { OnboardingWizard } from "@/features/onboarding/OnboardingWizard";
import { HealthPanel } from "@/features/onboarding/HealthPanel";
import { pickMarkdownFile, isTauri } from "@/shared/api";
import { provisionInspect } from "@/shared/api/provision";

/** Lazy: keeps Yjs/editor out of the main ledger coordinator bundle path. */
const LegacyDocumentApp = lazy(async () => {
  const m = await import("@/app/LegacyDocumentApp");
  return { default: m.LegacyDocumentApp };
});

const ONBOARDING_DONE_KEY = "moraine.onboarding.completed";

/**
 * C3 installed-product shell:
 *   bootstrap → first-run wizard (if needed) → service health → ledger workspace
 *   optional legacy document route (collab frozen; secondary only)
 */
export function App() {
  const bootstrap = useProductBootstrap();
  const [route, setRoute] = useState<"ledger" | "legacy" | "onboarding">("ledger");
  const [legacyPath, setLegacyPath] = useState<string | null>(null);
  const [showHealth, setShowHealth] = useState(false);
  const [enabledProject, setEnabledProject] = useState<string | null>(null);
  const [onboardingChecked, setOnboardingChecked] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const done =
          typeof localStorage !== "undefined" &&
          localStorage.getItem(ONBOARDING_DONE_KEY) === "1";
        if (done) {
          if (!cancelled) setOnboardingChecked(true);
          return;
        }
        const st = await provisionInspect();
        if (cancelled) return;
        // First-run when no projects and capture not running.
        const needs =
          st.projects.length === 0 &&
          st.readiness !== "ready" &&
          !st.service.running;
        if (needs) {
          setRoute("onboarding");
        }
      } catch {
        // stay on ledger
      } finally {
        if (!cancelled) setOnboardingChecked(true);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const openLegacyDocument = useCallback(async () => {
    if (!SURFACE_LEGACY_DOCUMENT) return;
    if (!isTauri) {
      setLegacyPath(null);
      setRoute("legacy");
      return;
    }
    const path = await pickMarkdownFile();
    if (path) {
      setLegacyPath(path);
      setRoute("legacy");
    }
  }, []);

  if (route === "onboarding") {
    return (
      <div className="h-screen" data-testid="product-shell">
        <OnboardingWizard
          onComplete={(projectPath) => {
            try {
              localStorage.setItem(ONBOARDING_DONE_KEY, "1");
            } catch {
              /* ignore */
            }
            setEnabledProject(projectPath);
            setRoute("ledger");
          }}
          onDismiss={() => {
            try {
              localStorage.setItem(ONBOARDING_DONE_KEY, "1");
            } catch {
              /* ignore */
            }
            setRoute("ledger");
          }}
        />
      </div>
    );
  }

  if (route === "legacy" && SURFACE_LEGACY_DOCUMENT) {
    return (
      <Suspense
        fallback={
          <div className="p-6 text-sm" style={{ color: "var(--muted)" }}>
            Loading legacy document route…
          </div>
        }
      >
        <LegacyDocumentApp
          initialPath={legacyPath}
          productStatus={bootstrap.productLine}
          onBackToWorkspace={() => {
            setRoute("ledger");
            setLegacyPath(null);
          }}
        />
      </Suspense>
    );
  }

  return (
    <div className="flex h-screen flex-col" data-testid="product-shell">
      <header
        className="flex h-11 shrink-0 items-center justify-between border-b px-3"
        style={{ borderColor: "var(--border)", background: "var(--panel)" }}
      >
        <div className="flex items-center gap-3">
          <span className="text-sm font-semibold" style={{ color: "var(--fg)" }}>
            Moraine
          </span>
          <span className="text-[11px]" style={{ color: "var(--muted)" }}>
            Ledger workspace
          </span>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            className="rounded px-2 py-1 text-[11px] font-medium"
            style={{
              border: "1px solid var(--border)",
              background: "var(--bg)",
              color: "var(--fg)",
            }}
            onClick={() => setRoute("onboarding")}
            data-testid="enable-moraine-btn"
          >
            Enable Moraine…
          </button>
          <button
            type="button"
            className="rounded px-2 py-1 text-[11px] font-medium"
            style={{
              border: "1px solid var(--border)",
              background: showHealth ? "var(--accent-soft)" : "var(--bg)",
              color: "var(--muted)",
            }}
            onClick={() => setShowHealth((v) => !v)}
            data-testid="health-toggle"
          >
            Health
          </button>
          {SURFACE_LEGACY_DOCUMENT ? (
            <button
              type="button"
              className="rounded px-2 py-1 text-[11px] font-medium"
              style={{
                border: "1px solid var(--border)",
                background: "var(--bg)",
                color: "var(--muted)",
              }}
              onClick={() => void openLegacyDocument()}
              title="Historical/compatibility free-form Markdown editor"
            >
              Open legacy document…
            </button>
          ) : null}
        </div>
      </header>

      <ServiceHealthBanner
        service={bootstrap.service}
        doctorHint={bootstrap.doctorHint}
        productLine={bootstrap.productLine}
      />

      {bootstrap.error ? (
        <div className="px-3 py-1 text-[11px]" style={{ color: "#b91c1c" }}>
          Bootstrap: {bootstrap.error}
        </div>
      ) : null}

      <div className="flex min-h-0 flex-1">
        <div className="min-w-0 flex-1">
          <Workspace focusProjectPath={enabledProject} />
        </div>
        {showHealth ? (
          <aside
            className="w-72 shrink-0 overflow-auto border-l p-3"
            style={{ borderColor: "var(--border)", background: "var(--panel)" }}
          >
            <HealthPanel projectPath={enabledProject} />
          </aside>
        ) : null}
      </div>

      <StatusBar
        wordCount={0}
        charCount={0}
        collabPeers={0}
        peerNames=""
        roomId={null}
        autosavePaused={false}
        pendingComments={0}
        pendingSuggestions={0}
        orphanedMarks={0}
        message={
          !onboardingChecked
            ? "Starting…"
            : bootstrap.ready
              ? bootstrap.service?.online
                ? "Ledger workspace · capture may continue with desktop closed"
                : "Ledger workspace · service offline (direct discovery)"
              : "Starting…"
        }
      />
    </div>
  );
}
