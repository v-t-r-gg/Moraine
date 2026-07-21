import { useEffect, useState } from "react";
import { appInfo, isTauriRuntime } from "@/shared/api";
import {
  discoveryStatus,
  type DiscoveryStatusDto,
} from "@/shared/api/discovery";

export interface ProductBootstrap {
  ready: boolean;
  productLine: string;
  service: DiscoveryStatusDto | null;
  doctorHint: string;
  error: string | null;
}

/**
 * Installed-product bootstrap: version/identity + service discovery health.
 * Does not create project state or start Yjs.
 */
export function useProductBootstrap(): ProductBootstrap {
  const [ready, setReady] = useState(false);
  const [productLine, setProductLine] = useState("Moraine");
  const [service, setService] = useState<DiscoveryStatusDto | null>(null);
  const [doctorHint, setDoctorHint] = useState("moraine doctor --json");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const info = await appInfo();
        if (cancelled) return;
        const svc =
          info.serviceOnline === undefined
            ? null
            : info.serviceOnline
              ? info.serviceCompatible === false
                ? `service ${info.serviceVersion ?? "?"} (mismatch)`
                : `service ${info.serviceVersion ?? "ok"}`
              : "service offline";
        setProductLine(
          [
            info.name,
            info.version,
            info.gitCommit && info.gitCommit !== "unknown"
              ? info.gitCommit.slice(0, 7)
              : null,
            svc,
            !isTauriRuntime() ? "browser" : null,
          ]
            .filter(Boolean)
            .join(" · "),
        );
        if (info.doctorHint) setDoctorHint(info.doctorHint);
      } catch (e) {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : String(e));
          setProductLine("Moraine");
        }
      }

      try {
        const st = await discoveryStatus();
        if (!cancelled) setService(st);
      } catch {
        if (!cancelled) {
          setService({
            online: false,
            revision: 0,
            mode: "direct",
            message: "discovery status unavailable",
          });
        }
      }
      if (!cancelled) setReady(true);
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  return { ready, productLine, service, doctorHint, error };
}
