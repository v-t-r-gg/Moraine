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
      dataDir: "",
      historyDir: "",
      configDir: "",
    }),
    takeStartupPath: vi.fn().mockResolvedValue(null),
    openDocument: vi.fn().mockResolvedValue({
      meta: {
        id: "1",
        path: "welcome.md",
        title: "welcome.md",
        dirty: false,
        lastSavedAt: null,
        lastModifiedOnDisk: null,
        byteLen: 10,
      },
      content: "# hi\n\n## Human notes\n\nnotes\n",
      contentHash: "0".repeat(64),
    }),
    onFileChanged: () => () => {},
  };
});

// Editor is heavy; stub to avoid Yjs/Tiptap complexity in smoke test
vi.mock("@/features/editor/Editor", () => ({
  Editor: () => <div data-testid="editor-stub">editor</div>,
}));

import { App } from "./App";

describe("App shell", () => {
  it("starts and shows toolbar", async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /Open/i })).toBeInTheDocument();
    });
    expect(screen.getByRole("button", { name: /Save/i })).toBeInTheDocument();
  });
});
