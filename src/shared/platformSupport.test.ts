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
  it("requires every product and desktop capability", () => {
    expect(deriveDesktopProductSupport(linux).desktopSupported).toBe(true);
    for (const field of [
      "captureTransport",
      "backgroundRuntime",
      "userInstallation",
      "desktopHost",
    ] as const) {
      expect(
        deriveDesktopProductSupport({ ...linux, [field]: "degraded" })
          .desktopSupported,
      ).toBe(false);
    }
  });

  it("models Windows as described but product unsupported", () => {
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
  });
});
