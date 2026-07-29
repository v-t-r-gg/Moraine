import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { SystemStateDto } from "@/shared/api/provision";

vi.mock("@/shared/api", () => ({ isTauri: true }));
vi.mock("@/shared/api/provision", async () => {
  const actual = await vi.importActual<typeof import("@/shared/api/provision")>(
    "@/shared/api/provision",
  );
  return {
    ...actual,
    provisionInspect: vi.fn(),
    provisionPlan: vi.fn(),
    provisionApplyPlan: vi.fn(),
    provisionRollback: vi.fn(),
  };
});

import { OnboardingWizard } from "./OnboardingWizard";
import {
  provisionApplyPlan,
  provisionInspect,
  provisionPlan,
} from "@/shared/api/provision";

const unsupportedSystem: SystemStateDto = {
  platform: {
    host: "windows",
    userPaths: "supported",
    suiteLayout: "supported",
    captureTransport: "unsupported",
    backgroundRuntime: "unsupported",
    desktopHost: "unsupported",
    userInstallation: "unsupported",
  },
  suite: {
    prefix: "",
    cliPath: "",
    cliPresent: true,
    servicePath: "",
    servicePresent: true,
    desktopPath: "",
    desktopPresent: true,
    manifestPath: "",
    manifestPresent: true,
    componentsCoherent: true,
  },
  service: {
    installed: false,
    running: false,
    binaryPresent: true,
    registrationPresent: false,
    registrationValid: false,
    autostartEnabled: false,
    backend: "unsupported",
    supported: false,
    endpointReady: false,
    diagnosticsReady: false,
    captureReady: false,
    statusMessage: "unsupported",
    platform: "windows",
  },
  agents: [],
  projects: [],
  readiness: "notConfigured",
};

describe("OnboardingWizard unsupported host defense", () => {
  it("shows no setup flow and performs no provisioning calls", () => {
    render(
      <OnboardingWizard
        systemState={unsupportedSystem}
        onComplete={vi.fn()}
      />,
    );

    expect(screen.getByTestId("unsupported-platform")).toBeInTheDocument();
    expect(screen.queryByTestId("onboarding-wizard")).not.toBeInTheDocument();
    expect(screen.queryByText("Welcome to Moraine")).not.toBeInTheDocument();
    expect(provisionInspect).not.toHaveBeenCalled();
    expect(provisionPlan).not.toHaveBeenCalled();
    expect(provisionApplyPlan).not.toHaveBeenCalled();
  });
});
