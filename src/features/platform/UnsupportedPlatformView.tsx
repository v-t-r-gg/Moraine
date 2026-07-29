import type { DesktopProductSupport } from "@/shared/platformSupport";

export function UnsupportedPlatformView({
  support,
}: {
  support: DesktopProductSupport;
}) {
  const host = support.host === "windows" ? "Windows" : support.host;
  return (
    <main
      className="flex h-screen items-center justify-center p-8"
      data-testid="unsupported-platform"
      style={{ background: "var(--bg)", color: "var(--fg)" }}
    >
      <section className="max-w-xl">
        <h1 className="text-xl font-semibold">
          Moraine background capture is not available on {host} yet.
        </h1>
        <p className="mt-3 text-sm" style={{ color: "var(--muted)" }}>
          This build can identify the host and validate Moraine&apos;s portable
          components, but it will not install or start a background runtime or
          accept agent events.
        </p>
        <p className="mt-2 text-sm" style={{ color: "var(--muted)" }}>
          Native Windows support belongs to W2.
        </p>
      </section>
    </main>
  );
}
