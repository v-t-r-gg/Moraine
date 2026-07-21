import { lazy, Suspense, useCallback, useState } from "react";
import { Workspace } from "@/app/Workspace";
import { ServiceHealthBanner } from "@/app/ServiceHealthBanner";
import { useProductBootstrap } from "@/app/useProductBootstrap";
import { SURFACE_LEGACY_DOCUMENT } from "@/app/surfaceFreeze";
import { StatusBar } from "@/features/shell/StatusBar";
import { pickMarkdownFile, isTauri } from "@/shared/api";

/** Lazy: keeps Yjs/editor out of the main ledger coordinator bundle path. */
const LegacyDocumentApp = lazy(async () => {
  const m = await import("@/app/LegacyDocumentApp");
  return { default: m.LegacyDocumentApp };
});

/**
 * C3 installed-product shell:
 *   bootstrap → service health → ledger workspace
 *   optional legacy document route (collab frozen; secondary only)
 */
export function App() {
  const bootstrap = useProductBootstrap();
  const [route, setRoute] = useState<"ledger" | "legacy">("ledger");
  const [legacyPath, setLegacyPath] = useState<string | null>(null);

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

      <div className="min-h-0 flex-1">
        <Workspace />
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
          bootstrap.ready
            ? bootstrap.service?.online
              ? "Ledger workspace · capture may continue with desktop closed"
              : "Ledger workspace · service offline (direct discovery)"
            : "Starting…"
        }
      />
    </div>
  );
}
