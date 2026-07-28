import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

vi.mock("@/shared/api", async () => {
  const actual = await vi.importActual<typeof import("@/shared/api")>("@/shared/api");
  return {
    ...actual,
    isTauri: false,
    appInfo: vi.fn().mockResolvedValue({
      name: "Moraine",
      version: "test",
      gitCommit: "abc1234",
      dataDir: "",
      historyDir: "",
      configDir: "",
      serviceOnline: false,
      doctorHint: "moraine doctor --json",
    }),
    takeStartupPath: vi.fn().mockResolvedValue(null),
    pickMarkdownFile: vi.fn().mockResolvedValue(null),
    onFileChanged: () => () => {},
  };
});

vi.mock("@/shared/api/discovery", async () => {
  const actual = await vi.importActual<typeof import("@/shared/api/discovery")>(
    "@/shared/api/discovery",
  );
  return {
    ...actual,
    discoveryStatus: vi.fn().mockResolvedValue({
      online: false,
      revision: 0,
      mode: "direct",
      message: "service unavailable",
    }),
    discoveryProjects: vi.fn().mockResolvedValue([]),
    subscribeDiscoveryRevision: () => () => {},
  };
});

vi.mock("@/shared/api/provision", () => ({
  provisionInspect: vi.fn().mockResolvedValue({
    suite: {
      prefix: "",
      cliPath: "",
      cliPresent: true,
      servicePath: "",
      servicePresent: false,
      desktopPath: "",
      desktopPresent: true,
      manifestPath: "",
      manifestPresent: false,
      componentsCoherent: true,
    },
    service: {
      installed: true,
      running: true,
      binaryPresent: true,
      registrationPresent: true,
      registrationValid: true,
      autostartEnabled: true,
      endpointReady: true,
      statusMessage: "running",
      platform: "test",
    },
    agents: [],
    projects: [{
      path: "/tmp/p",
      name: "p",
      initialized: true,
      isGit: true,
      integrationConfigured: true,
      integrationNeedsRepair: false,
    }],
    readiness: "ready",
  }),
  provisionHealth: vi.fn().mockResolvedValue({ ok: true, checks: [], readiness: "ready" }),
  provisionRepair: vi.fn(),
  provisionPlan: vi.fn(),
  provisionApply: vi.fn(),
  provisionRollback: vi.fn(),
}));

import { App } from "./App";
import { provisionInspect } from "@/shared/api/provision";

describe("App product shell (C3)", () => {
  it("starts on ledger workspace with service health banner", async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByTestId("product-shell")).toBeInTheDocument();
    });
    expect(screen.getByTestId("service-health-banner")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Open legacy document/i })).toBeInTheDocument();
    expect(screen.getByTestId("service-health-banner")).toHaveTextContent(/Discovery/i);
  });

  it("does not let a legacy localStorage flag override real setup state", async () => {
    Object.defineProperty(globalThis, "localStorage", {
      configurable: true,
      value: {
        getItem: vi.fn().mockReturnValue("1"),
        setItem: vi.fn(),
        removeItem: vi.fn(),
      },
    });
    vi.mocked(provisionInspect).mockResolvedValueOnce({
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
        endpointReady: true,
        statusMessage: "running",
        platform: "test",
      },
      agents: [],
      projects: [],
      readiness: "degraded",
    });

    render(<App />);

    expect(await screen.findByTestId("onboarding-wizard")).toBeInTheDocument();
  });
});
