import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { SystemStateDto } from "@/shared/api/provision";

const mocks = vi.hoisted(() => ({
  provisionInspect: vi.fn(),
}));

vi.mock("@/shared/api", () => ({
  isTauri: true,
  pickMarkdownFile: vi.fn(),
}));
vi.mock("@/shared/api/provision", () => ({
  provisionInspect: mocks.provisionInspect,
}));
vi.mock("@/app/useProductBootstrap", () => ({
  useProductBootstrap: () => ({
    ready: true,
    service: { online: true },
    doctorHint: "",
    productLine: "ready",
    error: null,
  }),
}));
vi.mock("@/app/Workspace", () => ({
  Workspace: () => <div data-testid="workspace">Workspace</div>,
}));
vi.mock("@/app/ServiceHealthBanner", () => ({
  ServiceHealthBanner: () => <div data-testid="service-health-banner" />,
}));
vi.mock("@/features/shell/StatusBar", () => ({
  StatusBar: () => <div data-testid="status-bar" />,
}));
vi.mock("@/features/onboarding/HealthPanel", () => ({
  HealthPanel: () => <div />,
}));
vi.mock("@/features/onboarding/OnboardingWizard", () => ({
  OnboardingWizard: () => <div data-testid="onboarding">Onboarding</div>,
}));

import { App } from "./App";

const supportedLinux: SystemStateDto = {
  platform: {
    host: "linux",
    userPaths: "supported",
    suiteLayout: "supported",
    captureTransport: "supported",
    backgroundRuntime: "supported",
    desktopHost: "supported",
    userInstallation: "supported",
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
    installed: true,
    running: true,
    binaryPresent: true,
    registrationPresent: true,
    registrationValid: true,
    autostartEnabled: true,
    backend: "linux_systemd_user",
    supported: true,
    endpointReady: true,
    diagnosticsReady: true,
    captureReady: true,
    statusMessage: "running",
    platform: "linux",
  },
  agents: [],
  projects: [
    {
      path: "/tmp/project",
      name: "project",
      initialized: true,
      isGit: true,
      integrationConfigured: true,
      integrationNeedsRepair: false,
    },
  ],
  readiness: "ready",
};

describe("App native bootstrap", () => {
  it("transitions from loading to the supported product without changing hook order", async () => {
    let resolveInspect!: (state: SystemStateDto) => void;
    mocks.provisionInspect.mockReturnValueOnce(
      new Promise<SystemStateDto>((resolve) => {
        resolveInspect = resolve;
      }),
    );

    render(<App />);
    expect(screen.getByTestId("product-loading")).toBeInTheDocument();

    resolveInspect(supportedLinux);

    expect(await screen.findByTestId("workspace")).toBeInTheDocument();
    expect(screen.queryByTestId("product-loading")).not.toBeInTheDocument();
  });

  it("routes a manually staged Windows runtime to onboarding", async () => {
    mocks.provisionInspect.mockResolvedValueOnce({
      ...supportedLinux,
      platform: {
        ...supportedLinux.platform,
        host: "windows",
        userInstallation: "unsupported",
      },
      service: {
        ...supportedLinux.service,
        backend: "windows_task_scheduler",
        platform: "windows",
        installed: false,
        running: false,
        registrationPresent: false,
        registrationValid: false,
        endpointReady: false,
        diagnosticsReady: false,
        captureReady: false,
      },
      projects: [],
      readiness: "not_configured",
    });

    render(<App />);
    expect(await screen.findByTestId("onboarding")).toBeInTheDocument();
    expect(screen.queryByText(/not available on Windows yet/i)).not.toBeInTheDocument();
  });
});
