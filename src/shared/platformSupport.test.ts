import { describe, expect, it } from "vitest";
import type { PlatformCapabilitiesDto } from "@/shared/api/provision";
import { deriveDesktopProductSupport } from "./platformSupport";

const linux: PlatformCapabilitiesDto = {
  host: "linux",
  userPaths: "supported",
  suiteLayout: "supported",
  captureTransport: "supported",
  backgroundRuntime: "supported",
  desktopHost: "supported",
  userInstallation: "supported",
};

describe("deriveDesktopProductSupport", () => {
  it("requires runtime and desktop capabilities but not distribution", () => {
    expect(deriveDesktopProductSupport(linux).desktopSupported).toBe(true);
    for (const field of ["captureTransport", "backgroundRuntime", "desktopHost"] as const) {
      expect(
        deriveDesktopProductSupport({ ...linux, [field]: "degraded" })
          .desktopSupported,
      ).toBe(false);
    }
    const staged = deriveDesktopProductSupport({
      ...linux,
      host: "windows",
      userInstallation: "unsupported",
    });
    expect(staged.supported).toBe(true);
    expect(staged.desktopSupported).toBe(true);
    expect(staged.distributionSupported).toBe(false);
  });

  it("fails closed for unsupported runtime capabilities", () => {
    const support = deriveDesktopProductSupport({
      ...linux,
      host: "windows",
      captureTransport: "unsupported",
      backgroundRuntime: "unsupported",
      desktopHost: "unsupported",
      userInstallation: "unsupported",
    });
    expect(support.supported).toBe(false);
    expect(support.desktopSupported).toBe(false);
    expect(support.reason).toBe("capture_unsupported");

    expect(
      deriveDesktopProductSupport({
        ...linux,
        host: "mac_os",
        backgroundRuntime: "unsupported",
      }).desktopSupported,
    ).toBe(false);
  });
});
