import type { DesktopProductSupport } from "@/shared/platformSupport";

export function UnsupportedPlatformView({
  support,
}: {
  support: DesktopProductSupport;
}) {
  if (support.host === "windows") {
    return (
      <main
        className="flex h-screen items-center justify-center p-8"
        data-testid="unsupported-platform"
        style={{ background: "var(--bg)", color: "var(--fg)" }}
      >
        <section className="max-w-xl">
          <h1 className="text-xl font-semibold">Onboarding stays closed.</h1>
          <p className="mt-3 text-sm" style={{ color: "var(--muted)" }}>
            Reported Windows capabilities do not include a usable capture
            runtime and desktop host together.
          </p>
          <p className="mt-2 text-sm" style={{ color: "var(--muted)" }}>
            Stage the demo suite; no installer is available. A supported
            Windows installer does not exist. Windows Product Ready remains No.
          </p>
        </section>
      </main>
    );
  }

  return (
    <main
      className="flex h-screen items-center justify-center p-8"
      data-testid="unsupported-platform"
      style={{ background: "var(--bg)", color: "var(--fg)" }}
    >
      <section className="max-w-xl">
        <h1 className="text-xl font-semibold">
          Moraine background capture is not available on {support.host} yet.
        </h1>
        <p className="mt-3 text-sm" style={{ color: "var(--muted)" }}>
          This build can identify the host and validate Moraine&apos;s portable
          components, but it will not install or start a background runtime or
          accept agent events.
        </p>
      </section>
    </main>
  );
}
