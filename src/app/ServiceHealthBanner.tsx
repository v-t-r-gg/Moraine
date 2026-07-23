import type { DiscoveryStatusDto } from "@/shared/api/discovery";

export function ServiceHealthBanner({
  service,
  doctorHint,
  productLine,
}: {
  service: DiscoveryStatusDto | null;
  doctorHint: string;
  productLine: string;
}) {
  const offline = !service?.online;
  return (
    <div
      className="flex shrink-0 flex-wrap items-center gap-3 border-b px-3 py-1.5 text-[11px]"
      style={{
        borderColor: "var(--border)",
        background: offline ? "color-mix(in srgb, #b45309 12%, var(--panel))" : "var(--panel)",
        color: "var(--muted)",
      }}
      data-testid="service-health-banner"
    >
      <span style={{ color: "var(--fg)", fontWeight: 600 }}>{productLine}</span>
      <span>
        Discovery:{" "}
        {service?.online ? (
          <span style={{ color: "#16a34a" }}>service · rev {service.revision}</span>
        ) : (
          <span style={{ color: "#b45309" }}>
            offline / direct{service?.message ? ` — ${service.message}` : ""}
          </span>
        )}
      </span>
      <span title="Advanced diagnostics (terminal)">
        Advanced: <code style={{ fontSize: "10px" }}>{doctorHint}</code>
      </span>
    </div>
  );
}
