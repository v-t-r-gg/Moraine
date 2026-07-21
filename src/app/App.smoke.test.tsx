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

import { App } from "./App";

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
});
