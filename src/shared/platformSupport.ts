import type {
  CapabilityStatusDto,
  HostPlatformDto,
  PlatformCapabilitiesDto,
} from "@/shared/api/provision";

export interface ProductCaptureSupport {
  supported: boolean;
  reason: "supported" | "capture_unsupported" | "runtime_unsupported";
  distributionSupported: boolean;
}

export interface DesktopProductSupport extends ProductCaptureSupport {
  host: HostPlatformDto;
  desktopSupported: boolean;
  message: string;
}

function isSupported(status: CapabilityStatusDto): boolean {
  return status === "supported";
}

export function deriveDesktopProductSupport(
  capabilities: PlatformCapabilitiesDto,
): DesktopProductSupport {
  const reason = !isSupported(capabilities.captureTransport)
    ? "capture_unsupported"
    : !isSupported(capabilities.backgroundRuntime)
      ? "runtime_unsupported"
      : "supported";
  const supported = reason === "supported";
  const desktopSupported = supported && isSupported(capabilities.desktopHost);
  return {
    host: capabilities.host,
    supported,
    desktopSupported,
    distributionSupported: isSupported(capabilities.userInstallation),
    reason,
    message: desktopSupported
      ? "Moraine background capture is supported."
      : `Moraine background capture is not available on ${capabilities.host} yet.`,
  };
}
