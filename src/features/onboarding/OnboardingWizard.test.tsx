import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";

vi.mock("@/shared/api", () => ({
  isTauri: false,
}));

vi.mock("@/shared/api/provision", () => ({
  provisionInspect: vi.fn().mockResolvedValue({
    suite: {
      prefix: "/tmp",
      cliPath: "/tmp/moraine",
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
      installed: false,
      running: false,
      binaryPresent: false,
      statusMessage: "not set up",
      platform: "test",
    },
    agents: [
      {
        kind: "codex",
        id: "codex",
        displayName: "Codex",
        detected: true,
        executable: "/usr/bin/codex",
        status: "readyToConnect",
        statusMessage: "Ready to connect",
      },
    ],
    projects: [],
    readiness: "notConfigured",
  }),
  provisionPlan: vi.fn().mockResolvedValue({
    intent: {
      project: "/tmp/proj",
      agent: "codex",
      enableAutostart: true,
    },
    operations: [
      {
        id: "initialize_project",
        kind: "initializeProject",
        productLabel: "Preparing project records",
        detail: "x",
        reversible: true,
      },
    ],
    warnings: [],
    absoluteCli: "/tmp/moraine",
    productSummary: [
      "Enable background capture",
      "Initialize “proj”",
      "Connect Codex for this project",
      "Keep records inside /tmp/proj",
    ],
  }),
  provisionApply: vi.fn(),
  provisionRollback: vi.fn(),
}));

import { OnboardingWizard } from "./OnboardingWizard";

describe("OnboardingWizard", () => {
  it("walks welcome → agents → project with product copy only", async () => {
    render(<OnboardingWizard onComplete={() => {}} />);
    await waitFor(() => {
      expect(screen.getByTestId("onboarding-wizard")).toBeInTheDocument();
    });
    expect(screen.getByTestId("wizard-welcome")).toHaveTextContent(
      /local record of coding-agent work/i,
    );
    // No infra jargon on welcome
    expect(screen.getByTestId("wizard-welcome").textContent).not.toMatch(
      /systemd|systemctl|MCP|PATH|hooks\.json|\.moraine/i,
    );

    fireEvent.click(screen.getByRole("button", { name: /Continue/i }));
    await waitFor(() => {
      expect(screen.getByTestId("wizard-agents")).toBeInTheDocument();
    });
    expect(screen.getByTestId("wizard-agents")).toHaveTextContent(/Codex/);
    expect(screen.getByTestId("wizard-agents")).toHaveTextContent(/Detected/);

    fireEvent.click(screen.getByRole("button", { name: /Continue/i }));
    await waitFor(() => {
      expect(screen.getByTestId("wizard-project")).toBeInTheDocument();
    });
    expect(screen.getByTestId("wizard-project").textContent).not.toMatch(
      /systemd|MCP|hooks/i,
    );
  });

  it("shows planned changes without infra jargon", async () => {
    const { provisionPlan } = await import("@/shared/api/provision");
    render(
      <OnboardingWizard onComplete={() => {}} initialProject="/tmp/proj" />,
    );

    // Drive wizard through product steps using role queries (stable across re-renders).
    fireEvent.click(screen.getByRole("button", { name: /^Continue$/i }));
    expect(await screen.findByText(/Coding agent/i)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /^Continue$/i }));
    expect(await screen.findByText(/Select a project/i)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /^Continue$/i }));

    expect(await screen.findByText(/Planned changes/i)).toBeInTheDocument();
    expect(provisionPlan).toHaveBeenCalled();
    const plan = screen.getByTestId("wizard-plan");
    expect(plan).toHaveTextContent(/Enable background capture/);
    expect(plan).toHaveTextContent(/Connect Codex/);
    expect(plan.textContent).not.toMatch(/systemctl|systemd|MCP|hooks\.json/i);
  });
});
